---
title: Hooks
---

# Hooks

Everything on this page is exported from `egui_react::prelude`.

**Deps are compared by `Hash`.** `use_memo`, `use_effect` and `use_future` all
take a `deps` argument and re-run when its hash changes. Hashing rather than
`PartialEq` is what lets deps be borrowed — `(&str, &[T])` is a valid deps
tuple — and hash collisions are as negligible here as they are for egui's own
ids. A dep of `()` means "once, on mount".

**Hook ids come from the call site**, so you may call a hook inside an `if`,
and two calls on two lines never share a slot. Inside a loop, give the
surrounding element a `key`. In a function of your own, add `#[hook]`.

## `use_state`

```rust
pub fn use_state<'s, T: 'static>(cx: &mut Cx<'s, '_>, init: impl FnOnce() -> T) -> State<'s, T>
```

Keeps a `T` in the store and returns a guard onto it. `init` runs only the
first time this component instance is drawn. The guard derefs both ways, so
`*count += 1` is the whole API; deref-mut marks the slot dirty and asks for a
repaint when the guard drops.

`state.bind()` hands out `&mut T` *without* marking it dirty, for widgets that
write every frame. `state.into_handle()` releases the guard and gives you a
`Copy` `Handle`. `state.update_later(f)` queues a write for the end of the
pass.

```rust
let mut count = use_state(cx, || 0i32);
rsx! { <Button on_click={|| *count += 1}>{format!("{}", *count)}</Button> }
```

## `use_handle`

```rust
pub fn use_handle<'s, T: 'static>(cx: &mut Cx<'s, '_>, init: impl FnOnce() -> T) -> Handle<'s, T>
```

The same slot, but as a `Handle` from the start, creating no guard. `Handle` is
`Copy` and works through `&self`: `.get()` (needs `T: Clone`), `.set(v)`,
`.update(|v| ..)`, `.with(|v| ..)`, `.update_later(f)`. Use it for state you
want to publish with `provide_context`, or to pass around inside a frame.

```rust
let theme = use_handle(cx, || Theme { dark: true });
provide_context(cx, theme, |cx| children.show(cx));
```

## `use_persisted`

```rust
pub fn use_persisted<'s, T: Serialize + DeserializeOwned + 'static>(
    cx: &mut Cx<'s, '_>,
    key: &str,
    init: impl FnOnce() -> T,
) -> State<'s, T>
```

`use_state` that survives a restart. The value is saved as JSON into eframe's
storage under one key, and the slot is identified by the string you pass, not
by the call site — inserting a line must not make saved data unreadable. The
same key used twice is the same value, so prefix keys with the feature they
belong to. Unreadable stored JSON is ignored with a warning and the value falls
back to `init`.

```rust
let mut todos = use_persisted(cx, "todo/todos", Vec::<Todo>::new);
```

## `use_memo`

```rust
pub fn use_memo<'s, D: Hash, T: 'static>(
    cx: &mut Cx<'s, '_>,
    deps: D,
    f: impl FnOnce() -> T,
) -> &'s T
```

Runs `f` only when the hash of `deps` changes, and returns a reference to the
kept value. The reference is independent of the `&mut Cx` borrow, so it happily
coexists with `State` guards and later hooks.

Use it for values that cost real time to build — a filtered and formatted list,
a parsed document — not for cheap ones; drawing cost is not what it saves.

```rust
let filtered = use_memo(cx, (query.as_str(), removed.len()), || {
    rows.iter().filter(|r| r.contains(query.as_str())).cloned().collect::<Vec<_>>()
});
```

## `use_effect`

```rust
pub fn use_effect<D, C, M>(cx: &mut Cx<'_, '_>, deps: D, f: impl FnOnce() -> C)
```

Runs `f` when the hash of `deps` changes, and the first time. The body runs
**in place**, at the call site, so it can borrow guards and locals — unlike
React, where effects run after commit.

`f` may return nothing, or a cleanup closure (`FnOnce() + 'static`). A previous
cleanup runs before the body runs again, and the end-of-pass sweep runs the last
one on unmount. Because a cleanup is stored, it is `'static`: hold a `Dispatch`
if it has to change other state.

```rust
let ctx = cx.ctx().clone();
use_effect(cx, dark, move || {
    ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
});
```

## `use_animate` / `use_animate_with`

```rust
pub fn use_animate(cx: &mut Cx<'_, '_>, on: bool, time: f32) -> f32
pub fn use_animate_with(cx: &mut Cx<'_, '_>, on: bool, time: f32, easing: fn(f32) -> f32) -> f32
```

0 while `on` is false, 1 while it is true, eased between the two for `time`
seconds after `on` flips (`cubic_out` by default). egui's animation manager owns
the value and asks for the repaints while it moves, so there is no slot in the
store and nothing for the sweep to drop.

```rust
let down = use_animate(cx, open, 0.25);
if down <= 0.0 {
    return;
}
```

## `use_reducer`

```rust
pub fn use_reducer<'s, S: 'static, M: Send + 'static>(
    cx: &mut Cx<'s, '_>,
    reducer: impl FnMut(&mut S, M),
    init: impl FnOnce() -> S,
) -> (State<'s, S>, Dispatch<M>)
```

State plus a message queue. `Dispatch::send` pushes a message and calls
`request_repaint`; the reducer folds queued messages in **the next time the
hook is visited**, which is why a message from another thread costs no extra
frame. The queue is drained on every visit, so the second pass of a multi-pass
frame applies nothing twice.

`Dispatch<M>` is `Clone + Send + 'static` — the only hook handle that outlives
a pass, and therefore the way to report back from a thread or a spawned future.

```rust
let (todos, dispatch) = use_reducer(cx, |state: &mut Vec<Todo>, msg| reduce(state, msg), Vec::new);
```

## `provide_context` / `use_context`

```rust
pub fn provide_context<'s, T: 'static>(
    cx: &mut Cx<'s, '_>,
    value: Handle<'s, T>,
    children: impl FnOnce(&mut Cx<'s, '_>),
)
pub fn use_context<'s, T: 'static>(cx: &Cx<'s, '_>) -> Option<Handle<'s, T>>
```

Pass a value down a subtree by type. The binding lasts exactly as long as
`children` runs, and what comes back is a `Handle`, not a guard, so the
provider can keep its own state at the same time. Outside the provider,
`use_context` returns `None`.

There is no `<Provide value={handle}>` element — a props type may not name the
store's lifetime — so write a provider component that creates the value itself.

```rust
#[component(shares_ui)]
fn Themed(cx: &mut Cx, children: impl View) {
    let theme = use_handle(cx, || Theme { dark: true });
    provide_context(cx, theme, |cx| children.show(cx));
}
```

## `use_future`

```rust
pub fn use_future<'s, D, T, F>(cx: &mut Cx<'s, '_>, deps: D, f: impl FnOnce() -> F) -> &'s Poll<T>
```

Builds and starts a future when the hash of `deps` changes, and returns
`&Poll<T>` (`Poll` is re-exported from the prelude). `f` runs in place, so it
can read guards and locals to build the future; the future itself is `'static`
and owns what it captured. `T` must be `Send + 'static`.

A result that arrives after deps changed is dropped, so a stale response never
overwrites a newer one. While pending, the hook counts itself toward the
nearest `<Suspense>`.

```rust
let response = use_future(cx, (url, attempt), || {
    let request = ehttp::Request::get(url);
    async move { ehttp::fetch_async(request).await }
});
let Poll::Ready(response) = response else {
    return;
};
```

## `spawn`

```rust
pub fn spawn(fut: impl SpawnFuture<()>)
```

Not a hook: fire-and-forget work on the same executor (a thread with
`pollster::block_on` natively, `spawn_local` on the web). It returns nothing, so
report results with a `Dispatch`.

```rust
spawn(async move { dispatch.send(Msg::Saved(save().await)); });
```

## `update_later` and `cx.defer`

```rust
state.update_later(|v: &mut T| ..)    // also on Handle
cx.defer(|| ..)
```

Not hooks either, but the same queue: both run at the end of the pass, before
the sweep, so a write to a slot that unmounts this pass still lands. The
closures are `'static`, so captured locals need `move`. `update_later` asks for
a repaint when it is applied; `defer` does not, because it touches no state.

This is the fix for "a handler inside a loop wants to modify the collection the
loop is iterating":

```rust
<Button key={i} on_click={|| todos.update_later(move |t| { t.remove(i); })}>"x"</Button>
```

## Hooks of your own

An ordinary function that takes `&mut Cx` and calls other hooks, with `#[hook]`
on it. The attribute is what makes it reusable: it enters a scope keyed by the
*call site*, so two calls get their own state instead of colliding.

```rust
#[hook]
pub fn use_previous<T: Clone + PartialEq + 'static>(cx: &mut Cx, value: T) -> Option<T> {
    let mut seen = use_state(cx, || (value.clone(), None::<T>));
    if seen.0 != value {
        let last = std::mem::replace(&mut seen.0, value);
        seen.1 = Some(last);
    }
    seen.1.clone()
}
```

## Not provided

`use_callback` and `memo` do not exist. There is no diffing, so preserving
referential identity would buy nothing; a callback that has to cross frames is
a `Dispatch`.

## More

The semantics in full — id derivation, collision detection, what the sweep
does, the exact ordering guarantees — are in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#4-hooks-list)
section 4.
