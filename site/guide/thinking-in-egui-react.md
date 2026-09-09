---
title: Thinking in egui-react
---

# Thinking in egui-react

The syntax is React's. The machine underneath is not. Four ideas explain almost
every difference you will run into.

## There is no reconciler

React's diff exists to keep DOM operations down, because they are expensive.
egui re-emits every widget every frame, so there is no retained side to diff
against. `rsx!` builds no tree: it expands into egui calls in place.

That single fact is what makes the code short. Your event handlers are called
right where they are written and then thrown away, so they never need to be
`'static`, and they can borrow local state with `&mut` as usual:

```rust
let mut count = use_state(cx, || 0i32);

rsx! {
    <View direction="row" gap={8}>
        <Button on_click={|| *count -= 1}>"-"</Button>
        <Button on_click={|| *count += 1}>"+"</Button>
    </View>
}
```

Two closures, both taking `count` as `&mut`, one after the other. No `Rc`, no
`RefCell`, no `.clone()`, no `Callback`. Keep this property in mind when you
weigh a design: it is the one thing the library will not trade away.

## State is keyed by position, not by call order

egui keeps no widget tree, but it does keep scroll offsets and open/closed
flags in `Context::Memory`, keyed by `Id` — a hash derived from the parent
down, so a widget drawn in the same place in the same order gets the same `Id`
every frame. Hooks ride on exactly that.

State identity is **position in the tree + call site + key**. Compose's
positional memoization and SwiftUI's structural identity work the same way;
React's "index into the call order" does not. The practical consequences:

- You may call `use_state` inside an `if`. React's "call hooks unconditionally,
  in the same order" rule does not exist here.
- Two `<Counter/>` written in different places have independent state.
- `<Counter key={i}/>` inside a `for` is distinguished by its key. Forget the
  key and two instances ask for the same slot; that is a collision, and the
  library says so loudly rather than silently sharing (in debug builds it draws
  a red overlay naming the file and line).
- Move a component to another position in the tree and its state resets, as in
  React.
- `if show { <Counter/> }` going false means the counter's id is not visited
  this pass; the end-of-pass sweep drops its state and runs its `use_effect`
  cleanups. That is unmount. Going true again is a fresh mount.

## Everything is re-read every frame

Signals push: state knows its subscribers and updates just those nodes. This
design pulls: nobody subscribes, everything is re-read. With no retained side
to skip, fine-grained updates would buy nothing.

"Every frame" does not mean 60 fps. egui redraws when there is input or when
something calls `request_repaint`; with nothing going on it draws zero frames.
The runtime keeps that property by calling `request_repaint` for you whenever
state changes — when a `State` guard was dereferenced mutably, when a deferred
write is applied, when a future completes, when a `Dispatch` message arrives.

The flip side is a rule worth memorising: **a component that rewrites state on
every pass asks for a repaint on every pass, and the app never goes idle.**
That is the same mistake as calling `setState` during a React render. The
escape for widgets that write every frame (`TextEdit` and friends) is
`State::bind`, which hands out `&mut T` without marking the state dirty.

Drawing cost itself cannot be skipped. Cache expensive derived values by hand
with `use_memo`, and virtualise long lists with `<VirtualList>`, exactly as a
plain egui app would.

## Differences from React, in one table

| React | egui-react |
|---|---|
| Virtual DOM and a reconciler | Direct expansion, nothing retained |
| `memo()`, `useCallback` | Not provided, and not needed: there is no identity to preserve. Use `use_memo` for expensive values |
| Effects run after commit | `use_effect` runs **in place**, at the call site, so it can borrow guards and locals |
| Hooks must be called unconditionally | Hook ids come from the call site, so `if` is fine; loops need `key` |
| `setState` shows on the next render | A write in a handler shows on the next frame — widgets already drawn in this pass keep the old value |
| Deps compared with `Object.is` | Deps compared by `Hash`, so borrowed deps like `(&str, &[T])` work |
| Context via `<Provider value=..>` | `provide_context(cx, handle, children)` inside a provider component that owns the value |
| `throw` for Suspense | `let Poll::Ready(x) = use_future(..) else { return };` |
| Callback that outlives the render | `Dispatch`, the one handle that crosses frames |

## When this is a good fit, and when it is not

Reach for egui-react when the UI has structure: forms, panels, lists of things
with their own state, layouts that should reflow. Flexbox and Grid as
attributes, components with props, and hooks keyed by position are worth real
lines of code there — the [board](/examples/board),
[form](/examples/form) and [todo](/examples/todo) pages show the same program
written both ways, with the line counts on the tabs.

Plain egui stays the better answer for a debug panel, a tool window, or
anything that is a handful of `ui.label` and `ui.button` calls in a row: the
library would add a layout tree and a hook store to something that needs
neither. And it is honest about the crossover — see
[list-10k](/examples/list-10k), where the egui-react and plain versions come
out at almost the same length, because the interesting part is
`ScrollArea::show_rows` in both.

You never have to choose once and for all. `cx.leaf(&style, |ui| ..)` drops you
into plain egui anywhere inside a tree, and a whole plain-egui screen can live
inside one node; see [Escape hatches](/guide/escape-hatches).

## See it running

[counter](/examples/counter) is the smallest version of everything above;
[showcase](/examples/showcase) uses most of the library at once.

The implementation behind this page — id derivation, the sweep, the repaint
policy — is in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md)
sections 2 and 5.
