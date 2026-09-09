---
title: How it works
---

# How it works

The syntax is React's. The engine is egui's immediate mode. Four facts explain
most of the differences you will meet.

## Nothing is retained

egui redraws every widget every frame, so there is no tree to diff. `rsx!`
builds no tree either. It expands into egui calls right where you write it.

This is why handlers can borrow state with `&mut`. A handler runs during the
frame and is dropped right after, so it never needs to be `'static`:

```rust
let mut count = use_state(cx, || 0i32);

rsx! {
    <View direction="row" gap={8}>
        <Button on_click={|| *count -= 1}>"-"</Button>
        <Button on_click={|| *count += 1}>"+"</Button>
    </View>
}
```

Two closures borrow `count` mutably, one after the other. No `Rc`, no
`RefCell`, no `.clone()`.

## State is keyed by position

egui keeps scroll offsets and open/closed flags in `Context::Memory`, keyed by
an `Id` derived from the widget's place in the tree. Hooks use the same idea.
A hook's slot is keyed by **position in the tree + call site + key**.

What this means:

- You can call `use_state` inside an `if`. React's "same order every render"
  rule does not apply.
- Two `<Counter/>` in different places have separate state.
- `<Counter key={i}/>` in a loop is told apart by its key. Without a key,
  every iteration hits the same slot. Debug builds draw a red overlay with the
  file and line.
- Move a component to another place in the tree and its state resets.
- When `if show { <Counter/> }` turns false, the counter is not visited that
  frame. Its state is dropped and its `use_effect` cleanups run. That is
  unmount. Turning true again is a fresh mount.

## Everything is re-read every frame

Nothing subscribes to state. Every component reads what it needs, each frame.
With nothing retained, fine-grained updates would gain nothing.

"Every frame" is not 60 fps. egui redraws on input, or when something calls
`request_repaint`. The runtime calls it for you when state changes: a mutable
deref of a `State` guard, a deferred write, a finished future, a `Dispatch`
message.

So: **a component that writes state every frame repaints every frame, and the
app never idles.** Widgets that must write every frame, like `TextEdit`, take
`State::bind`. It hands out `&mut T` without marking the state changed.

Drawing still costs. Cache expensive values with `use_memo` and use
`<VirtualList>` for long lists, as you would in plain egui.

## React vs egui-react

| React | egui-react |
|---|---|
| Virtual DOM, reconciler | Direct expansion, nothing retained |
| `memo()`, `useCallback` | Not needed. Use `use_memo` for expensive values |
| Effects run after commit | `use_effect` runs in place, so it can borrow locals |
| Hooks in the same order every render | Hook ids come from the call site. `if` is fine, loops need `key` |
| `setState` shows next render | A write shows next frame. Widgets already drawn keep the old value |
| Deps compared with `Object.is` | Deps compared by `Hash`, so `(&str, &[T])` works |
| `<Provider value=..>` | `provide_context(cx, handle, children)` in a component that owns the value |
| `throw` for Suspense | `let Poll::Ready(x) = use_future(..) else { return };` |
| Callback that outlives the render | `Dispatch`, the one handle that crosses frames |

## When to use it

Use egui-react when the UI has structure: forms, panels, lists with their own
state, layouts that reflow. [board](/examples/board), [form](/examples/form)
and [todo](/examples/todo) show the same app both ways, with line counts on
the tabs.

Plain egui is better for a debug panel or a few `ui.label` calls in a row.
[list-10k](/examples/list-10k) shows the crossover: both versions are about
the same length, because `ScrollArea::show_rows` does the work in both.

You can mix them. `cx.leaf(&style, |ui| ..)` drops into plain egui anywhere in
a tree. See [Escape hatches](/guide/escape-hatches).

## Read more

[counter](/examples/counter) is the smallest example.
[notes](/examples/notes) uses most of the library at once. The runtime
details are in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md)
sections 2 and 5.
