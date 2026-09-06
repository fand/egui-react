# egui-react architecture

A Rust library for writing egui apps in a React style (JSX, function components, hooks). This document records the design decisions. If the implementation drifts from this document, update this document first.

## 1. Goals and non-goals

### Goals

- User code is Rust only. You write UI with the `rsx!` macro (JSX-like syntax), `#[component]` functions, and hooks.
- Native (macOS / Windows / Linux) and wasm are required targets. iOS / Android come later.
- Remove the hooks ceremony that made Yew painful (`'static` closures, `Rc<RefCell>`, `Callback`, `.clone()` on everything) by using the nature of immediate mode.
- Provide Flexbox / Grid layout as a first-class citizen.

### Non-goals

- Running JS / TS React. No JS runtime is bundled.
- Faithful reproduction of React semantics. No VDOM, no reconciler, no `memo()`, no `useCallback`.
- Signal-style fine-grained reactivity. There is no subscription mechanism.
- Screen reader support on web. This depends on web support in egui / AccessKit. egui emits its widget tree to AccessKit, and on native it reaches the OS accessibility API, but on web there is no upstream adapter that mirrors it into the DOM (`docs/tasks/a11y/`). Element labels (`label` on `Button`, `alt` on `Image`) will work as-is the day an adapter lands, so they are filled in ahead of time.

## 2. Basic principles

### 2.1 No reconciler

React's diff exists to "minimize DOM operations because they are expensive". egui re-emits every widget every frame, so there is no retained side to diff against. `rsx!` does not build a retained tree. It expands directly into egui calls in place.

As a result, event handlers are called right where they are expanded and then thrown away. Handlers do not live across frames, so neither `'static` nor `Rc` is needed, and local variables can be borrowed with `&mut` as usual. This is the biggest advantage of this design, and every decision below puts keeping this property first.

### 2.2 State lives in a store keyed by Id

egui does not keep a widget tree, but it does store the open/closed state of `CollapsingHeader` and scroll positions in `Context::Memory`, keyed by `Id`. `Id` is a hierarchical hash derived from the parent down, so widgets drawn in the same place in the same order get the same Id every frame. This library keeps user state (hooks) by the same principle. Instead of adding a new paradigm to egui, it puts user state on top of egui's paradigm.

State identity is "position in the tree + call site + key". This is the same idea as Jetpack Compose's positional memoization and SwiftUI's structural identity, and differs from React's "call order index". Drawing is Dear ImGui / egui style; state identity is Compose / SwiftUI style.

### 2.3 Pull-based, re-read every frame

Signals (SolidJS, Preact Signals) are push-based: state knows its subscribers and pushes changes to just the affected nodes. This design is pull-based: nobody subscribes, and everything is re-read every frame. egui has no retained side to skip, so fine-grained updates gain nothing.

egui's "every frame" is not a fixed 60fps. It redraws only when there is input or when `request_repaint` is called. With nothing going on, zero frames. To keep this property, the runtime always calls `request_repaint` when state changes (see 5.6).

Drawing cost cannot be cut, so only heavy derived values are cached by hand with `use_memo`. Long lists are virtualized with `ScrollArea::show_rows` and the like, just as in a plain egui app.

## 3. Core concepts

### 3.1 `Cx`

The context passed to components and hooks. It holds the following.

- `store: &'s Store`: a shared reference to the hooks state store. The lifetime `'s` is independent of the `&mut Cx` borrow, and the handles that hooks return carry this `'s`.
- `surface: Surface<'u>`: the current drawing target. Either `Surface::Ui(&mut egui::Ui)` or `Surface::Taffy(&mut egui_taffy::Tui)`. Whether we are inside a taffy container is this enum itself (see section 6). The field is private; the `cx.ui()` method returns the current `&mut egui::Ui` (`tui.egui_ui_mut()` for Taffy).
- `scope: Id`: the Id of the current component scope. The base for deriving hook Ids.

The main methods are below.

| Method | Meaning |
|---|---|
| `ui() -> &mut egui::Ui` | The current `Ui`. In Taffy mode, an escape hatch that does not get taffy placement |
| `ctx() -> &egui::Context` / `scope_id() -> Id` / `in_taffy() -> bool` | Accessors |
| `scope(source, f)` | Goes one level deeper in component scope. Ui mode uses `ui.push_id`, Taffy mode uses `tui.with_auto_id_prefix`. In both cases the hook Ids and the egui widget Ids split at the same time |
| `hook_scope(location, f)` | Scope for a custom hook. Does not touch egui Ids |
| `leaf(&ItemStyle, f)` | Draws one egui widget. In Taffy mode it becomes a taffy leaf via `tui.style(style.to_taffy()).ui(f)`; in Ui mode it is `f(ui)` (`style` is ignored) |
| `leaf_fill(&ItemStyle, f)` | Same as `leaf` but does not report content size (`min_size = 0`, `infinite`). For widgets like `ScrollArea` that "fill the given space and return that size". Placing one with `leaf` locks it to the size of the first frame. taffy decides the size from `w` / `h` / `grow` / remaining space |
| `container(id, taffy::Style, f)` | Creates a taffy node and draws inside it with a Taffy-mode `Cx`. In Ui mode this is `egui_taffy::tui(ui, id).reserve_available_width()`; in Taffy mode it adds a child node |
| `defer(f)` | Pushes onto the deferred queue that runs at the end of the pass (see 5.5). `f` is `'static` |

When entering an egui container closure (`ui.vertical(|ui| ..)` etc.), rebuild `Cx::new(store, ui, scope)` with the inner `Ui` as before. No implicit globals or thread-locals. `cx` is always passed around explicitly.

### 3.2 `View` and `rsx!`

`rsx!{ ... }` returns `impl View`. `View` is a trait with `fn show(self, cx: &mut Cx)`, and in practice it is an `FnOnce(&mut Cx)` closure. `View` is also implemented for the following.

- `()`: does nothing. The `children` of an element with no children is this.
- `&str`, `String`: drawn as a `Label`.
- `Option<V: View>`: drawn if `Some`.
- `Vec<V: View>`, `[V: View; N]`: drawn in order.
- `FnOnce(&mut Cx)`: called as-is (escape hatch for touching egui directly).

A blanket impl for `IntoIterator<Item = V>` conflicts under coherence with both the `FnOnce` blanket impl and `Option<V>`, so it was not adopted; `Vec` and arrays get separate impls. Repetition inside `rsx!` can be written with `for`, so there is no practical difference.

`rsx!` always emits `::egui_react::view(|cx| { .. })`. `pub fn view<F: FnOnce(&mut Cx<'_, '_>)>(f: F) -> impl View` is a helper that exists only to pin the closure's argument type; writing a bare closure in an `impl View` position sometimes fails to infer the type of `cx`. Users also use `view(|cx| ..)` when writing an escape hatch.

The `rsx!` closure is not `move` and borrows locals. The closure is consumed right away inside the generated statement, so the borrow is short-lived.

Inside `rsx!` you can write the following.

- Elements: `<Button on_click={..}>"text"</Button>`. Every element name is a Rust function component (paths like `<elements::Button/>` also work). There are no HTML-style lowercase tags.
- Expression embedding: `{expr}`. `expr: impl View`. Unquoted text is an error; strings must always be literals.
- Attributes: `key={expr}`, `on_*={handler}`, `events={closure}`, layout attributes (`w` / `h` / `grow` / `p` / `m` etc., collected into `.style(ItemStyle::default()..)`), and everything else is a Props setter. An attribute with no value (`disabled`) is `true`.
- Child nodes are always passed with `.children(..)`. With no children it is `()`; a single string literal or a single `{expr}` is that expression itself; anything else is `view(|cx| ..)`. This lets `<Button>"OK"</Button>` with `children: impl Into<WidgetText>` and `<View>..</View>` with `children: impl View` share the same syntax.
- Control flow: write `if` / `else` / `for` / `match` directly (the Dioxus way). Because expansion is direct, the macro only needs to emit real Rust control flow, which avoids the "cannot return a borrow from `FnMut`" problem that `items.iter().map(|i| rsx!{..})` causes.
- `key={expr}`: mixed into the element's scope Id. Required when drawing components with hooks inside `for`. The expression must satisfy `Hash + Debug` (because `Ui::push_id` in egui 0.36 requires `AsIdSalt = Hash + Debug`).
- Handlers are generated and called immediately in the form `Handler::call(closure, payload)`. The `(|| ..)()` from the hand-written spike expansion is not used, so the `rsx!` expansion does not need `#[allow(clippy::redundant_closure_call)]`.

### 3.3 Components

```rust
#[component]
fn Counter(cx: &mut Cx, initial: i32, label: Option<&str>, #[event] on_change: i32) {
    ...
}
```

`#[component]` generates the following.

- A Props struct `CounterProps`. It gets typed-builder's `#[derive(TypedBuilder)]` and becomes the second argument of `Counter(cx, props)`. The function's arguments become the Props fields as-is, and elided lifetimes on `&T` are rewritten to the Props `'e`. `impl Trait` arguments are desugared to type parameters.
- Optional props are arguments of type `Option<T>` (automatic) and arguments marked `#[prop(default)]` / `#[prop(default = expr)]`. With `#[prop(into)]` the setter takes `impl Into<T>`. Everything else is required; leaving it out is a typed-builder compile error.
- A `children` field always exists. If not declared, `children: ()` is generated with `#[builder(default)]` (because `rsx!` always calls `.children(..)`). A component that takes children declares `children: impl View`.
- From `#[event]` arguments, an event enum `CounterEvent` and an `events` field (see 3.6).
- An impl of the `Props` trait. `rsx!` uses it to get the builder from `props_builder(&Counter)`.

The tail expression of the body is rewritten to `::egui_react::View::show(tail, cx)`. Wrapping the whole body as `View::show({ body }, cx)` does not compile, because the guards would still borrow the block's locals when returned, so only the tail expression is replaced. A body that returns a different `rsx!` per `if` / `match` arm has mismatched closure types, so write it as `rsx!{ if .. }`.

`<Counter initial={0} />` expands to the following.

```rust
cx.scope((file!(), line!(), column!(), 3usize, key), |cx| {
    Counter(cx, ::egui_react::props_builder(&Counter).initial(0).children(()).build());
});
```

`scope` goes one level deeper in `cx.scope` and at the same time calls `ui.push_id` (`tui.with_auto_id_prefix` in Taffy mode). This keeps both the hook Ids and the egui widget Ids stable per component instance. The Id material is the `rsx!` call site, the element's sequence number within that `rsx!`, and `key`. A function item's type cannot be named, so the Props type is inferred from the `Fn` bound of `props_builder<P: Props, F: Fn(&mut Cx, P)>(_: &F) -> P::Builder`. The user only needs to `use` `Counter`.

If every element created a child `Ui`, egui containers that carve space out of the parent `Ui` (docked panels) and ones that rewrite the parent `Ui` (`Grid`'s `Ui::end_row`) would not work. So there is `#[component(shares_ui)]`. A component marked with it still gets a deeper hook scope as usual, but skips `Ui::push_id` and draws directly into the parent `Ui`. The implementation is `const SHARES_UI: bool` on `Props` (default `false`; `#[component(shares_ui)]` sets it to `true`), and `rsx!` routes element calls through `::egui_react::__private::enter_scope(cx, source, props, Name)`. `enter_scope` looks at `P::SHARES_UI` and picks `cx.scope` or `cx.scope_sharing_ui`. `Panel` / `CentralPanel` / `Row` use this.

Instance identity behaves as follows (matching React).

- Two `<Counter/>` written in different places have independent state.
- `<Counter key={i}/>` inside `for` is distinguished by key. If you forget the key, Ids collide and collision detection (3.4) warns.
- With `if show { <Counter/> }`, on the frame where `show` becomes false the Counter's Id is not visited, and the end-of-frame sweep drops the state and runs the `use_effect` cleanup (unmount). When it becomes true again, it starts over from initialization (mount).
- Moving the position in the tree changes the Id and resets the state.

### 3.4 Id derivation and collision detection

A hook's Id is derived as `scope.with(Location::caller())`. Hook functions like `use_state` have `#[track_caller]` and get the caller's `file:line:column`. React's rule "call hooks unconditionally and in the same order" is not needed; you may call `use_state` inside `if`.

Custom hooks get `#[hook]`. `#[hook]` adds `#[track_caller]` to the function and wraps the body in `cx.hook_scope(Location::caller(), |cx| { .. })`. Nested hook Ids then form a stack of call sites: "scope -> custom hook call site -> inner hook call site", unique at any depth. This is what guarantees composability.

If the same Id is requested twice in one pass, that is a collision (detectable via the visited mark). If the first guard is still alive, it is a double borrow of the same `RefCell`, so we panic with a message that explains the cause and the fix (a missing `#[hook]`, a missing `key` inside `for`). It is not a bare `RefCell` panic. If the first guard has already dropped, the second request silently reuses the same slot (`for i in 0..3 { use_state(cx, || i) }` returns 0 all three times). This is exactly the "silent bug" we want to avoid, so in addition to recording it and `log::warn!`, debug builds show a warning on screen with the same UX as egui's Id collision warning. The main purpose of the overlay is this latter case. The implementation is at the end of `Store::end_pass`: if `warn_on_collision` (default `cfg!(debug_assertions)`, toggled with `set_warn_on_collision`) is on and there were collisions, it places an `egui::Area` with `Order::Debug` in the top left and prints `egui-react: hook id collision at {file}:{line}:{column}. Wrap custom hooks in #[hook], or add key= inside loops.` in red. The same call site is collapsed to one line per pass.

Rejected alternative: mix the "occurrence count at the same site" into the Id to remove collisions. Collisions disappear, but when the structure changes, state silently moves to another instance (the same phenomenon as a React rules-of-hooks violation). Because a changing occurrence count in a loop is allowed as legitimate use, it cannot be detected either. This design trades a "loud error" for a "silent bug", so it is not adopted.

`Location::caller()` changes when code is edited. That is fine for state that lives within the process, but identifying persisted state by position means inserting a line makes saved data unreadable. `use_persisted` requires an explicit string key.

### 3.5 `State<'s, T>`

The handle returned by `use_state(cx, init)`. A `RefMut`-like guard onto a slot in the store that implements `Deref<Target = T>` and `DerefMut`. `*count += 1` and `&mut *name` work as-is. The value lives directly in the store and is not cached in the handle, so there is no write-back on Drop and no `T: Clone` requirement.

The guard lives only for the duration of the component body (`'s` is the store lifetime, but as a local variable it drops when the body exits). A custom hook can return the guard by value. It can also return a closure that holds the guard by move.

As a helper, `count.into_handle()` consumes the guard and turns it into a `Handle<'s, T>`. `State::handle(&self)` is not provided, because using a `Handle` while the guard is alive panics with a double borrow of the same `RefCell`. For state that you want as a `Handle` from the start, for example to pass into context, use `use_handle(cx, init)`. There are only two ways to get a `Handle`: `use_handle` (creates no guard) and `into_handle` (releases the guard), and `provide_context` accepts only a `Handle`, so the public API cannot create a state where "a guard and a `Handle` exist for the same slot at the same time" (confirmed in the spike). `Handle` is Copy and offers `.get()` (`T: Clone`), `.set(v)`, `.update(|&mut T|)`, `.with(|&T|)` through `&self`. Use it when carrying state around in a struct within a frame, or with `use_context` (section 4). To carry across frames, use `Dispatch` (section 4).

### 3.6 Events (callback props)

With direct expansion, handlers on sibling elements are generated and consumed in order, so they do not clash even when they capture the same state with `&mut`. The only clash is "passing multiple callback props to the same element, where both capture the same state with `&mut`". `rsx!` solves this by fusing them into one closure.

```rust
// user code
<Dialog title="Quit?" on_ok={|| *open = false} on_cancel={|| *open = false} />

// expansion
Dialog(cx, DialogProps {
    title: "Quit?",
    events: &mut |ev| match ev {
        DialogEvent::Ok(a)     => Handler::call(|| *open = false, a),
        DialogEvent::Cancel(a) => Handler::call(|| *open = false, a),
        _ => {}
    },
});
```

- `rsx!` builds `DialogEvent::Ok` textually from the element name `Dialog` and the attribute name `on_ok` (strip `on_`, PascalCase). No type information is needed. A nonexistent event name has no variant, so it is a compile error.
- `Handler<A, Marker>` is a trait that accepts both `FnOnce() -> R` and `FnOnce(A) -> R`. The two blanket impls conflict under coherence, so they are told apart by the marker type argument `(Arity0, R)` / `(Arity1, R)`. The marker is always inferred, and `on_ok={|| ..}`, `on_change={|v| ..}` (even without an argument type annotation), a `|| ..` that drops the payload, and a body that returns a non-`()` value can all be called through the single form `::egui_react::Handler::call(closure, a)` (confirmed in the spike). The macro always emits this fully qualified path.
- The closure literals are generated and called immediately inside the `match` arms. Only the single outer fused closure captures `open` with `&mut`.
- On the component side, `#[event] on_ok: ()` becomes an `Emitter`, fired with `on_ok.emit(())`. `Emitter<'a, 'e, E, A>` holds an `&'a` reference to a shared `EventSink<'e, E> = RefCell<&'e mut dyn FnMut(E)>` and a function `fn(A) -> E` that wraps the payload `A` into the variant. Several can be alive at once inside the child (the `RefCell` is immutable, so two lifetimes are needed). A re-entrant emit panics with a clear message.
- The Props `events` field is `Option<&'e mut dyn FnMut(E)>` with `#[builder(default, setter(strip_option))]`. `rsx!` passes the fused closure by `&mut`, and passes nothing if there is no `on_*` at all. When nothing is passed, the component body falls back to a local no-op closure in the `match` arm, so `emit` does nothing.
- The escape hatch of writing `events={|e| match e {..}}` with no `on_*` at all is also allowed. `rsx!` wraps the expression in `&mut (..)` and passes it to `events`.
- Payloads may be borrows (`on_change: &str` is possible). The event enum carries only the generics the payloads actually use (`CounterEvent<'e>` for `&'e str`).
- The fused closure names `CounterEvent::Ok(..)`, so wherever you write `<Counter on_ok=../>`, `CounterEvent` must be imported in addition to `Counter` (glob `use` the module, or write a path like `<components::Counter/>`).

Rejected alternative: the child returns events as its return value, and the macro `match`es after the child call. Simple, but when several events happen in one frame (TextEdit change and submit) a `Vec` is needed, and borrowed payloads cannot be returned. Forcing users to write only with a Cell-style `Handle` loses the `*count += 1` sugar. Having users write `callback={|e| match e {..}}` every time is verbose; the fused closure is the macro doing that for them.

This is effectively the Elm / Yew `Msg` enum, but it is generated and hidden, so users write with the same feel as React's `onOk` / `onCancel`.

### 3.7 Remaining borrow constraints

Two borrow conflicts remain that the fused closure does not remove. Both are constraints of Rust itself.

The first is "a handler inside a loop modifies the state the loop iterates over". The `for` expansion reads with a shared borrow, so `todos.remove(i)` inside the handler is a compile error (not a runtime panic). The fix is `todos.update_later(move |t| { t.remove(i); })`, which pushes onto the write queue applied at the end of the pass. This matches egui's "delete later" convention. The queue lives until the end of the pass, so the closure is `'static`, and using locals such as loop variables needs `move`. To borrow instead, use `Dispatch` or clone the value. `update_later` can be called while the guard is alive (by apply time the guard is long gone).

The second is "passing to the same element both a value prop that borrows state and a handler that modifies the same state". `<Dialog title={&*title} on_rename={|s| *title = s} />` gives E0502, because the props struct holds a shared `&str` borrow and the fused closure holds a mutable borrow of the same state (confirmed in the spike). The fix is either to copy the value first (`title={title.clone()}`) or to route the write through `update_later`; either way the user writes it. Props stay borrowed (zero-copy) by default, and `rsx!` adds no implicit clones. If this comes up often and hurts in the phase 3 examples, avoid it with a component design that "passes read and write through one `&mut`", like `bind` on `TextEdit`.

## 4. Hooks list

| hook | Semantics |
|---|---|
| `use_state(cx, init) -> State<T>` | Keeps `T` in the store. Calls `init` only the first time |
| `use_persisted(cx, "key", init) -> State<T>` | `T: Serialize + DeserializeOwned`. Saved to eframe storage and survives restarts. The key is an explicit string (see below) |
| `use_memo(cx, deps, f) -> &T` | Re-runs `f` only when the hash of deps changes |
| `use_effect(cx, deps, f)` | Runs `f` **in place** when deps change (and the first time). `f` may return a cleanup (`FnOnce + 'static`) |
| `use_reducer(cx, reducer, init) -> (State<S>, Dispatch<Msg>)` | `Dispatch` is `Clone + Send + 'static`. `send` pushes onto a queue and calls `request_repaint`; the reducer is applied in order **the next time the hook is visited** (see below) |
| `provide_context(cx, handle, children)` / `use_context::<T>(cx) -> Option<Handle<T>>` | Valid only while descendants render. Returns a `Handle`, not a guard (to avoid a double borrow with the parent's guard). The store holds a stack of `(TypeId, slot Id)`, and `use_context` rebuilds a `Handle` from the slot Id. `Handle` itself carries `'s`, so it cannot go into `dyn Any` |
| `use_future(cx, deps, \|\| async { .. }) -> &Poll<T>` | Rebuilds and starts the future each time the hash of deps changes. Native uses one thread + `pollster::block_on`, wasm uses `wasm_bindgen_futures::spawn_local`. On completion it writes the result to the slot and calls `request_repaint`. Stale results that arrive after deps changed are dropped. `Pending` is counted toward the nearest `<Suspense>` (see below) |
| `spawn(fut)` | Not a hook, but the same execution mechanism. Fire and forget. Send results with `Dispatch` (`spawn(async move { dispatch.send(Msg::Saved(api.await)) })`) |
| `cx.defer(f)` / `state.update_later(f)` / `handle.update_later(f)` | Deferred queue run at the end of the pass (before the sweep). The closure is `'static`. `update_later` calls `request_repaint` when applied; `defer` does not, because it does not touch state |

`use_callback` and `memo` are not provided. There is no diffing, so keeping referential identity has no meaning. The replacement for a callback that crosses frames is `Dispatch`.

### `use_effect` details

- deps are compared by `Hash`. Requiring `PartialEq + Clone + 'static` would forbid borrowed deps like `(&str, &[T])`. Hash collisions are as negligible as egui Ids.
- The body runs in place at the call site. React runs effects after commit to measure the DOM, but in egui the `Response` rect is in hand right after the widget call. Deferring would make the body `'static` and bring back the Yew ceremony. Running in place lets it borrow `State` guards and locals.
- The body may return either `()` or a cleanup closure. The two blanket impls of `IntoCleanup<Marker>` that accept these (`()` and `FnOnce() + 'static`) conflict under coherence, so they are told apart by a marker type argument. The marker is always inferred and never shows up at the call site.
- The cleanup is stored, so it is `'static`. What it holds is what the body created (task handles, unsubscribers), so this is naturally satisfied. To change other state on unmount, hold a `Dispatch`.
- If there is a previous cleanup, it runs before the body. Unmount is detected by the end-of-pass sweep, which runs the cleanup.
- On the second pass of a multi-pass frame the deps match, so it does not re-run.

### `use_memo` details

- The return value is `&'s T` (`'s` is the store lifetime). It is independent of the `&mut Cx` borrow, so it can coexist with `State` guards and later hooks.
- The value is kept outside the `RefCell`, in an `elsa::FrozenVec<Box<dyn Any>>` on the slot. When the deps hash changes, a new value is pushed and a new reference is returned. Old values are not removed, because an `&'s T` handed out earlier in the same pass may still point at them; the end-of-pass sweep keeps the newest one and drops the rest.
- deps comparison is the same Hash as `use_effect`.

### `use_persisted` details

- The slot Id is `Id::new(("egui_react_persisted", key))`, not the scope, and does not depend on the call site. Adding lines never makes saved data unreadable; in exchange, using the same key in two places shares the same single value, and visiting it twice in one pass is recorded as a collision as usual.
- `Store` holds a `HashMap` of "key -> JSON string". `load_persisted(&mut self, json)` loads it wholesale, and `save_persisted(&self) -> String` serializes the live slots, overwrites the map, and then turns the whole thing into JSON. Unreadable JSON is ignored with `log::warn!`, and the value falls back to `init`.
- `Slot` holds `persist: Option<(key, fn(&dyn Any) -> Option<String>)>`. When the sweep drops a slot with persist, it serializes it into the map first. Even after unmount it remains for the next launch.
- The save format is JSON, written to eframe `Storage` under the single key `"egui_react"`. The runner's `App::save` calls it (eframe calls it on `auto_save_interval` and at exit).

### `use_reducer` details

- Messages are applied "the next time the hook is visited", not "at the end of the pass". Two reasons. (a) Applying at the end of the pass requires storing the reducer, which makes it `'static`; at visit time the reducer can be an ordinary closure. (b) If a message arriving from another thread were applied at the end of the pass, "the body of the next pass sees the old state, it is applied at the end of that pass, and it shows on the frame after", one frame late. With apply-at-visit, the body of the next frame, triggered by the `request_repaint` in `send`, sees the new state.
- The queue is drained on every visit, so a message is never applied twice on the second pass of the same frame.
- The visible behavior when `send` is called from a handler (reflected next frame) is the same as writing to `State` (5.7).
- Two slots are used. The state slot holds the bare `S` (so `State` and `update_later` can downcast it as-is), and the message queue lives in a separate slot holding `Arc<Mutex<Vec<M>>>`.

### `use_future` details

- The platform difference in the execution mechanism is confined to the `SpawnFuture<T>` trait and `task::spawn` in core. The bound is `Future<Output = T> + Send + 'static` on native and `Future<Output = T> + 'static` on wasm, given the same name via two blanket impls switched by cfg. The difference never shows in types the user writes. The result type `T` requires `Send + 'static` on both platforms. Only the future side varies per platform, so the user's types can be one kind (to return `JsValue` on wasm, convert to a `Send` type inside the future).
- The native executor is one thread + `pollster::block_on`. One thread per future, no pool. Enough for "just waiting" futures like `ehttp` or file IO, with minimal dependencies. CPU-heavy work should split off its own thread inside the future. If thread creation fails, the future is dropped and `log::error!` is called; no panic (the hook stays at `Pending`). Apps that need tokio call `Handle::current().spawn(..).await` inside the future.
- `f` is called in place at the call site (same as `use_effect`). It may read locals and `State` guards to build the future. The future itself is `'static`, so it holds `move`d clones.
- Two slots are used (split the same way as `use_reducer`). The **state slot** holds the deps hash and the same `FrozenVec` as `use_memo`; it pushes `Poll::Pending` at start and `Poll::Ready(T)` when the result arrives. The **receive slot** holds `Arc<Mutex<Option<(u64, T)>>>` (a generation-tagged result) and `Cell<u64>` (the last started generation). The return value is `&'s Poll<T>`, same as `use_memo`, and coexists with `State` guards. Old `Poll`s may still be pointed at by references handed out earlier in the same pass, so the end-of-pass sweep (the existing `prune_memo`) keeps only the newest one.
- Right after starting, it only returns `Pending` and does not try to receive. Even if the future completed on the spot, it is picked up next frame (completion fires `request_repaint`, so nothing is missed). On the second pass of the same frame the deps match, so it does not start; it only tries to receive.
- Rebuilding the future when deps change happens at visit time. The old running future cannot be stopped and runs to completion, but its result is dropped due to a generation mismatch. If both arrive before the visit in the order "new -> old", the old one would overwrite the new result, so the future side writes only "when the cell is empty, or when it is newer than the stored generation".
- On unmount, both slots are dropped by the sweep. The running future holds its own side of the `Arc`, so the write succeeds and one extra `request_repaint` fires. Nobody reads it next frame, and the last `Arc` reference goes away when the future ends.
- Just before returning `Pending`, it calls `Store::note_pending()`, which adds 1 to the counter of the nearest `<Suspense>` boundary (5.8). If there is no boundary, nothing happens.
- Child components are written as `let Poll::Ready(x) = use_future(..) else { return };`. This replaces React's throw; the boundary draws the fallback.

## 5. Runtime

### 5.1 Store

`Store` is a slab of slots. A slot holds `RefCell<Box<dyn Any>>` and `last_visited: u64` (pass number). It holds an Id -> slot map. The runner's `App` struct owns the store (it is not put in egui's `Context::data()`, for testability).

### 5.2 Sweep

At the end of the pass, list the slots whose `last_visited` is older than the current pass, run their cleanups, and drop them. This is unmount, and it also prevents state memory leaks. At sweep time every guard has dropped (they die with the component body). The same goes for `Handle`; `end_pass` takes `&mut Store`, so the type system forbids running the sweep while a `State` / `Handle` that borrows the store is alive. The context stack is cleared defensively in `begin_pass`.

### 5.3 Multi-pass

egui_taffy calls `request_discard` when layout changes, which runs a second pass within the same frame. egui's `Context::run` passes `new_input.take()` for each pass, and `RawInput::take` moves the events out with `events: core::mem::take(&mut self.events)`. **The second pass runs with empty events, so handlers fire only once** (confirmed in the egui source). No state rollback or journal is needed. Effects whose deps did not change do not re-run on the second pass. However, an effect whose deps derive from state that a handler changed in the first pass, and which is placed before the handler, does run on the second pass because the deps really changed. This is just "the one run that would have happened next frame" pulled forward into the second pass of the same frame; one deps change means one run (confirmed by the spike test). The runner sets `Options::max_passes = 3`. It is 3 rather than 2 because a `<View>` inside an egui container (`ScrollArea` etc.) inside a `<View>` becomes a separate egui_taffy tree, and the inner tree only learns the size the outer tree settled on in pass 2 during pass 3. If the pass budget runs out and the discard is refused, the runner calls `request_repaint` to converge on the next frame (otherwise it stays stuck on the old layout until the next input). Use `egui::Context::current_pass_index()` to check the pass count.

### 5.4 Flow of one frame

1. Input is received and the root `rsx!` runs.
2. Each component takes a guard from the store with `use_state` and draws its widgets.
3. When a click or similar happens, the handler runs on the spot and rewrites `State`.
4. When the component body exits, the guard drops (the value is written directly into the store, so there is no write-back).
5. At the end of the pass, the deferred queue (`defer`, `update_later`) is applied, the sweep drops unvisited Ids and runs cleanups, and finally the Id collision overlay is drawn.
6. If `request_discard` was called, a second pass runs with empty events.
7. The next frame starts drawing from the updated values.

### 5.5 Deferred queue

`cx.defer(f)` and `state.update_later(f)` / `handle.update_later(f)` are pushed onto a single queue in `Store` (`Vec<Box<dyn FnOnce(&Store)>>`) and applied until empty at the start of `end_pass`, before the sweep. Because it runs before the sweep, writes to a slot that unmounts in that pass still land (if the slot is already gone, they are silently dropped). `update_later` calls `request_repaint` when applied; `defer` does not, because it does not touch the store.

`Dispatch::send` does not go on this queue. Messages are applied the next time `use_reducer` is visited (section 4).

### 5.6 Repaint policy

- When `DerefMut` on `State` is called, mark it dirty and call `request_repaint` on guard Drop. The exception is `State::bind()`, which just hands out `&mut T` and does not mark dirty. Passing `&mut *state` to a bind-style widget like `TextEdit` would mark dirty every frame and the app would never go idle (egui_kittest's `Harness::run` panics with `ExceededMaxSteps`). The value changes only when there is input, and egui repaints on its own then, so nothing is missed.
- If applying the deferred queue changes state, `request_repaint`.
- Completion of `use_future` and `Dispatch::send` (from another thread) call `request_repaint`. Forget this and the screen does not change when an async result arrives until the mouse moves.
- The flip side: a component that rewrites state every pass requests a repaint every pass, and the app never goes idle (egui_kittest's `Harness::run` panics with `ExceededMaxSteps`). This is the same infinite loop as React's "setState during render"; avoid it except for animation.

### 5.7 One-frame delay

When a handler or effect rewrites state, widgets drawn earlier in the same component reflect it on the next frame. This is the normal nature of an egui app, and is called out here as the counterpart of React's "setState shows up on the next render".

### 5.8 Suspense

`<Suspense fallback={..}>children</Suspense>` (section 6, `egui-react-elements`) draws `fallback` instead of children if even one `use_future` inside is `Pending`. Rust has nothing like React's throw, so the child exits with `let Poll::Ready(x) = use_future(..) else { return };`, and the number of `Pending`s is counted with a counter in `Store`.

- **Counter stack** `Store` holds a `RefCell<Vec<usize>>` in the same shape as `provide_context`. `begin_suspense` pushes 0, `end_suspense` returns the pushed count (= the number of `use_future`s that were `Pending` inside), and `note_pending` adds 1 to the nearest (= innermost) counter. With nesting, the inner one consumes its own count, so the outer one does not count it. The stack is cleared in `begin_pass`. Only these 3 methods are added to core; the boundary itself lives in elements (an "element that wraps children", like `Collapsing`).
- **Initial state is suspended** The first time, draw offscreen, then switch to visible if there is no `Pending`. This is so that when `max_passes` runs out and `request_discard` is refused, what shows is `fallback` rather than half-drawn children.
- **Children are drawn while suspended too** They are drawn into an offscreen invisible `Ui` (`egui::Ui::new(ctx, id, UiBuilder::new().max_rect(fixed offscreen rect).invisible().sizing_pass())`). Hooks run, and futures start and complete. The rect is a fixed value so egui_taffy sees the same size every pass and does not issue useless `request_discard`s. `invisible()` disables both drawing and interaction, so the children's handlers do not fire offscreen. However, egui creates accessibility nodes for widgets regardless of visibility, so screen readers and `egui_kittest` see suspended children as "nodes at offscreen coordinates".
- **Children scope** Both suspended and visible paths use the `scope_id()` of `Suspense` itself (the third argument of `Cx::new(store, &mut ui, scope)`). The hook slots having the same Id on both paths is what preserves state and futures across the switch. `fallback` is drawn into the same `cx`, but the `rsx!` element Ids differ by line and column, so it does not collide with children.
- **The switch happens within the same frame** At the moment of the switch, flip the state with `Handle::set` and redo the same frame with `request_discard`. Neither half-drawn children nor a one-frame gap between fallback and children is visible. `Handle::set` calls `request_repaint`, but it is only called at the switch, so it does not repaint every frame while suspended. If `max_passes` (runner default 3) runs out and the discard is refused, the runner calls `request_repaint` and it settles on the next frame.
- **`shares_ui`** `Suspense` creates no `Ui` / leaf of its own; it streams children and fallback into the parent surface (Ui or Taffy) as-is. Placed inside a `<View>`, the children's `<View>` become children of the parent's taffy tree.
- **Differences from React** `use_effect` in suspended children runs (in React it does not, because nothing commits). There is no counterpart to React's `SuspenseList` / `useTransition`.

## 6. Layout

To make Flexbox / Grid a first-class citizen, egui_taffy is used (0.14, for egui 0.36 and taffy 0.9). As in React Native, "`<View>` is a taffy node, egui widgets are leaves".

```rust
<View direction="row" justify="space-between" align="center" gap={8} p={12}>
    <Text grow={1}>"Title"</Text>
    <Button onclick={|| *open = true}>"Open"</Button>
</View>
```

- `Cx` holds "are we inside a taffy container now" as `Surface` (3.1). `cx.leaf(&style, f)` is `tui.style(style.to_taffy()).ui(f)` inside, and goes to the bare `ui` outside. `cx.container(id, style, f)` adds a child node inside, and starts a new `egui_taffy::tui(..)` tree outside; in both cases `f` gets a Taffy-mode `Cx`.
- A `container` directly under Ui mode defaults to `reserve_available_width()`; only the runner's root uses `reserve_available_space()`. `grow` and `justify="space-between"` distribute leftover space, so they have no effect unless the `<View>` itself has a width (`w`). `reserve_available_space()` only tells egui_taffy "this much space is available"; the root node's own `size` stays `auto`. Without it, the root node shrinks to its content and two things break. (a) `<View grow={1.0} justify="center">` has no room to grow into and nothing to center against, and the app huddles in the top left. (b) Children that do not fit do not shrink. The parent grows to fit its content, so no overflow occurs, the premise for `flex-shrink` disappears, and rows spill past the right edge of the window. So the runner's root item style sets `w` to `100%` (the window width is fixed, so this is a definite value) and `min_h` to `100%` (vertically, grow if the content is taller) (section 7).
- egui's standard containers such as `<Vertical>` / `<Horizontal>` / `<Grid>` remain as leaves, as an escape hatch where performance matters. When called from Taffy mode, these egui-native containers behave as a single leaf, and the children inside are drawn in Ui mode. Only `ScrollArea` is placed with `leaf_fill` (3.1). It is a widget that fills the given space, so a leaf measured by content would lock it to the size of the first frame. Give a `ScrollArea` inside a `<View>` either `grow` or `h`. **The max-content of `leaf_fill` is exactly "the height of the current tree's root rect"** (egui_taffy's measure reads `infinite` that way), so inside a `<View direction="column">` with no definite height, neither `grow` nor `basis={0}` works, and the `ScrollArea` asks for "the whole window height" rather than "what is left after siblings". A screen with a `ScrollArea` under a toolbar becomes taller than the window by that much, and since the runner's root item style is `min_h: 100%` (section 7) with the height itself auto, the overflow is not clipped and extends downward. Either give it a definite height, or arrange things so nothing sits at the bottom edge that would get pushed out (`examples/board` puts the column's `+ card` at the end inside the `ScrollArea`. `docs/tasks/board/plan.md` 8.3).
- Every leaf that carries text sets wrap to `Extend` (`Text` `Label` `Button` `Checkbox` `Slider` `ComboBox` labels, the `Collapsing` header). egui_taffy measures a leaf as "the size it was drawn at last time" and returns that single value to taffy as both min-content and max-content. The first draw happens in a `Ui` of width 0, so a wrapping widget reports "one character wide" there, the node is locked at that narrow width, and the label stacks one character per line. Leaves with `grow` or `w` are unaffected because taffy decides their width. Widgets with a `wrap_mode` builder (`Button` / `Label`) use it; those without (`Checkbox` / `Slider` / `ComboBox` / `CollapsingHeader`) set it on the leaf `Ui`'s `style.wrap_mode`. `Collapsing` restores the original value before entering the body (what the children draw is the caller's business).
- Both `<Text>` and `<Label>` can switch to egui's default wrapping with the `wrap` attribute. Wrapping needs a width, so use it together with `w` or (inside a container with a definite width) `grow`. The only difference between the two is that `Text` has `size` / `color` / `strong`.
- The root panel is wrapped by default in a `<View>` with `direction="column"`.

### Elements list (`egui-react-elements`)

All elements are written with `#[component]` and take `#[prop(default)] style: ItemStyle`. `rsx!` packs the layout attributes into `style`. Everything, including the event enums, is re-exported from `egui_react_elements::prelude` (as in 3.6, the enum name is needed wherever `on_*` is used).

| Kind | Elements |
|---|---|
| Layout | `View` (`display` / `direction` / `wrap` / `justify` / `align` / `align_content` / `gap` / `cols`), `Text` (`size` / `color` / `strong` / `wrap`) |
| Widgets | `Button` (`enabled` / `label`, `on_click`), `Label` (`wrap`), `TextEdit` (`bind` / `multiline` / `hint` / `desired_width` / `rows`, `on_change` / `on_submit`), `Checkbox` (`bind` / `label`, `on_change`), `Slider<T: Numeric>` (`bind` / `range` / `label`, `on_change`), `ComboBox` (`bind` / `options` / `label`, `on_change`), `Image` (`source` / `fit` / `alt`), `Separator` (`vertical`) |
| Containers | `ScrollArea`, `VirtualList` (`rows` / `row_h` / `render`), `Collapsing`, `Frame`, `Window` (`title` / `open` / `resizable` / `default_pos` / `default_size`), `Panel` (`side`), `CentralPanel`, `Vertical`, `Horizontal`, `Grid` + `row()` |
| Drawing | `Canvas` (`sense` / `paint`, `on_drag` / `on_hover`. A leaf that passes on the rect taffy gave it as-is) |
| Async | `Suspense` (`fallback: impl View`, `shares_ui`. Draws `fallback` instead of children if even one `use_future` inside is `Pending`. 5.8) |

`label` on `Button` and `alt` on `Image` are the names assistive technology reads. `label` on `Button` does not change what is drawn (children); it only replaces the name of the accesskit node (`Context::accesskit_node_builder`). A button that is only an icon or `"x"` would otherwise be read as just that text, so pass it. `alt` on `Image` goes to `egui::Image::alt_text`, and is also drawn next to the warning sign when loading fails. Reading aloud on web is waiting on upstream, as in the non-goals in section 1, but these work on native from today.

Inside taffy (`Cx::in_taffy()`), `TextEdit` fills its node. Single-line sets `desired_width` to the node width, and `multiline` fills both ways with `ui.add_sized(ui.available_size(), ..)` (`desired_rows` only fits in whole rows, and the remainder spills out of the node). This is because drawing at egui's default 280pt / 4 rows inside a node widened with `grow` or `w` leaves the rest empty. If `desired_width` / `rows` is given explicitly, that wins. `Slider` / `ComboBox` / `Button` do not stretch for now (they stay at `spacing.slider_width` / `spacing.combo_width` / content width respectively).

Elements with `bind` have the widget write directly into state, so they go through `State::bind()`. Unlike `&mut *state`, this does not mark the state dirty (5.6). Passing a handler that touches the same state to the same element gives E0502, so `on_change` on a `bind` element is limited to notifying somewhere else, such as logging or `Dispatch`.

Among egui's standard containers, those that carve space out of the parent (`Panel` / `CentralPanel`) and those that depend on the parent `Ui` (`Grid` row breaks) are affected by `rsx!` creating a child `Ui` with `Ui::push_id` per element. Row breaks are provided as `{row()}` rather than an element (`{expr}` nodes are not scoped). These get `#[component(shares_ui)]` and inherit the parent surface as-is.

**A panel carves its space out of "the nearest egui `Ui`", that is, the `Ui` that started the current taffy tree.** It skips past however many `<View>`s sit in between. In taffy mode, `Panel` / `CentralPanel` do not use `cx.leaf`; they `show_inside` against `cx.ui()`. If a leaf were created, the panel would carve out the inside of its own small private node, and 4 panels placed as siblings would all pile up in the same corner. Under the runner, this `Ui` is the window itself, so "panels are used at the app root" holds automatically. The flip side is that a `<Panel>` written deep inside a `<View>` is not part of that row; it flies to the window edge. That is what docking means, not a bug (`examples/shell`). `ScrollArea` draws all of its children as-is. For long lists use `VirtualList`. It takes `rows` and `row_h` and calls `render(cx, i)` only for "visible rows" (internally `egui::ScrollArea::show_rows`). Rows are drawn inside `cx.scope(i, ..)`, so each row can have hooks, same as `for` + `key={i}`. The condition is that all rows have the same height. The bound on `render` is written explicitly as `impl for<'a, 's, 'u> FnMut(&'a mut Cx<'s, 'u>, usize)`. `#[component]` rewrites elided lifetimes on props to those of the props struct, so the elided form (`impl FnMut(&mut Cx, usize)`) does not compile.

`Canvas` is a leaf that draws nothing itself. With `leaf_fill` it reserves the size taffy gave it via `ui.allocate_exact_size(ui.available_size(), sense)` and just calls `paint(ui, rect)`; the caller pushes the content onto `ui.painter()` (lines and shapes, or `egui_wgpu::Callback::new_paint_callback(rect, ..)`). elements does not depend on egui-wgpu. `sense` defaults to `Sense::hover()`; `on_hover` (pointer position within the rect) works as-is, and `on_drag` (`drag_delta`) needs `Sense::drag()`. The closure prop is named `paint`, not `on_paint`. `rsx!` treats every attribute starting with `on_` as an event enum variant, so an ordinary prop cannot start with `on_` (same naming as `render` on `VirtualList`). The bound is written explicitly as `impl for<'a> FnOnce(&'a mut egui::Ui, egui::Rect)` (same reason as `VirtualList`).

`Suspense` is `shares_ui` for the same reason. It draws nothing itself and streams children and `fallback` into the parent as-is, so placed inside a `<View>`, the children become children of the parent's taffy tree.

A generic element like `<Provide value={handle}>` cannot be provided for the context provider. The `Handle<'s, T>` that `provide_context` takes borrows the store, while `props_builder` requires the component to be `for<'a, 's, 'u> Fn(&'a mut Cx<'s, 'u>, P)`, so the props type `P` cannot name `'s` (trying gives `implementation of Fn is not general enough`). For the same reason you cannot provide inside a `View` closure either (`View::show` is also higher-ranked over `'s`). The form that works is "a provider component that creates the value itself and hands it out itself": inside `#[component(shares_ui)] fn Themed(cx, children: impl View)`, call `use_handle` and then `provide_context(cx, handle, |cx| children.show(cx))` (`examples/theme`). This is the same shape as a React provider that owns state, so it is not a problem in practice.

### Layout attributes

They live in `egui_react::layout`. taffy types come from `egui_taffy::taffy`, re-exported as `egui_react::taffy`.

- `Length`: `Px(f32)` / `Percent(f32)` (a 0.0 to 1.0 fraction, as in taffy) / `Auto`. `From<f32>` and `From<i32>` give `Px`; `From<&str>` parses `"auto"` / `"50%"` / `"12px"` / `"12"` and panics on anything else.
- `ItemStyle`: the item-side attributes every element accepts. `w h min_w min_h max_w max_h grow shrink basis align_self m mx my mt mr mb ml p px py pt pr pb pl col_span row_span`. Setters take `impl Into<Length>`, so `rsx!` can pass number literals and string literals as-is. The `m` / `p` shorthands go "all -> `x` / `y` -> each side", and the more specific one wins. `to_taffy()` turns it into a `taffy::Style`.
- Shorthand attributes and `style={expr}` fill the same `style` prop, so `rsx!` collapses them into one `.style(..)`. If both are present, the shorthand attributes chain off the `style=` expression (`<Chip style={style} p={6}/>` becomes `.style((style).p(6))`). This lets a wrapper component that takes `style: ItemStyle` receive the caller's layout as-is and add its own.
- `ContainerStyle`: the parent-side attributes `<View>` accepts. `display direction wrap justify align align_content gap cols`. `merge(&ItemStyle)` combines it with the item side into one `taffy::Style` (a taffy node keeps its own item attributes and the container attributes for its children in a single `Style`). `cols` becomes equal-width columns only when `display="grid"`.
- `Direction` / `Justify` / `Align` (= `AlignSelf`) / `Display` are enums, and `From<&str>` parses the CSS spelling (`"row"`, `"space-between"`, `"center"`, `"grid"` etc.). An invalid string panics with a list of candidates. `Justify` and `Align` have the default `Normal`, which means "unspecified" and becomes `None` on the taffy side. Implementing `From<&str>` for `Option<Justify>` is impossible under the orphan rule, so "unspecified" is a `Normal` variant instead of an Option.

Rejected alternative: egui_flex. It stops at egui 0.35, has no `justify-content`, and its own README says `flex-shrink` is structurally impossible. Not enough for a first-class citizen.

## 7. Crate layout

```
egui-react/            core: View, Cx, Store, State, Handle, Dispatch, hooks, sweep, deferred queue, persistence
egui-react-macros/     rsx! (based on rstml 0.13), #[component], #[hook]
egui-react-elements/   wrappers for egui widgets / containers. View / Text sit on taffy
egui-react-app/        run(Options, |_cx| rsx!{ <App/> }). Wraps eframe and absorbs native / wasm / Android. The iOS runner also lives here
examples/              counter, todo (use_reducer + use_persisted), layout, fetch (use_future + Suspense + ehttp), later mobile
```

What `egui_react_app::run(Options, root)` does in one frame is the following.

1. Wrap in `CentralPanel` (the root `Ui` eframe hands over has no margin and no background, and text is unreadable in light mode).
2. `store.begin_pass(ctx)`.
3. Create the root `Cx`, and `show` the `View` returned by `root(cx)` inside `cx.root_container(..)` (`direction: column`, `w` is `100%`, `min_h` is `100%`, `reserve_available_space`). Nested containers reserve only width, so only the root also takes height. This id and style are public as `egui_react_app::root_id()` / `root_style()`, so tests and custom runners can reproduce the same frame.
4. `store.end_pass()`.

`Options` has `title` / `max_passes` (default 3, set explicitly with `ctx.options_mut`. See 5.3) / `persist` / `canvas_id` (wasm) / `native` (native only) / `setup`. `setup: Option<Box<dyn FnOnce(&eframe::CreationContext)>>` is a hole called exactly once right after eframe has set up the window and render backend; it runs at the top of `ReactApp::new`. This is where you create wgpu pipelines and put them in `renderer.write().callback_resources` of `cc.wgpu_render_state` (the same shape as `custom3d_wgpu` in the official egui demo). Handing out `RenderState` through hooks or context is not adopted. The line drawn is: wgpu types never appear anywhere in the hook API, and apps that do not use wgpu never see it. It exists on both native and wasm. `App::save` writes `store.save_persisted()` to the `"egui_react"` key in `Storage`, and `load_persisted` reads from `CreationContext::storage`. On wasm, `cfg(target_arch = "wasm32")` puts `WebRunner` on `wasm_bindgen_futures::spawn_local`, and the canvas is looked up by `canvas_id`.

`root` is called every pass, and the `View` it returns cannot borrow anything created inside `root` (an `rsx!` that borrows a hook guard would return a value that borrows a local). Put hooks in components and write the root as `|_cx| rsx!{ <App/> }`.

egui is pinned to the 0.36 series. Since egui 0.35, `eframe::App::ui` takes `&mut Ui`, so the runner wraps that directly in `Cx`. `egui-react` (core) has `egui_taffy` as a normal dependency. This is because `Surface` in `Cx` must know `Tui`, and it builds as-is on the wasm target too. In addition, as the future execution mechanism it has `pollster` on native (`cfg(not(target_arch = "wasm32"))`) and `wasm-bindgen-futures` on wasm (`cfg(target_arch = "wasm32")`).

## 8. Platforms

- native / wasm / Android: eframe. wasm is built with trunk. The render backend is wgpu. **The default features of eframe 0.36 include `wgpu` and not `glow`** (the reverse of 0.35 and earlier), so it renders with wgpu with no extra work. Since glow is now the opt-in, there is no feature for choosing. On web it falls back to WebGL for browsers without WebGPU. This propagates as `eframe/wgpu` -> `egui-wgpu/default` -> `wgpu/webgl`, so we do not need a direct dependency on `wgpu` ourselves.
- iOS: eframe does not support it (emilk/egui#3117 is open). A thin `egui-winit` + `egui-wgpu` runner is written inside `egui-react-app`. Built with cargo-mobile2.
- Accessibility: on native, eframe (egui-winit) calls `Context::enable_accesskit`, and the widget tree reaches the OS accessibility API. **On web it does not.** eframe's web runner throws away the per-frame `TreeUpdate` with `accesskit_update: _, // not currently implemented` (`eframe/src/web/app_runner.rs`), and there is no upstream adapter that mirrors the canvas contents into the DOM. The policy and prototype are in `docs/tasks/a11y/`.
- The library itself only touches `&mut egui::Ui`, so platform support is confined to the runner layer and touch / IME tuning.
- The async execution mechanism is confined to `task::spawn` in core (section 4, "`use_future` details"). iOS / Android use the same thread path as native.

## 9. Testing

- Interaction tests and snapshot tests for components with `egui_kittest`.
- `trybuild` pins the wording of macro compile errors (wrong event names, missing `key`, etc.).
- examples are libs too, so they have kittest. For examples that have a raw egui version, run the same interactions against both and check they give the same result.
- Visual parity is captured with the `snapshot` feature of `examples/gallery`. Draw the egui-react version and the raw egui version at the same size and compare against **the same single image**. The only remaining difference is text edges, absorbed with `SnapshotOptions::max_failed_pixels` (the value is in tasks/examples/plan.md section 7).
- Snapshot tests behind a feature do not run in CI, so retake them by hand after changing anything that feature touches.
- kittest is used from the spike stage on, and a test pins down that handlers fire only once across multiple passes.

## 10. Items verified in the spike

The following were confirmed with hand-written expansions without the macro (PR1, `crates/egui-react/tests/`). Every item has a test and is green. Where an assumption broke, the relevant section of this document has been updated.

- A `State` guard lives only for the component body, and handlers on sibling elements can borrow the same state with `&mut` in turn.
- A guard can be returned through `#[hook]` (the lifetime `'s` is independent of `&mut Cx`).
- The fused closure expansion compiles, and the `Handler` trait can call `FnMut()` / `FnMut(A)` uniformly.
- The `Handle` from `use_context` and the parent's guard can coexist.
- On egui_taffy's second pass, the click handler fires only once and the effect does not re-run.
- Id collision detection catches a missing `#[hook]` and a missing `key`.
- The sweep drops the state of a component removed by `if` and runs its cleanup.
- A new `Cx` can be created inside an egui container closure (`ui.vertical`), and hooks work there.

## 11. Decision log

| Decision | Adopted | Rejected | Reason |
|---|---|---|---|
| Host language | Rust only | JS React + JS runtime | Several times the size, iOS JIT limits, a wasm-bindgen layer |
| Tree | Direct expansion | Retained VNode + traversal | Handlers become `'static` + `Rc` and the Yew pain returns |
| hook Id | Call site stack + collision detection | Mix in occurrence count | The latter silently shifts state and cannot be detected |
| State | guard (`Deref/DerefMut`) + helper `Handle` | Cell-style `Handle` only | Keeps `*count += 1`. Conflicts disappear with the fused closure |
| callback props | Fuse `on_*` into an enum | Return events as return value / hand-written single `callback` | Borrowed payloads, multiple events, verbosity |
| effect timing | Immediately at the call site | After commit | egui has no commit, and deferring invites `'static` |
| deps comparison | Hash | PartialEq + Clone | To allow borrowed deps |
| Layout | egui_taffy | egui_flex | No justify / shrink, slow to track egui |
| Multi-pass handling | Not needed | Snapshot rollback | Confirmed egui empties events on the second pass |
| Where the store lives | The runner's `App` | `Context::data()` | Testability |
| `Handler` implementation | Two blanket impls coexist via marker type argument | Pick `call0` / `call1` by argument count | The marker is always inferred, and the macro emits only one form |
| Borrow conflict between value prop and handler | User clones or uses `update_later` | `rsx!` clones implicitly | Cannot insert clones without type info, and zero-copy should be the default |
| future executor | One thread + `pollster` | Require tokio | Small dependency, enough for futures that just wait. Apps that need tokio can use `Handle::current()` inside the future |
| Representation of async results | `std::task::Poll<T>` | Custom `Loading` / `Ready` / `Error` enum | It is in std, and errors can be `T = Result<..>`. Does not add more state kinds |
| Suspense implementation | Offscreen drawing + counter + `request_discard` | Rollback via panic / `catch_unwind` | Rust has no cheap rollback. The child exits with a one-line let-else |
