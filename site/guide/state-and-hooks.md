---
title: State and hooks
---

# State and hooks

## `use_state` returns a guard

```rust
let mut count = use_state(cx, || 0i32);
let mut name = use_state(cx, String::new);
```

`use_state(cx, init)` returns a `State<T>`: a `RefMut`-like guard onto a slot
in the store. No setter, no tuple.

```rust
*count += 1;
name.push_str("!");
let shown: String = name.clone();
```

The initialiser runs once, the first time the component is drawn. The value
lives in the store, so `T` need not be `Clone`. The guard dies with the
component body. That is why two sibling handlers can each borrow it mutably.

If handlers below will take the state as `&mut`, read the value first:

```rust
let mut selected = use_state(cx, || 0usize);
let current = *selected;
```

## `bind()` versus `&mut *state`

A mutable deref marks the state dirty, and a dirty state asks for a repaint.
Good for a click handler. Bad for a widget that writes every frame: the app
would repaint forever.

Bound widgets take `state.bind()`, which hands out `&mut T` without marking
anything dirty:

```rust
<TextEdit bind={name.bind()} hint="name"/>
<Checkbox bind={done.bind()} label="done"/>
<Slider bind={amount.bind()} range={0.0..=1.0} label="amount"/>
```

Nothing is lost. These widgets only change the value on input, and input makes
egui repaint anyway.

The widget holds the state mutably, so an `on_change` on the same element may
not touch it too (`E0502`). Use `on_change` to notify something else, like a
`Dispatch`.

## `Handle`

A `Handle<T>` is a `Copy` handle to the same slot with a cell-like API:
`.get()`, `.set(v)`, `.update(|v| ..)`, `.with(|v| ..)`. Use it to carry state
around within a frame, or to pass to `provide_context`.

```rust
let theme = use_handle(cx, || Theme { dark: true });   // no guard
let handle = count.into_handle();                      // releases the guard
```

These are the only two ways to get one. You can never hold a guard and a
handle on the same slot at once. That would double-borrow and panic.

## Context

`provide_context` publishes a `Handle` to its children. `use_context::<T>`
finds the nearest one by type.

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

Give each context its own type. There is no `<Provide value={..}>` element:
the provider must be a component that creates the value itself.

## `use_reducer` and `Dispatch`

When changes are a fixed set of messages, a reducer keeps them in one place:

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

`dispatch.send` queues the message and asks for a repaint. The reducer runs on
the next frame.

`Dispatch<M>` is `Clone + Send + 'static`, the one hook handle that outlives a
frame. Give it to a thread or a future so it can report back.

## `use_persisted`

Same guard, but saved to eframe's storage and reloaded on next launch:

```rust
let mut todos = use_persisted(cx, "todo/todos", Vec::<Todo>::new);
```

`T` must be `Serialize + DeserializeOwned`. The key is a string, not the call
site, so saved data survives edits to the code. The same key in two places is
the same value. Prefix keys by feature (`"todo/todos"`), since all persisted
data shares one namespace.

## Two borrow errors you will hit

Both are plain Rust, and both have a one-line fix.

### A handler edits the collection its loop iterates

```rust
for (i, todo) in todos.iter().enumerate() {
    // `todos` is borrowed by the loop, so this does not compile:
    <Button on_click={|| todos.remove(i)}>"x"</Button>
}
```

Queue the write. `update_later` runs at the end of the frame and asks for a
repaint:

```rust
<Button key={i} on_click={|| todos.update_later(move |t| { t.remove(i); })}>"x"</Button>
```

The closure is `'static`, so captured locals need `move`. `cx.defer(f)` is
the same queue for work that touches no state.

### A value prop and a handler over the same state

```rust
// E0502: the props hold a shared borrow of `title`,
// the handler holds a mutable one.
<Dialog title={&*title} on_rename={|s| *title = s}/>
```

Clone the value in (`title={title.clone()}`), or route the write through
`update_later` or a `Dispatch`. `rsx!` adds no implicit clones. A component
built around `bind` avoids this shape.

## Other hooks

`use_effect`, `use_memo`, `use_animate`, `use_future` and your own `#[hook]`
are in the [Hooks reference](/reference/hooks).

## See it running

[counter](/examples/counter) for the guard, [form](/examples/form) for
`bind`, [todo](/examples/todo) for `use_reducer` and `use_persisted`,
[theme](/examples/theme) for context, [custom-hook](/examples/custom-hook)
for `#[hook]`. The store and repaint policy are in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#5-runtime)
section 5.
