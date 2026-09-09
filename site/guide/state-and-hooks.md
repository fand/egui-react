---
title: State and hooks
---

# State and hooks

## `use_state` returns a guard

```rust
let mut count = use_state(cx, || 0i32);
let mut name = use_state(cx, String::new);
```

`use_state(cx, init)` gives you a `State<T>`: a `RefMut`-like guard onto a slot
in the store, with `Deref` and `DerefMut`. There is no setter and no tuple.

```rust
*count += 1;
name.push_str("!");
let shown: String = name.clone();
```

The initialiser runs the first time this component instance is drawn, and never
again. The value lives in the store, not in the guard, so nothing is written
back on drop and `T` need not be `Clone`. The guard itself lives for the
component body and dies with it — which is exactly why two sibling handlers can
each borrow it mutably in turn.

Reading the value once at the top of the body is a useful habit when handlers
below will take the state mutably:

```rust
let mut selected = use_state(cx, || 0usize);
// Read now, so the handlers below are free to take `selected` as `&mut`.
let current = *selected;
```

## `bind()` versus `&mut *state`

Deref-mut marks the state dirty, and a dirty state asks for a repaint when the
guard drops. That is what you want for a click handler. It is *not* what you
want for a widget that writes into the value every frame: the app would ask for
a repaint every frame and never go idle.

So bound widgets take `state.bind()`, which hands out `&mut T` without marking
anything dirty:

```rust
<TextEdit bind={name.bind()} hint="name"/>
<Checkbox bind={done.bind()} label="done"/>
<Slider bind={amount.bind()} range={0.0..=1.0} label="amount"/>
```

Nothing is lost: those widgets only change the value in response to input, and
input makes egui repaint anyway.

Because the widget already holds the state mutably, an `on_change` on the same
element may not touch it too (that is `E0502`). Use `on_change` to notify
something else — a log, a `Dispatch` — and let `bind` own the value.

## `Handle`, `into_handle`, `use_handle`

A `Handle<T>` is a `Copy` handle to the same slot with a cell-like API —
`.get()` (needs `T: Clone`), `.set(v)`, `.update(|v| ..)`, `.with(|v| ..)` —
all through `&self`. Use it when you want to carry state around inside a frame,
or hand it to `provide_context`.

```rust
let theme = use_handle(cx, || Theme { dark: true });   // no guard is created
let handle = count.into_handle();                      // releases the guard
```

There are exactly two ways to get one, and both of them make it impossible to
hold a guard and a handle on the same slot at once. There is deliberately no
`state.handle()`: that would double-borrow the same `RefCell` and panic.

## Context

`provide_context` publishes a `Handle` for the duration of its children;
`use_context::<T>` finds the innermost one by type.

```rust
#[component(shares_ui)]
fn Themed(cx: &mut Cx, children: impl View) {
    let theme = use_handle(cx, || Theme { dark: true });
    provide_context(cx, theme, |cx| children.show(cx));
}

#[component]
fn Swatch(cx: &mut Cx) {
    let theme = use_context::<Theme>(cx).map_or(Theme { dark: true }, |t| t.get());
    rsx! { <Text>{theme.name()}</Text> }
}
```

The value is keyed by type, so give each context its own type. There is no
`<Provide value={handle}>` element: a props type cannot name the store's
lifetime, so the provider has to be a component that creates the value itself.

## `use_reducer` and `Dispatch`

When the changes are a fixed set of messages, a reducer keeps them in one
place:

```rust
enum Msg {
    Add(String),
    Toggle(usize),
    Remove(usize),
}

let (todos, dispatch) = use_reducer(
    cx,
    |state: &mut Vec<Todo>, msg| reduce(state, msg),
    Vec::new,
);

rsx! { <Button on_click={|| dispatch.send(Msg::Add(draft.clone()))}>"add"</Button> }
```

`dispatch.send` queues the message and asks for a repaint; the reducer runs the
next time the hook is visited, which the repaint guarantees will happen. So a
message sent from a handler shows on the next frame, exactly like a write to
`State`.

`Dispatch<M>` is `Clone + Send + 'static` — the one hook handle that outlives a
pass. That makes it the answer to "I need a callback that crosses frames":
give a background thread or a spawned future a `Dispatch` and let it report
back.

## `use_persisted`

Same guard, but saved to eframe's storage and reloaded on the next launch:

```rust
let mut todos = use_persisted(cx, "todo/todos", Vec::<Todo>::new);
```

`T` must be `Serialize + DeserializeOwned`. The key is an explicit string
rather than the call site, and that is the point: call-site ids move when you
insert a line, which would make saved data unreadable after an edit. In
exchange, the same key in two places is the same value, and visiting it twice
in one pass is a collision like any other. Prefix keys with the feature they
belong to (`"todo/todos"`), because everything an app persists shares one
namespace.

## The two borrow errors you will hit

Both are Rust itself, not the library, and both have a one-line fix.

### A handler modifies the collection its loop is iterating

```rust
for (i, todo) in todos.iter().enumerate() {
    // `todos` is borrowed by the loop, so this does not compile:
    <Button on_click={|| todos.remove(i)}>"x"</Button>
}
```

Queue the write instead. `update_later` runs at the end of the pass, before the
sweep, and asks for a repaint:

```rust
<Button key={i} on_click={|| todos.update_later(move |t| { t.remove(i); })}>"x"</Button>
```

The closure is `'static`, so captured locals — the loop index here — need
`move`. Calling `update_later` while the guard is alive is fine; by the time it
runs the guard is long gone. `cx.defer(f)` is the same queue for work that
touches no state (it does not request a repaint).

### A value prop and a handler over the same state, on one element

```rust
// E0502: the props struct holds a shared borrow of `title`,
// the fused handler closure holds a mutable one.
<Dialog title={&*title} on_rename={|s| *title = s}/>
```

Either copy the value in — `title={title.clone()}` — or route the write through
`update_later` or a `Dispatch`. Props stay borrowed by default and `rsx!` adds
no implicit clones, so this stays a decision you make. A component designed
around `bind` (read and write through one `&mut`) avoids the shape entirely.

## Effects, memos and the rest

`use_effect`, `use_memo`, `use_animate`, `use_future` and hooks of your own with
`#[hook]` each get a section in the [Hooks reference](/reference/hooks).

## See it running

[counter](/examples/counter) for the guard, [form](/examples/form) for `bind`,
[todo](/examples/todo) for `use_reducer` and `use_persisted`,
[theme](/examples/theme) for context, and
[custom-hook](/examples/custom-hook) for `#[hook]`.

The store, the sweep and the repaint policy are in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#5-runtime)
section 5.
