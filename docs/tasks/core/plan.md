# Plan: core

> `egui_taffy` below is historical. It was replaced in 2026-09 by egui-reactor's
> own layout engine over taffy (`crates/egui-reactor/src/engine.rs`, ARCHITECTURE
> section 6), which ports its measure function and node rules, so the layout
> behaviour described here still holds unless ARCHITECTURE says otherwise.

The task definition is in [task.md](task.md). The design rationale is in [docs/ARCHITECTURE.md](../../ARCHITECTURE.md). This document sets the implementation steps and how to verify them. If you decide to deviate from it during implementation, update this document, and update ARCHITECTURE.md too if the change matters to the design.

## 0. Overview

Work through the 4 phases in order. Each phase closes with "implement, test, update ARCHITECTURE.md, commit", and the next phase builds on the previous phase's API.

| Phase | Main targets | Crates touched |
|---|---|---|
| 2 | `View`, the rest of the hooks, the deferred queue, the layout context in `Cx`, the collision overlay | `egui-reactor` |
| 3 | `#[component]` / `#[hook]` / `rsx!`, trybuild, replacing spike with the macro versions | `egui-reactor-macros`, `egui-reactor` (re-exports and tests) |
| 4 | `View` / `Text`, widgets, containers, snapshots | `egui-reactor-elements` |
| 5 | `run`, `use_persisted`, wasm, examples, CI | `egui-reactor-app`, `examples/*`, `egui-reactor` (persist) |

Dependencies to add (pin them in `[workspace.dependencies]`).

| crate | Purpose | Where |
|---|---|---|
| egui_taffy 0.14 (promote from dev to normal dependency) | `Tui` mode of `Cx` | egui-reactor |
| serde / serde_json | `use_persisted` | egui-reactor |
| typed-builder | Props builder | egui-reactor (re-exported under `__private`) |
| syn 2 (`full`, `extra-traits`) / quote / proc-macro2 | macros | egui-reactor-macros |
| trybuild | pin compile errors | egui-reactor (dev) |
| egui_kittest `snapshot` + `wgpu` | snapshots | egui-reactor-elements (dev, behind feature `snapshot`) |
| eframe `persistence` feature | `Storage` | egui-reactor-app |
| wasm-bindgen-futures / web-sys (`Document`, `HtmlCanvasElement`) | wasm runner | egui-reactor-app (`cfg(target_arch = "wasm32")`) |

`egui-reactor` has `egui-reactor-macros` as a normal dependency and re-exports `rsx!` / `component` / `hook`. Users only `use` `egui_reactor::prelude::*`. The trybuild tests for the macros live in `tests/ui/` on the `egui-reactor` side (this avoids a dev-dependency cycle from the proc-macro crate to the facade).

## 1. Phase 2: core hooks (`crates/egui-reactor/src/`)

### 1.1 `view.rs`

```rust
pub trait View {
    fn show(self, cx: &mut Cx<'_, '_>);
}
impl View for ()                                   // does nothing
impl View for &str / String                        // cx.leaf(default, |ui| ui.label(..))
impl<V: View> View for Option<V>
impl<V: View> View for Vec<V>
impl<V: View, const N: usize> View for [V; N]
impl<F: FnOnce(&mut Cx<'_, '_>)> View for F        // escape hatch

/// What `rsx!` expands to. A helper so closure type inference works
pub fn view<F: FnOnce(&mut Cx<'_, '_>)>(f: F) -> impl View { f }
```

The blanket impl for `IntoIterator<Item = V>` in ARCHITECTURE.md 3.2 conflicts under coherence with the `FnOnce` blanket impl and with `Option<V>` (confirmed with rustc). Replace it with separate impls for `Option` / `Vec` / arrays. Loops inside `rsx!` can be written with `for`, so there is no practical difference. Update ARCHITECTURE.md 3.2.

Passing `{ |cx| .. }` straight to an `impl View` argument sometimes fails to infer the closure's argument type, so `rsx!` always emits `::egui_reactor::view(|cx| { .. })`. Point users to `view(|cx| ..)` for the escape hatch too.

### 1.2 `use_memo`

```rust
#[track_caller]
pub fn use_memo<'s, D: Hash, T: 'static>(cx: &mut Cx<'s, '_>, deps: D, f: impl FnOnce() -> T) -> &'s T;
```

To return `&'s T`, keep the value outside the `RefCell`. Add `memo: elsa::FrozenVec<Box<dyn Any>>` to `Slot`. When the deps hash changes, push a new value with `push_get` and return `&'s T`. Do not delete old values, since `&'s T` handed out during the pass may still point at them; drop all but the last one in `end_pass(&mut self)` (take `&mut Vec` with `as_mut()`). Share the deps hash `deps_hash` with `use_effect`. Always compute on the first visit.

### 1.3 `use_reducer` and `Dispatch` (`dispatch.rs`)

```rust
#[track_caller]
pub fn use_reducer<'s, S: 'static, M: Send + 'static>(
    cx: &mut Cx<'s, '_>,
    reducer: impl FnMut(&mut S, M),
    init: impl FnOnce() -> S,
) -> (State<'s, S>, Dispatch<M>);

pub struct Dispatch<M> { queue: Arc<Mutex<Vec<M>>>, ctx: egui::Context }   // Clone. Send + Sync if M: Send
impl<M> Dispatch<M> { pub fn send(&self, msg: M); }   // push, then request_repaint
```

The slot value is `(S, Arc<Mutex<Vec<M>>>)`. Each time the hook is visited, `take` the queue, apply the reducer to each message in order, then return the `State` guard and the `Dispatch`.

ARCHITECTURE.md 5.5 says "apply the reducer at the end of the pass", but change this to apply on visit. Two reasons. (a) Applying at the end of the pass means storing the reducer, which makes it `'static`; applying on visit lets the reducer be a normal closure. (b) With end-of-pass application, a message from another thread goes "the next pass's body sees the old state, it is applied at the end of that pass, then shown in the frame after", which is one frame of extra delay. With on-visit application, the body of the next frame (triggered by `request_repaint` in `send`) sees the new state. When `send` is called from a handler, what the user sees (applied in the next frame) is the same as writing to `State`, so nothing changes. Update ARCHITECTURE.md 4 and 5.5.

### 1.4 Deferred queue: `defer` and `update_later`

```rust
// Store
deferred: RefCell<Vec<Box<dyn FnOnce(&Store)>>>,
pub(crate) fn defer_raw(&self, f: Box<dyn FnOnce(&Store)>);

// Cx
pub fn defer(&self, f: impl FnOnce() + 'static);

// Both State<'s, T> and Handle<'s, T>
pub fn update_later(&self, f: impl FnOnce(&mut T) + 'static);
```

`update_later` queues a closure that captured the slot Id. When applied, look up the slot with `slot_by_id`, `borrow_mut` it, call `f`, and `request_repaint`. If the slot is gone (unmounted in that pass), drop it silently. `State::update_later` can be called while the guard is alive (it is applied at the end of the pass, long after the guard is dropped).

Order in `end_pass`: apply the deferred queue until it is empty, then sweep, then the collision overlay (1.6). The public API cannot push new items while the queue is being applied (`FnOnce()` cannot touch `Store`).

Because the closure is `'static`, using a loop variable needs `move`: `todos.update_later(move |t| t.remove(i))`. Add this to ARCHITECTURE.md 3.7 and the table in 4.

### 1.5 repaint

`Dispatch::send` and applying `update_later` always `request_repaint`. `defer` does not (it does not touch state). Add cases to the tests in `repaint.rs`.

### 1.6 Collision overlay

Add `warn_on_collision: bool` (default `cfg!(debug_assertions)`) and `set_warn_on_collision` to `Store`. At the end of `end_pass`, if enabled and `collisions` is not empty, draw red text with `Frame::popup` in `egui::Area::new(Id::new("egui_reactor_collision_warning")).order(Order::Debug).anchor(Align2::LEFT_TOP, (8.0, 8.0))`. The text is `egui-reactor: hook id collision at {file}:{line}:{column}. Wrap custom hooks in #[hook], or add key= inside loops.`, with the same location collapsed to 1 line per pass. `end_pass` is called inside the runner's `App::ui`, so it can draw on that frame's `Context`.

### 1.7 Layout context in `Cx`

```rust
pub struct Cx<'s, 'u> {
    pub store: &'s Store,
    surface: Surface<'u>,
    scope: egui::Id,
}
enum Surface<'u> {
    Ui(&'u mut egui::Ui),
    Taffy(&'u mut egui_taffy::Tui),
}
impl<'s, 'u> Cx<'s, 'u> {
    pub fn new(store: &'s Store, ui: &'u mut egui::Ui, scope: Id) -> Self;
    pub fn new_taffy(store: &'s Store, tui: &'u mut egui_taffy::Tui, scope: Id) -> Self;
    pub fn ui(&mut self) -> &mut egui::Ui;          // Ui: as-is. Taffy: tui.egui_ui_mut() (not placed by taffy. elements do not use it)
    pub fn in_taffy(&self) -> bool;
    pub fn ctx(&self) -> &egui::Context;
    pub fn scope_id(&self) -> Id;
    pub fn scope<R>(&mut self, source: impl Hash + Debug, f: impl FnOnce(&mut Cx<'s, '_>) -> R) -> R;   // Ui: ui.push_id. Taffy: tui.with_auto_id_prefix(id, ..)
    pub fn hook_scope<R>(..) -> R;                                                                     // unchanged
    pub fn defer(&self, f: impl FnOnce() + 'static);
    /// leaf: draw one egui widget. Inside Taffy: tui.style(style.to_taffy()).ui(f). Outside: f(ui)
    pub fn leaf<R>(&mut self, style: &ItemStyle, f: impl FnOnce(&mut egui::Ui) -> R) -> R;
    /// container: create a taffy node and draw inside it with a Taffy-mode Cx. Outside: egui_taffy::tui(ui, id).style(..).show. Inside: tui.style(..).add
    pub fn container<R>(&mut self, id: Id, style: taffy::Style, f: impl FnOnce(&mut Cx<'s, '_>) -> R) -> R;
}
```

The `cx.ui` field becomes the `cx.ui()` method. Rewrite spike's tests and examples at this point (a stopgap until Phase 3 replaces them with the macro versions). The way to rebuild a `Cx` inside an egui container closure does not change (`let (store, scope) = (cx.store, cx.scope_id()); cx.ui().vertical(|ui| { let mut cx = Cx::new(store, ui, scope); .. })`). The Ui-mode side of `container` defaults to `reserve_available_width()`; only the runner's root uses `reserve_available_space()` (4.1). Update ARCHITECTURE.md 3.1 and 6.

### 1.8 `layout.rs`

```rust
pub enum Length { Px(f32), Percent(f32), Auto }      // From<f32> / From<i32> give Px. From<&str> parses "50%" / "auto" / "12px" / "12" (invalid strings panic)
#[derive(Default, Clone)]
pub struct ItemStyle { w, h, min_w, min_h, max_w, max_h: Option<Length>, grow, shrink: Option<f32>, basis: Option<Length>, align_self: Option<AlignSelf>, m, mx, my, mt, mr, mb, ml, p, px, py, pt, pr, pb, pl: Option<Length> }
impl ItemStyle { pub fn w(self, v: impl Into<Length>) -> Self; .. /* a setter per field */; pub fn to_taffy(&self) -> taffy::Style; }
#[derive(Default, Clone)]
pub struct ContainerStyle { display: Display(Flex | Grid | Block | None), direction: Direction, wrap: bool, justify: Justify, align: Align, align_content: Option<Align>, gap: (f32, f32), cols: Option<u16> /* number of equal-width grid columns */ }
impl ContainerStyle { pub fn merge(&self, item: &ItemStyle) -> taffy::Style; }
```

`Direction` / `Justify` / `Align` / `AlignSelf` / `Display` are enums that implement `From<&str>` (`"row"`, `"space-between"`, `"center"`, and so on; invalid strings panic with a location). This lets `rsx!` pass string literals straight to the setters. For the `m` / `p` shorthands, the more specific setting wins, in the order `mx` then `ml` + `mr`. Use the taffy types by re-exporting `egui_taffy::taffy`.

### 1.9 Tests (Phase 2)

| # | Test file | What it checks |
|---|---|---|
| 2-1 | `view.rs` | Each of `()` / `&str` / `String` / `Option` / `Vec` / array / closure draws. `view(|cx| ..)` compiles without a type annotation on `cx` |
| 2-2 | `memo.rs` | `f` does not rerun in a frame where deps are the same, and reruns when they change. The same `&T` can be read in 2 places in one frame. Dropped on unmount |
| 2-3 | `reducer.rs` | state changes in the frame after `send` from a handler. Pass `Dispatch` to `std::thread::spawn` and `send`; `ctx.has_requested_repaint()` is true and the change shows in the next frame. No double application in the 2nd pass (reuse the `multi_pass` steps) |
| 2-4 | `deferred.rs` | Read `todos` in a `for` loop while a handler inside the loop calls `update_later(move |t| t.remove(i))`; the element count drops in the next frame. Later widgets in the same frame see the old value (5.7). `defer` runs exactly once at the end of the pass. `update_later` on an unmounted slot does not panic |
| 2-5 | `repaint.rs` (additions) | repaint is requested only in the frames with `send` / `update_later` |
| 2-6 | `collision.rs` (additions) | In a frame with a collision, `get_by_label` finds the overlay text. It goes away with `set_warn_on_collision(false)` |
| 2-7 | `taffy_cx.rs` | Draw 3 `cx.leaf`s inside `cx.container`; x increases monotonically with `direction="row"` and y with `"column"` (check with kittest's `Node::rect()`). Hooks work inside nested `container`s. `cx.scope` separates hook Ids and egui Ids in Taffy mode too (draw the same function twice wrapped in `scope`; state is independent and egui-side state such as `Collapsing` is also independent) |

The shape of kittest's `Harness::new_ui_state` and `run_app` is the same as in spike. `run_app` in `tests/common/mod.rs` keeps using `Cx::new`.

## 2. Phase 3: macros (`crates/egui-reactor-macros/src/`)

### 2.1 `#[component]` (`component.rs`)

The input is `fn Name<generics>(cx: &mut Cx, <props>..)`. The return type must be `()` (anything else is an error). The first argument must be `&mut Cx`, or it is an error.

How arguments are handled.

| Shape | Generated |
|---|---|
| `x: T` | Required prop. Rewrite the elided lifetime of `&T` to `'e` (a `Type::Reference` with `lifetime: None`). Elisions inside a path, such as `Cow<str>`, must be written out by the user |
| `x: Option<T>` | Optional prop (`#[builder(default)]`) |
| `#[prop(default)] x: T` / `#[prop(default = expr)] x: T` | Optional prop |
| `#[prop(into)] x: T` | `#[builder(setter(into))]` |
| `children: C` where `C: View`, or `children: impl View` | `impl` desugars to a generic `C: View`. `rsx!` passes `.children(())` even when there are no children (2.3), so it is optional for any type that accepts `()` |
| no `children` declared | Generate `children: ()` with `#[builder(default)]`, because `rsx!` always calls `.children(..)` |
| `#[event] on_x: A` | Add `X(A)` to the event enum. In the body, `on_x: Emitter<'_, '_, NameEvent, A>` |

Generated output.

```rust
pub enum NameEvent { X(A), .. }                                  // only when there is at least one #[event]
#[derive(::egui_reactor::__private::TypedBuilder)]
#[builder(crate_module_path = ::egui_reactor::__private::typed_builder)]
pub struct NameProps<'e, C: View, ..generics> {
    pub x: T,
    #[builder(default)] pub y: Option<U>,
    #[builder(default)] pub events: Option<&'e mut dyn FnMut(NameEvent)>,   // only when there is an #[event]
    pub children: C,
}
#[allow(non_snake_case)]
pub fn Name<'e, C: View, ..>(cx: &mut Cx<'_, '_>, props: NameProps<'e, C, ..>) {
    let NameProps { x, y, events, children } = props;
    let mut __noop = |_: NameEvent| {};
    let __events: &mut dyn FnMut(NameEvent) = match events { Some(e) => e, None => &mut __noop };   // the match arms shorten the lifetime
    let __sink = ::egui_reactor::EventSink::new(__events);
    let on_x = ::egui_reactor::Emitter::new(&__sink, NameEvent::X);
    { /* body. The tail expression is rewritten to ::egui_reactor::View::show(tail, cx) */ }
}
impl<'e, C: View, ..> ::egui_reactor::__private::Props for NameProps<'e, C, ..> {   // for props_builder in 2.3
    type Builder = NamePropsBuilder<'e, C, ..>;
    fn builder() -> Self::Builder { Self::builder() }
}
```

To rewrite the tail expression, treat the body as a `syn::Block` and replace the last `Stmt::Expr(expr, None)`. A body that ends with `;` is left alone. A body that returns a different `rsx!` from each arm of `if` / `match` does not compile because the closure types differ, so point users to the `rsx!{ if .. }` form (pin the message with trybuild). Wrapping `{ body }` as `View::show({ body }, cx)` does not compile either, since it returns a value while a guard still borrows a local inside the block. Always replace only the tail expression.

`Emitter` extends spike's `Emitter<'a, 'e, E>` with a payload type `A` and a variant constructor `fn(A) -> E`. `on_x.emit(a)` becomes `sink(NameEvent::X(a))`. Update ARCHITECTURE.md 3.6.

### 2.2 `#[hook]` (`hook.rs`)

Add `#[track_caller]` and wrap the body in `cx.hook_scope(::std::panic::Location::caller(), |cx| { body })`. `cx` is "the first argument of type `&mut Cx`"; error if there is none. The return type and generics stay as they are. A `return` inside the body returns from the closure, but the closure's return value becomes the hook's return value, so the meaning does not change.

### 2.3 `rsx!` (`rsx/`)

#### Parsing

Parse into `Vec<Node<ControlFlow>>` with `rstml::ParserConfig::new().custom_node::<ControlFlow>()`. `ControlFlow` has 3 kinds, `if` / `for` / `match`, and `peek_element` looks at the leading ident. The body (`{ .. }`) is parsed into child nodes by repeating `RecoverableContext::parse_recoverable::<Node<ControlFlow>>` until `}`.

- `if cond { nodes } else if cond { nodes } else { nodes }`: `cond` is `syn::Expr::parse_without_eager_brace`.
- `for pat in expr { nodes }`: `expr` is parsed the same way.
- `match expr { pat (if guard)? => { nodes } | <Elem/> , .. }`: the right side of an arm is one element or `{ nodes }`.

Node kinds and how they are handled.

| Node | Handling |
|---|---|
| `<Name attrs>children</Name>` / `<Name attrs/>` | Component call. `Name` is a Rust path (`elements::Button` is fine too) |
| `"literal"` | String literal. `show` it as a `View` |
| Unquoted text | Error: `text must be a string literal: "..."` |
| `{expr}` | `::egui_reactor::View::show(expr, cx);` |
| `<> .. </>` | Expand the children in order |
| `<!-- -->` | Ignored |
| `if` / `for` / `match` | Emit the Rust control flow as-is and expand the body |

#### Attributes

| Shape | Handling |
|---|---|
| `key={expr}` | Mixed into the scope Id. At most 1 per element |
| `on_x={expr}` | An arm of the fused closure: `NameEvent::X(a) => ::egui_reactor::Handler::call(expr, a)` |
| `events={expr}` | Pass `expr` to `events` instead of the fused closure. Error if used together with `on_*` |
| Layout attributes (`w h min_w min_h max_w max_h grow shrink basis align_self m mx my mt mr mb ml p px py pt pr pb pl`) | Collected into one call: `.style(::egui_reactor::layout::ItemStyle::default().w(..).grow(..))`. Not called if there are none |
| Other `name={expr}` / `name="lit"` / `name` (bool true) | The builder setter `.name(expr)` |

`on_x` to `X`: strip `on_` and PascalCase the rest (`on_ok` to `Ok`, `on_value_change` to `ValueChange`). Duplicate attribute names are an error.

#### Expansion

The whole `rsx!{ nodes }` becomes `::egui_reactor::view(|cx| { stmts })` (not `move`). One element becomes the following statement.

```rust
cx.scope((line!(), column!(), 3usize, key), |cx| {          // line!/column! are emitted with the element's span. 3 is the element's sequence number inside the rsx!
    Name(cx, ::egui_reactor::props_builder(&Name)
        .x(expr)
        .style(::egui_reactor::layout::ItemStyle::default().grow(1.0))
        .events(&mut |__ev| {
            #[allow(unreachable_patterns)]
            match __ev {
                NameEvent::Ok(a) => ::egui_reactor::Handler::call(|| *open = false, a),
                NameEvent::Cancel(a) => ::egui_reactor::Handler::call(|| *open = false, a),
                _ => {}
            }
        })
        .children(::egui_reactor::view(|cx| { .. }))
        .build());
});
```

- Without `key`, it is `(line!(), column!(), n)`. If `line!()` / `column!()` do not return the element's position (they return the macro call site), embed `Span::line()` / `Span::column()` (stable in 1.88) on the macro side. Either is fine as long as each element gets a unique value.
- `children`: always call `.children(..)`. If the child is "one string literal" or "one `{expr}`", pass that expression as-is (this works for both `Button`'s `children: impl Into<WidgetText>` and `View`'s `children: impl View`). For multiple children or element children, use `::egui_reactor::view(|cx| { .. })`. With no children, pass `()`. `<View/>` compiles because `()` is a `View`; `<Button/>` fails because `()` is not `Into<WidgetText>`. The generic `C` is never left unspecified, so `children: impl View` does not need `#[builder(default)]`.
- `props_builder`: `pub fn props_builder<P: Props, F: for<'a, 's, 'u> Fn(&'a mut Cx<'s, 'u>, P)>(_: &F) -> P::Builder { P::builder() }`. A function item's type cannot be named, so `P` is inferred from the `Fn` bound. `#[component]` implements the `Props` trait for `NameProps` and ties it to `NamePropsBuilder` through the associated type. With this, `use components::Name;` alone is enough to write `<Name/>`.
- `.style(..)` fails with "no method named `style`" on components without a `style` prop. A user component that wants layout attributes declares `style: ItemStyle` and passes it to its children.
- Handlers are called as `Handler::call(closure, a)`, so the `redundant_closure_call` from spike's `(|| ..)()` no longer appears. Do not add `#[allow(clippy::redundant_closure_call)]` to the expansion (there is no reason to anymore. Update ARCHITECTURE.md 3.2).
- If `on_*` is passed to a component without `#[event]`, the fused closure fails to compile because there is no `.events(..)` method. A wrong variant name fails because `NameEvent::Foo` does not exist. Pin both messages with trybuild.

### 2.4 trybuild (`crates/egui-reactor/tests/ui/`)

`tests/compile_fail.rs` runs `trybuild::TestCases::new().compile_fail("tests/ui/*.rs")` and `pass("tests/ui/pass/*.rs")`.

| File | Content |
|---|---|
| `unknown_event.rs` | `<Dialog on_foo={..}/>`: `no variant named Foo` |
| `event_on_plain_component.rs` | `on_click` on a component without `#[event]` |
| `missing_prop.rs` | A required prop left out (typed-builder's error) |
| `unquoted_text.rs` | `<Text>hello</Text>` |
| `component_returns_value.rs` | `#[component] fn A(cx: &mut Cx) -> i32` |
| `hook_without_cx.rs` | `#[hook] fn use_x() {}` |
| `handler_borrows_prop.rs` | E0502 from `<Dialog title={&*title} on_rename={|s| *title = s}/>` (the 2nd case in ARCHITECTURE.md 3.7) |
| `loop_handler_mutates.rs` | `todos.remove(i)` in a handler inside `for` (the 1st case in 3.7) |
| `pass/borrow_shapes.rs` | Sibling handlers sharing `&mut`, `for` + `update_later`, `children` closures, and `view(|cx| ..)` inference all compile |

Commit the `.stderr` files. The toolchain is pinned, so the messages are stable.

### 2.5 Replacing spike

Turn the hand-written `counter` / `dialog` / `use_counter` in `tests/common/mod.rs` into `#[component]` / `#[hook]` / `rsx!` versions, and make the 10 spike tests pass without changes (other than following the move to `cx.ui()`). Keep `use_counter_unscoped` as a function without `#[hook]` (the control for the collision test). Do not change the button labels and logs the tests expect.

### 2.6 Tests (Phase 3)

| # | Test file | What it checks |
|---|---|---|
| 3-1 | spike's 10 tests | All green with the macro versions |
| 3-2 | `rsx_control_flow.rs` | `if` / `else if` / `else`, `for` + `key`, each `match` arm, fragments, `{expr}` with an `Option` |
| 3-3 | `rsx_children.rs` | Single literal / single `{expr}` / multiple children / no children. Inside a parent that takes `children: impl View`, the children's hooks run under the parent's scope |
| 3-4 | `component_props.rs` | Leaving out `Option`, `#[prop(default = expr)]`, `#[prop(into)]`, `&str` props, props with generics |
| 3-5 | `component_events.rs` | 3 `#[event]`s (no payload / a value / a borrowed `&str`), the `events=` escape hatch, `emit` is a no-op when no `on_*` is passed |
| 3-6 | `rsx_scope.rs` | Two `<Counter/>`s in the same `rsx!` are independent, `for` with and without `key` (without, `collisions()` is non-empty), removing an element with `if` unmounts it |
| 3-7 | `compile_fail.rs` | trybuild |

## 3. Phase 4: elements (`crates/egui-reactor-elements/src/`)

Write every element with `#[component]` and take `style: ItemStyle` with `#[prop(default)]`. Widgets call egui inside `cx.leaf(&style, |ui| ..)`. Containers (the egui-native ones) act as a leaf in Taffy mode, and their children draw in Ui mode.

### 3.1 `View` and `Text`

```rust
#[component]
pub fn View(cx: &mut Cx, #[prop(default)] style: ItemStyle, #[prop(default)] display: Display, #[prop(default)] direction: Direction, #[prop(default)] wrap: bool, #[prop(default)] justify: Justify, #[prop(default)] align: Align, align_content: Option<Align>, #[prop(default)] gap: Gap, cols: Option<u16>, children: impl View)
```

Build a `taffy::Style` with `ContainerStyle::merge(&style)`, then `cx.container(id, style, |cx| children.show(cx))`. `id` is `cx.scope_id()`. `gap: Gap` implements `From<f32>` and `From<(f32, f32)>`. With `display="grid"`, turn `cols` into `grid_template_columns: vec![fr(1.0); cols]`. Add `col_span` / `row_span` to `ItemStyle`.

```rust
#[component]
pub fn Text(cx: &mut Cx, #[prop(default)] style: ItemStyle, size: Option<f32>, color: Option<egui::Color32>, #[prop(default)] strong: bool, #[prop(default)] wrap: bool, children: impl Into<egui::WidgetText>)
```

Put `egui::Label::new(rich).wrap_mode(if wrap { Wrap } else { Extend })` in a leaf. The default `Extend` follows ARCHITECTURE.md 6.

### 3.2 Widgets

| Element | props | events | Implementation |
|---|---|---|---|
| `Button` | `children: impl Into<WidgetText>`, `enabled: bool = true` | `on_click: ()` | `ui.add_enabled(enabled, egui::Button::new(..))` |
| `Label` | `children: impl Into<WidgetText>` | | `ui.label` (egui's default wrap. The only difference from `Text` is the wrap default) |
| `TextEdit` | `bind: &mut String`, `multiline: bool = false`, `hint: Option<&str>`, `desired_width: Option<f32>` | `on_change: ()`, `on_submit: ()` | `on_change` on `changed()`, `on_submit` on `lost_focus && Enter` |
| `Checkbox` | `bind: &mut bool`, `label: Option<&str>` | `on_change: bool` | |
| `Slider<T: Numeric>` | `bind: &mut T`, `range: RangeInclusive<T>`, `label: Option<&str>` | `on_change: ()` | |
| `ComboBox` | `bind: &mut usize`, `options: &[impl AsRef<str>]`, `label: Option<&str>` | `on_change: usize` | `egui::ComboBox::from_id_salt(cx.scope_id())` |
| `Image` | `source: egui::ImageSource`, `fit: Option<egui::Vec2>` | | `egui::Image::new(source)`. Loaders are the app's job |
| `Separator` | `vertical: bool = false` | | |

Passing a `bind` element a handler that touches the same state on the same element gives E0502 (`handler_borrows_prop` in 2.4). Document that `on_change` on `bind` elements is only for uses that do not touch state (logging, `Dispatch`).

### 3.3 Containers

| Element | props | Implementation |
|---|---|---|
| `ScrollArea` | `horizontal`, `vertical = true`, `max_h: Option<f32>`, `children: impl View` | `Cx::new(store, ui, scope)` inside `egui::ScrollArea::..show(ui, ..)` |
| `Collapsing` | `header: &str`, `default_open: bool`, `children` | `CollapsingHeader::new(header).id_salt(cx.scope_id())` |
| `Frame` | `fill: Option<Color32>`, `stroke: Option<Stroke>`, `inner_margin: Option<f32>`, `corner_radius: Option<f32>`, `children` | `egui::Frame::new()..show(ui, ..)` |
| `Window` | `title: &str`, `open: Option<&mut bool>`, `resizable`, `children` | `egui::Window::new(title).id(cx.scope_id()).show(cx.ctx(), ..)` |
| `SidePanel` / `TopBottomPanel` / `CentralPanel` | `side: Side` / `TopBottom`, `default_size: Option<f32>`, `resizable`, `children` | `show_inside(ui, ..)` |
| `Vertical` / `Horizontal` | `children` | `ui.vertical` / `ui.horizontal` |
| `Grid` | `cols: usize`, `striped: bool`, `children` | `egui::Grid::new(cx.scope_id()).show`. Split `children` with `Row` elements (`Row` is an element that only calls `ui.end_row()`) |

All of them rebuild `Cx::new(store, ui, scope)` before drawing `children` (the same shape as spike's nested_ui). `Window` and each `Panel` can also be called from inside `cx.container` (Taffy mode). In that case they are not treated as a leaf; they draw directly on `cx.ctx()` / `cx.ui()` (`Window` floats on the context, so it is not placed by the layout).

### 3.4 Tests (Phase 4)

| # | Test file | What it checks |
|---|---|---|
| 4-1 | `widgets.rs` | Drive each widget with kittest (`click` / `type_text` / `key_press(Enter)`); both `bind` and events behave as expected. `ComboBox`: `click`, then `click` an option |
| 4-2 | `containers.rs` | Put `use_state` in the children of each container; it survives across frames. Setting `Window`'s `open` to false unmounts the children. `Grid`'s `Row` starts a new row (y of `rect()`) |
| 4-3 | `layout.rs` | Verify `View`'s `direction` / `justify` / `align` / `gap` / `grow` / `w` / `p` / `m` with `rect()`. `display="grid" cols={2}` lays out in 2 columns. `Text` inside a nested `View` does not wrap at the parent's width (`Extend`) |
| 4-4 | `snapshots.rs` (feature `snapshot`) | `harness.snapshot("..")` for each section of the `layout` example and for `Text` wrap. Use the per-OS default for `SnapshotOptions::threshold` |
| 4-5 | `multi_pass.rs` (core side, replaced) | Rewrite spike's taffy test with `<View>` + `<Button>` + `<Text>` and keep checking that the handler fires exactly once in the 2nd pass |

For snapshots in CI, see 4.5. Commit the images to `crates/egui-reactor-elements/tests/snapshots/`.

## 4. Phase 5: runner and examples

### 4.1 `egui-reactor-app::run`

```rust
pub struct Options {
    pub title: String,
    pub max_passes: usize,               // default 3 (see section 8)
    pub persist: bool,                   // default true. Uses eframe's Storage
    pub canvas_id: String,               // wasm. Default "egui_reactor_canvas"
    pub native: eframe::NativeOptions,
}   // impl Default
pub fn run<V: View>(options: Options, root: impl FnMut(&mut Cx<'_, '_>) -> V + 'static) -> eframe::Result;
```

- native: `eframe::run_native`. In `App::ui`, inside `CentralPanel::default().show(ui, ..)`, do `store.begin_pass`, then `cx.container(root_id, column + reserve_available_space, |cx| root(cx).show(cx))`, then `store.end_pass`. In `App::save`, write `store.save_persisted()` with `storage.set_string("egui_reactor", ..)`. `load_persisted` from the `storage` of `CreationContext`.
- wasm: under `cfg(target_arch = "wasm32")`, `wasm_bindgen_futures::spawn_local(eframe::WebRunner::new().start(canvas, WebOptions::default(), Box::new(..)))`. The canvas is `web_sys::window().document().get_element_by_id(canvas_id)`. `run` returns `Ok(())`.
- `root` is called every frame. Using hooks inside `root` and returning an `rsx!` that borrows that state gives a "cannot return value borrowing a local" error. Write in the doc comment and README that the root should be `|cx| rsx!{ <App/> }` and hooks go in components. The tail of a `#[component]` body avoids the same problem through the rewrite in 2.1.
- Set `Options::max_passes` with `ctx.options_mut`.

### 4.2 `use_persisted` (core, `hooks.rs`)

```rust
#[track_caller]
pub fn use_persisted<'s, T: Serialize + DeserializeOwned + 'static>(cx: &mut Cx<'s, '_>, key: &str, init: impl FnOnce() -> T) -> State<'s, T>;
```

- The Id is `Id::new(("egui_reactor_persisted", key))`. It does not depend on the scope.
- `Store` holds `persisted: RefCell<HashMap<String, String>>` (key to JSON string). `load_persisted(&mut self, json: &str)` loads everything at once; `save_persisted(&self) -> String` serializes the live slots, overwrites the map, then turns the whole map into JSON.
- Add `persist: Option<(String, fn(&dyn Any) -> Option<String>)>` to `Slot`. On the first visit, deserialize if the key is in the map; on failure or absence, use `init`.
- When sweep drops a slot with persist, serialize it into the map first (unmounted state also survives the next start).
- The runner's `App::save` is called by eframe periodically (`auto_save_interval`) and at exit.

### 4.3 examples

Each example is a single `src/main.rs` plus `index.html` / `Trunk.toml`. `main` is shared by native / wasm: `egui_reactor_app::run(Options { title, ..Default::default() }, |cx| rsx!{ <App/> })`.

| example | Content |
|---|---|
| `counter` | `use_state` + `View` / `Text` / `Button`. The same code as the README example |
| `todo` | `Vec<Todo>` with `use_reducer` (Add / Toggle / Remove / Clear). Add with `TextEdit bind` + `on_submit`, list with `for` + `key`, toggle with `Checkbox`, remove through `Dispatch`. Save with `use_persisted("todos", ..)`. Group finished items in a `Collapsing` |
| `layout` | A demo listing `View`'s flex attributes (row / column / each justify / align / grow / gap / nesting / grid). Placed inside a `ScrollArea`. Same layout as the 4-4 snapshots |

Delete `examples/spike`. `members` in `Cargo.toml` can stay `examples/*`.

### 4.4 README

In the "Usage" section, write the full counter code, `cargo run -p counter`, and `trunk serve examples/counter/index.html`. For the hooks / elements tables, just link to ARCHITECTURE.md (English translation and cleanup are Phase 8).

### 4.5 CI

Steps to add to `ci.yml`.

1. `cargo check --workspace --target wasm32-unknown-unknown` (widen from `-p egui-reactor` to `--workspace`. The examples must compile for wasm too)
2. trunk: install `trunk` with `jetli/trunk-action@v0.5`, then `trunk build --release examples/counter/index.html`
3. Do not run snapshots in CI. The committed images were made with the macOS renderer and do not match Linux's software renderer. Write how to run them locally in the README Testing section

Keep the existing `Swatinem/rust-cache` key.

## 5. Steps

1. Phase 2: `view.rs`, then `layout.rs`, then `Surface` in `Cx` (fix spike's tests and examples to follow `cx.ui()`), then `use_memo`, then `dispatch.rs` / `use_reducer`, then the deferred queue, then the overlay. Tests 2-1 to 2-7. Update ARCHITECTURE.md 3.1 / 3.2 / 3.7 / 4 / 5.5 / 6. Commit.
2. Phase 3: add the `egui-reactor-macros` dependencies, then `#[hook]`, then `#[component]`, then `rsx!` (parsing, then attributes, then expansion, then control flow). Add `__private` (typed_builder, the `Component` / `Props` traits, `props_builder`) and re-exports to `egui-reactor`. Replace `tests/common` with the macro version and make spike's tests pass. Tests 3-2 to 3-7. Update ARCHITECTURE.md 3.2 / 3.3 / 3.6. Commit.
3. Phase 4: `View` / `Text`, then widgets, then containers. Tests 4-1 to 4-3, replace `multi_pass` (4-5). Write the snapshots (4-4) behind the feature, generate the images locally, and commit them. Update ARCHITECTURE.md 6. Commit.
4. Phase 5: `use_persisted` (core), then `run` (native), then the 3 examples, then the wasm runner, then `index.html` / `Trunk.toml`, then README, then CI. Delete `examples/spike`. Check the 3 `cargo run`s and `trunk serve` by eye. Commit.
5. Confirm every CI step is green. If the snapshot step is flaky, remove it as described in 4.5.
6. In the PR body, write the results per phase, the ARCHITECTURE.md changes, and anything dropped (if any).

## 6. Points that may need a decision

- **Inference of `props_builder(&Name)`** (2.3). If inference through the `Fn` bound fails for props with lifetime `'e` and generic `C`, have `#[component]` generate a braced struct with the same name as the function (the type namespace does not clash with the function) as the Props, and have `rsx!` emit `Name::builder()`. The user still needs the same single `use`. Drop the `NameProps` name.
- **`typed-builder`'s `crate_module_path`**. If it does not work, have `#[component]` generate its own typestate builder for required fields (one marker generic per required field).
- **The span of `line!()` / `column!()`** (2.3). If it is not unique per element, read `Span::line()` / `Span::column()` on the macro side and embed them as numeric literals.
- **`&'s T` in `use_memo`** (1.2). If carrying it around in `FrozenVec` is too clumsy, fall back to returning a `Memo<'s, T>: Deref<Target = T>` guard. In that case update the table in ARCHITECTURE.md 4.
- **Size of a `View` directly under Ui mode** (1.7). If `reserve_available_width()` breaks when placed inside a `Window`, add a `fill: bool` prop to `View` to switch.
- **`cx.ui()` under `Surface::Taffy`**. If drawing on `egui_ui_mut()` looks clearly broken in tests, make Taffy-mode `ui()` warn once with `log::warn!` instead of panicking, rather than inserting `leaf(default)` automatically.
- **Snapshots in CI** (4.5).
- **wasm `cargo check --workspace`**. If check fails because of `egui-reactor-app`'s wasm dependencies, review `eframe`'s `wasm-bindgen` features. Until the examples pass on their own, narrowing to `-p egui-reactor -p egui-reactor-elements -p egui-reactor-app` is fine.

## 7. Changes to apply to ARCHITECTURE.md (known at the start)

- 3.1: `Cx` has a `ui()` method, not a `ui` field. It has `Surface` (Ui / Taffy), `leaf` / `container`, and `defer`.
- 3.2: Change the list of `View` impls to `Option` / `Vec` / arrays / closures. `rsx!` emits `view(|cx| ..)`. `#[allow(clippy::redundant_closure_call)]` is no longer needed.
- 3.3: Props are built with typed-builder's builder. `Option` and `#[prop(default)]` are optional. `children` is required. The body's tail expression is rewritten to `View::show(tail, cx)`.
- 3.6: `Emitter<'a, 'e, E, A>` has a payload type and a variant constructor. `events` is `Option<&mut dyn FnMut(E)>`; when left out it is a no-op.
- 3.7: The `update_later` closure is `'static` (`move`).
- 4: `use_reducer` messages are applied on the next visit. `use_persisted` is identified by key only and stored as JSON under one key in eframe `Storage`.
- 5.5: The deferred queue is applied before sweep and does not include `Dispatch`.
- 6: The list of layout attributes, `Length` units, `View`'s props, and that egui-native containers become leaves in Taffy mode.
- 7: `egui-reactor` depends on `egui_taffy`. The `run(Options, |cx| rsx!{ <App/> })` form.

## 8. Differences found during implementation

### Phase 2 (step 1)

- **1.1** `impl View for &str` / `String` / `Option` / `Vec` / arrays and the `FnOnce` blanket impl did not conflict under coherence; they coexist as-is. Type inference for `view(|cx| ..)` also passes without annotations (test `view::every_view_impl_draws`).
- **1.2** `push_get` on `elsa::FrozenVec<Box<dyn Any>>` simply returns `&'s dyn Any`, so `&'s T` worked without falling back to the section 6 alternative (the `Memo` guard). Added `memo_last` / `memo_push` / `prune_memo` to `Slot`; `prune_memo` is called on live slots inside sweep (not as a separate loop in `end_pass`). The deps hash is shared with `use_effect` as `hooks::deps_hash`.
- **1.3** The slot value is not a `(S, Arc<Mutex<Vec<M>>>)` tuple. It is split into a state slot (plain `S`) and a queue slot (`id.with("__egui_reactor_reducer_queue")`, `Arc<Mutex<Vec<M>>>`). With a tuple, `State` / `update_later` could not downcast from `Box<dyn Any>` to `S`, and `State` would need a projection function over the slot value. Both slots are visited in the same pass, so sweep behaves the same.
- **1.4** `update_later` exists on both `State` and `Handle`, but the implementation is a single `queue_update` in `state.rs`. `State` and `Handle` now hold `store: &'s Store` instead of `ctx: &'s egui::Context` (`ctx` comes from `store.ctx()`), so the signatures of `State::new` / `Handle::new` became `(store, slot, location)` / `(store, slot)`.
- **1.4** The deferred queue is applied before sweep, so within the public API the "slot already gone" path is never reached (slots unmounted in that pass are still alive). The branch that silently drops when `slot_by_id` is `None` is defensive; the test `deferred::update_later_on_a_slot_that_unmounts_in_the_same_pass_is_dropped` only checks that `update_later` on state unmounted in the same pass does not panic.
- **1.6** The overlay text drops duplicates per `Collision::location` with a `BTreeSet`. kittest can read it with `query_by_label_contains`.
- **1.7** `Surface` is not `pub`; it is a private enum in `cx.rs`. All that is needed from outside is `Cx::new` / `Cx::new_taffy` / `in_taffy()`. `Cx::hook_scope` goes through `reborrow()`, which reborrows `Surface` for a shorter lifetime. The `reserve_available_space()` version of `container` (for the runner's root) is added in Phase 5.
- **1.8** `justify` / `align` in `ContainerStyle` cannot be `Option`. For `rsx!` to pass `justify="center"`, `From<&str>` is needed, and `impl From<&str> for Option<Justify>` violates the orphan rule. Instead, `Justify` / `Align` got a default variant `Normal` (= unspecified, `None` in taffy). `align_content` is `Option<Justify>` rather than the plan's `Option<Align>`, because taffy's `AlignContent` is the same type as `JustifyContent`. `AlignSelf` is a type alias of `Align`, as in taffy. `ItemStyle.align_self` stays `Option<Align>`, and its setter takes `impl Into<AlignSelf>`.
- **1.8** Equal-width columns use `taffy::style_helpers::evenly_sized_tracks(cols)`. It is equivalent to the plan's `vec![fr(1.0); cols]`, but in taffy 0.9 it is a one-element `Vec` of `repeat(cols, 1fr)`.
- **1.8** `Length::Percent` holds a fraction from 0.0 to 1.0 to match taffy. `"50%"` becomes `Percent(0.5)`.
- **1.9** Added `tests/layout.rs`, which is not in the plan's table (parsing of `Length` and the layout enums, precedence of the `m` / `p` shorthands, how `to_taffy` / `merge` map). The 2-7 check "`cx.scope` separates Ids in Taffy mode too" is split into 2 tests, one for the hooks side (`use_state`) and one for the egui side (`ui.collapsing`).
- **Other** Split `Store::end_pass` into 3: `run_deferred`, `sweep`, `show_collision_overlay`. `egui-reactor` re-exports `egui_taffy::taffy` as `egui_reactor::taffy`.

### Phase 3 (step 2)

- **Dependency table in 0** `syn` is **3.0**, not 2. rstml 0.13 depends on syn 3, and `Node` / `KeyedAttribute` embed syn 3 types, so there is no choice. Features: `full` / `extra-traits` (needed to derive `Debug` for `Node<C>`) / `visit` / `visit-mut` / `parsing` / `printing` / `proc-macro`. `typed-builder` is 0.23, `trybuild` is 1.0.
- **2.1** `#[builder(crate_module_path = ::egui_reactor::__private::typed_builder)]` worked as-is through the re-export. The section 6 alternative "generate our own builder" is not needed.
- **2.3** Inference of `props_builder(&Name)` also passed for props with `'e` + generic `C`. The section 6 alternative "generate a braced struct with the same name as the function" is not needed. The `Props` trait and `props_builder` live in `egui_reactor::__private`, and only `props_builder` is also re-exported at the crate root.
- **2.1** `#[event]` arguments do not become Props fields (they only become `Emitter`s). The event enum carries only the generics the payload types actually use (`#[event] on_rename: &str` gives `NameEvent<'e>`), because unused parameters cannot be declared on the enum.
- **2.1** The `events` field is `#[builder(default, setter(strip_option))]`. Making the user pass `Option<&mut dyn FnMut(E)>` to the setter is clumsy, so `rsx!` can write `.events(&mut |ev| ..)`. The `events=` escape hatch is also wrapped as `&mut (expr)`.
- **2.1** The tail rewrite covers not only `Stmt::Expr(_, None)` but also `Stmt::Macro` (no semicolon). When `rsx! { .. }` is the last thing in the body, syn parses it as a statement macro.
- **2.1** `impl Trait` arguments desugar to type parameters `TProp0`, `TProp1`, and so on. The same generics (`'e` + the function's own parameters + `TProp*`) go on all 3 places: the props struct, the function, and the `Props` impl. `'e` is declared only when it is actually used (an unused parameter is an error).
- **2.2** `#[hook]` looks for "the first `&mut Cx` argument" (not limited to the first argument). Whether a type is `&mut Cx` is decided by whether its last path segment is `Cx`.
- **2.3** `Option<T>` props get only `#[builder(default)]`, no `strip_option` (as in the table in plan 2.1). So you write `hint={Some("x")}`. Revisit if this gets clumsy in elements later.
- **2.3** The element Id is made from `(file!(), line!(), column!(), sequence number, key)`. `line!()` / `column!()` return the `rsx!` call site, so elements in the same `rsx!` are told apart by sequence number and different `rsx!`s by position. `Span::line()` / `Span::column()` were not needed. `file!()` was added to reliably separate `rsx!`s in different files that expand at the same position.
- **2.3** For attribute values and `{expr}` nodes, if the block is a single expression, emit the inner expression. Passing `{ expr }` as-is gives an `unused_braces` warning in user code.
- **2.3** When there is an invalid node (unquoted text, `<!DOCTYPE>`), `rsx!` emits only `compile_error!` and an empty `view`. Continuing the element expansion causes a chain of type errors that buries the real message.
- **2.3 / 3.6** The fused closure names `NameEvent::Variant`, so wherever `on_*` is used, `NameEvent` must be imported too. The plan's "the user only needs to `use` `Name`" holds only for props. Stated in ARCHITECTURE.md 3.6.
- **2.4** trybuild has 9 cases (`compile_fail` 8 + `pass` 1). The `.stderr` files are committed. `missing_prop` gives an error on typed-builder's `Error_Missing_required_field_label` type.
- **2.5** `tests/common/mod.rs` has the macro versions `Counter` / `NamedCounter` / `Dialog` / `use_counter`, plus thin wrapper functions `counter(cx, initial)` / `named_counter(..)` (a one-line `rsx!` call each). With these, `sibling_handlers` / `custom_hook` / `collision` pass unchanged. Only `fused_events` built a hand-written `DialogProps { .. }`, so it was rewritten to `rsx!` + `on_ok` / `on_cancel` / `on_rename` (asserts unchanged).
- **2.6** Test file names follow the plan: `rsx_control_flow.rs` / `rsx_children.rs` / `component_props.rs` / `component_events.rs` / `rsx_scope.rs` / `compile_fail.rs`.
- **Other** `examples/spike` was switched to the macro version (`App` / `Counter` / `Dialog`). `main.rs` calls `rsx! { <components::App/> }.show(&mut cx)`.

### Phase 4 (step 3)

#### Follow-ups to Phase 3

- **2.1** `Option<T>` props are now `#[builder(default, setter(strip_option))]`. You can write `hint="x"` / `size={14.0}`. Combined with `#[prop(into)]` it becomes `setter(into, strip_option)`. This reverses the Phase 3 decision "no `strip_option`". The trybuild `.stderr` files were not affected.

#### Implementation

- **3.1** `Gap` (`From<f32>` / `From<i32>` / `From<(f32, f32)>`) lives in `egui_reactor::layout`. It belongs next to `ContainerStyle::gap`, and the `ContainerStyle::gap()` setter now takes `impl Into<Gap>`. Added `col_span` / `row_span` to `ItemStyle` and to the `rsx!` layout attribute list.
- **3.1** `View`'s `align_content` is `Option<Justify>` (the Phase 2 type). `display` / `direction` / `justify` / `align` / `gap` / `side` are `#[prop(default, into)]` and take string literals directly.
- **3.2** `ComboBox`'s `options` is `&[S]` with a generic `S: AsRef<str>`, not `&[impl AsRef<str>]`, because `#[component]` only desugars top-level `impl Trait` in argument types.
- **3.2 / 5.6** Passing `bind` as `&mut *state` sets dirty every frame through `DerefMut`, and the app never goes idle (kittest fails with `ExceededMaxSteps`). Added `State::bind(&mut self) -> &mut T` to core. It only lends `&mut T` without setting dirty; the value only changes on input, so egui issues the repaint. Added to ARCHITECTURE.md 5.6.
- **3.3** egui 0.36 has no `SidePanel` / `TopBottomPanel`; they are merged into `Panel::left/right/top/bottom`. The elements follow: one `Panel` (`side="left"|"right"|"top"|"bottom"`) + `CentralPanel`. The `Side` enum lives in `egui-reactor-elements`.
- **3.3** `Grid` row breaks are not a `<Row/>` element but a function `row()` that returns `impl View`, written as `{row()}`. `rsx!` creates a child `Ui` per element through `cx.scope` and `Ui::push_id`, so `ui.end_row()` inside `<Row/>` never reaches the grid's `Ui`. `{expr}` nodes are not scoped, so it does reach.
- **3.3** For the same reason, `<Panel>` and `<CentralPanel>` placed as sibling elements do not dock (each cuts its area out of its own child `Ui`, and the parent's cursor moves below). The test `containers::panels_dock_when_they_share_one_ui` pins that they line up as expected when drawn on the same `Ui` with no scope in between. Panels are meant to be used at the runner's root. Stated in ARCHITECTURE.md 6.
- **3.4 test 4-3** `grow` and `justify="space-between"` distribute leftover space, so there is no visible difference unless `<View>` has `w` (the Ui-mode `container` reserves the parent's width with `reserve_available_width()`, but the taffy node's own `size.width` stays `auto`). The tests use `w={300.0}`.
- **3.4 test 4-5** Core's `multi_pass.rs` stays as-is, and a `<View>` + `<Button>` + `<Text>` version was added as `egui-reactor-elements/tests/multi_pass.rs` (core does not depend on elements). taffy recomputes when "a node's content changed within the same pass", so the `<Text>` whose width changes must come **after** the handler.
- **3.4 test 4-4** Snapshots are behind feature `snapshot` (`egui_kittest/snapshot` + `egui_kittest/wgpu`). wgpu worked on this machine, so 5 PNGs were generated and committed (`row` / `column_justify` / `grid` / `text_wrap` / `widgets`).
- **Other** The `View` element (a function, value namespace) and the `View` trait (type namespace) can coexist, so glob importing both `egui_reactor::prelude` and `egui_reactor_elements::prelude` does not clash.

### Phase 5 (step 4)

#### Changes made on the coordinator's instructions

- Added **`#[component(shares_ui)]`**. `Props` got `const SHARES_UI: bool` (default `false`), and `rsx!` routes element calls through `::egui_reactor::__private::enter_scope(cx, source, props, Name)`. `enter_scope` picks `cx.scope` or the new `cx.scope_sharing_ui` (which only deepens the hook scope) based on `P::SHARES_UI`. `Panel` / `CentralPanel` / `Row` use this, so `<Panel side="left"/>` + `<CentralPanel/>` placed as siblings dock (test `containers::panels_written_as_siblings_dock`).
- The type argument `P` of `enter_scope` is decided by **the props value itself**, not by inference from an `Fn` bound like `props_builder`. Writing `&Name` twice in the same expression creates 2 independent inference variables, which is ambiguous for generic components. Since the props are built as an argument of `enter_scope`, the temporary of the fused closure `&mut |ev| ..` lives until the end of the statement.
- Removed `row()` and replaced it with `#[component(shares_ui)] Row { children }`. You can write `<Grid cols={2}><Row><A/><B/></Row></Grid>`.
- This change altered 3 trybuild `.stderr` files (the error span now points at the whole `rsx!`). Regenerated and committed.

#### Implementation

- **4.2** `Store` holds `persisted: RefCell<HashMap<String, String>>` and `persisted_keys: RefCell<BTreeSet<String>>`. The latter is needed so `save_persisted(&self)` can walk "the keys `use_persisted` used in this process" (`elsa::FrozenMap` cannot be enumerated through `&self`, so the `Id` is recomputed from the key to look up the slot).
- **4.2** Broken JSON does not panic; it does `log::warn!` and falls back to `init`. If the top level is broken, the whole `load_persisted` is ignored.
- **4.1** Added `Cx::root_container` (the only difference from `container` is `reserve_available_space()` versus `reserve_available_width()`).
- **4.1** `Options::native` is `#[cfg(not(target_arch = "wasm32"))]`. `eframe::NativeOptions` does not exist on wasm.
- **4.1** The root closure's `cx` is effectively unused, so the examples and README write `|_cx| rsx!{ <App/> }`. The signature stays `FnMut(&mut Cx) -> V` as planned.
- **Change to 3.2** Changed the payload of `TextEdit`'s `on_submit` from `()` to `String` and added `clear_on_submit: bool`. While `bind` holds `&mut String`, a handler on the same element cannot touch the same state (E0499). That made todo's "add on Enter and clear the input" impossible to write, so the text is passed as the event payload and clearing is the element's job.
- **4.3** todo treats `use_persisted("todos", ..)` as the real store and the `use_reducer` state as a copy of it. If they differ at the start of a pass, it writes back (writing unconditionally every pass keeps dirty set and never goes idle). The checkboxes bind to a scratch copy because `todos` is borrowed by the loop, and the real change goes through `Dispatch`.
- **2.3 (finishing)** `style={expr}` and the layout shorthand attributes fill the same `style` prop, so `rsx!` now merges them into one `.style(..)`. When both are present, the shorthand attributes chain off the `style=` expression (`<Chip style={style} p={6}/>` gives `.style((style).p(6))`). A wrapper that takes `style: ItemStyle` can receive the caller's layout and add its own. `Chip` in examples/layout and the test `layout::style_and_shorthand_attributes_are_merged` use this form. Added to ARCHITECTURE.md 6.
- **4.5** No snapshot CI step (see 4.5 above). The wasm check was widened to `--workspace`, and `trunk build --release examples/counter/index.html` was added with `jetli/trunk-action`.
- **Other** Deleted `examples/spike` and added `counter` / `todo` / `layout`. Each has `index.html` and `Trunk.toml`.

### Bugs found by manual checks (step 5)

- **3.3 `ScrollArea`** Only the first section was visible in `examples/layout`. The cause was `cx.leaf`. A finite egui_taffy leaf reports "the size of what it drew" as both its min and max size, but `ScrollArea` fills the rect it is given and returns that size, so it got stuck at the first frame's rect and `grow` had no effect. Added `Cx::leaf_fill`. It is a leaf that does not report a content size (`min_size = 0`, `infinite = true`) and leaves sizing to taffy; `ScrollArea` uses it. A `ScrollArea` inside a `<View>` is sized by `grow` / `h` / the remaining space. Added to the table in ARCHITECTURE.md 3.1 and to section 6.
- **4.1 `max_passes`** After resizing the window, the `<View>` inside a `ScrollArea` kept its old width. The inner `<View>` is a separate egui_taffy tree; it learns the width the outer one settled on in the 2nd pass and calls `request_discard` at the end of the 2nd pass, so a 3rd pass is needed. egui silently drops a discard beyond the limit and does not repaint, so the layout stays broken until the next input. The runner's `max_passes` default is now 3, and after `end_pass`, if "a discard was requested but rejected", it calls `request_repaint` so the next frame converges. The test is `egui-reactor-elements/tests/scroll_fill.rs`. Updated ARCHITECTURE.md 5.3 and 7.

### Carried over to later PRs

- `use_persisted_reducer(cx, key, reducer, init)`, which merges `use_persisted` and `use_reducer`. Right now todo "reconciles the persisted slot and the reducer state every pass", and this is the one place where the writing experience drops.
- Automated tests for the wasm (localStorage) path of `use_persisted`. Only tested with the native `Storage` equivalent.
- `App::save` serializes every persisted key each time. If values get large, give slots a dirty flag.

## 9. Material for the PR body

### Results per phase

| Phase | What was done |
|---|---|
| 2 (core hooks) | The `View` trait and `view()`, `layout` (`Length` / `ItemStyle` / `ContainerStyle` / `From<&str>` for each enum), `Surface` (Ui / Taffy) in `Cx` with `ui()` / `leaf` / `container` / `defer`, `use_memo` (`&'s T`), `dispatch.rs` and `use_reducer`, the deferred queue (`defer` / `update_later`), the Id collision overlay. Tests 2-1 to 2-7 + `layout.rs`. |
| 3 (macros) | `#[hook]`, `#[component]` (Props struct + typed-builder, event enum, `Emitter`, tail rewrite to `View::show`), `rsx!` (rstml + custom nodes for `if` / `for` / `match`, attribute routing, fused event closure), `__private` (`Props` / `props_builder`), 9 trybuild cases. Spike's 10 tests pass with the macro versions. |
| 4 (elements) | `egui-reactor-elements`: `View` / `Text`, 8 widgets, 10 containers, `prelude`. Tests 4-1 to 4-5 and 5 snapshots. `#[component(shares_ui)]` (added in Phase 5) draws `Panel` / `CentralPanel` / `Row` on the parent's `Ui`. |
| 5 (runner) | `use_persisted` and persistence in `Store`, `egui-reactor-app::run(Options, root)` (native / wasm), examples `counter` / `todo` / `layout` (`index.html` + `Trunk.toml`), README Usage and Testing, CI workspace-wide wasm check and trunk build. Deleted `examples/spike`. |

### Changes to ARCHITECTURE.md

- **3.1** `Cx` has a `ui()` method, not a `ui` field. It has `Surface` (Ui / Taffy), `leaf` / `container` / `root_container` / `defer` / `scope_sharing_ui`. Added a method table.
- **3.2** Changed the list of `View` impls to `()` / `&str` / `String` / `Option` / `Vec` / arrays / closures (the `IntoIterator` blanket impl is impossible under coherence). `rsx!` emits `view(|cx| ..)`. `#[allow(clippy::redundant_closure_call)]` is not needed. Spelled out what can be written inside `rsx!` (attributes, how `children` is passed).
- **3.3** Props use typed-builder. `Option<T>` and `#[prop(default)]` are optional, and the `Option<T>` setter is `strip_option`. `children` always exists. The body's tail expression is rewritten to `View::show(tail, cx)`. Type inference through `props_builder(&Name)`. `#[component(shares_ui)]` and `Props::SHARES_UI` / `enter_scope`.
- **3.4** The collision overlay implementation (`warn_on_collision`, an `Area` at `Order::Debug`, the text).
- **3.6** `Emitter<'a, 'e, E, A>` has a payload type and a variant constructor. `events` is `Option<&mut dyn FnMut(E)>` and a no-op when left out. The event enum's generics are only those actually used. Wherever `on_*` is written, the enum name must be imported.
- **3.7** The `update_later` closure is `'static` (`move`).
- **4** `use_reducer` messages are applied on the next visit (with the reason). `&'s T` in `use_memo` and `FrozenVec`. `use_persisted` is identified by key only and stored as JSON under one key in eframe `Storage`. Updated the deferred queue row.
- **5.4 / 5.5** End-of-pass order is "deferred queue, then sweep, then overlay". `Dispatch` does not go into the deferred queue.
- **5.6** `State::bind()` does not set dirty (so bind-style widgets do not request a repaint every frame).
- **6** Behavior of `leaf` / `container`, the layout attribute list (`Length` units, `ItemStyle` / `ContainerStyle`, `From<&str>` for the enums, merging `style=` with the shorthand attributes), the element table, that egui-native containers become leaves in Taffy mode, and why panels and `Row` are `shares_ui`.
- **7** `egui-reactor` depends on `egui_taffy` / `typed-builder` / `serde`. The flow of one frame in `run(Options, |_cx| rsx!{ <App/> })` and the contents of `Options`.

### Dropped

- The snapshot test CI step. The committed images were made with the macOS renderer and do not match the Linux software renderer on GitHub Actions. They stay behind feature `snapshot`, and how to run them locally is written in the README Testing section (task.md's done criteria and decisions are updated too).
