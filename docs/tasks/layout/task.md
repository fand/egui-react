# Task: overlays without the escape hatch

## Result (2026-09-07)

Every step done, in seven commits; the order is in [plan.md](plan.md) 9.8.

| Done criterion | Status |
|---|---|
| `<Overlay anchor="bottom-right">` reports a rect in the window's bottom-right quarter whatever the tree draws | **Pass**, by test (`overlay.rs`, `anchored_bottom_right_stays_in_the_corner`) |
| An overlay with `w="100%" h="100%"` covers the window, and a press on a button under it does not reach the button | **Pass**, by test (`a_sized_overlay_takes_the_presses`; the same app without the sheet counts the press) |
| Children of an overlay that is not drawn are unmounted | **Pass**, by test (`an_overlay_not_drawn_unmounts_its_children`; the store is back to zero slots) |
| `use_animate` goes from 0 to 1 over `time` seconds of steps, and is 1 at once when `time` is 0 | **Pass**, by test (`animate.rs`; ten steps of 0.05 s for `time` 0.5) |
| A `<View display="none">` has zero size, its `<Text>` is not in the accesskit tree, and a `use_state` inside keeps its value | **Pass**, by test (`hidden.rs`), and both layout paths agree (`lite_parity.rs`, `hidden_in_row`) |
| `cargo test -p gallery` passes unchanged, and `grep -c "egui::Area\|Cx::new" examples/gallery/src/lib.rs` is 0 | **Pass.** The grep is 0; the gallery tests are unchanged bar one comment, and one test was added |
| The gallery on a phone looks and moves as it did at the end of PR #15, example state included | **Pass by test** (`the_example_keeps_its_state_across_a_trip_to_the_code_pane`). By hand on a phone: not checked here |

Two things came out different from the plan, both small:

- A hidden `<Text>` registers `Rect::NOTHING`, and the parity test translates
  every rect by the row's origin, which turns that into NaN — never equal to
  itself. The test now counts two rects that are both nothing as the same
  answer.
- `<Button padding>` is compared against `style.spacing.button_padding`, read
  through `Context::global_style()`: egui 0.36 has no `Context::style()`.

## Background

The gallery's compact layout (PR #15, `examples/gallery/src/lib.rs`) puts two
things over the page: a button floating in the bottom-right corner that swaps
the running example for its code, and a menu that slides down over the whole
window. Neither can be a node of the flex tree, because both sit *over* the
tree rather than in it, so both are written against egui directly:

| Where | What is called | Why the elements could not do it |
|---|---|---|
| `PaneToggle` | `egui::Area` anchored `RIGHT_BOTTOM`, `egui::Frame` with a shadow, a raw `egui::Button` with its own padding and corner radius, `accesskit_node_builder` for the label | No element floats; `<Frame>` has no `shadow`; `<Button>` has no `padding` / `corner_radius` |
| `Menu` | `egui::Area` at a `fixed_pos` partly above the window (`constrain(false)`), `ctx.animate_bool_with_time_and_easing` for the slide, `ctx.move_to_top` to stay above the toggle, `allocate_rect(.., Sense::click())` so the pane beneath gets no presses, then `Cx::new` + `root_container` to get back into the tree | Same, plus no hook wraps egui's animation, and nothing re-enters the tree for the caller |
| `App` | Switching to the code pane unmounts the example, so it starts over when it comes back | `display="none"` is only honoured on the lite path (`engine/lite.rs`); the taffy engine still draws a hidden subtree into a zero rect |

The first two rows are the same shape as `<Window>` (`containers.rs`): an egui
container that is anchored to the `Context`, not to the parent, with children
that re-enter through a fresh `Cx`. There is no element for the general case,
and that is the gap. The third row is a separate limitation of the engine that
the same page runs into.

Two older hatches in the gallery are **not** this task: `Chip` draws
`ui.selectable_label` because no element shows a pressed state, and the code
pane draws `Label::new(Arc<Galley>)` per line because `<Label>` takes
`WidgetText` only. Both are widget gaps, not layout ones.

## Goal

The gallery's `lib.rs` has no `egui::Area`, no `Cx::new`, and reads
`cx.ctx()` for nothing but `content_rect()` (the width that picks the
layout). Everything the two overlays do is done with elements and hooks that
any app can use.

## Scope

### In scope

- `<Overlay>` (`egui-react-elements`): an `egui::Area` as an element. Where
  it sits (`anchor` + `offset`, or `pos`), which layer (`order`), whether it
  may leave the window (`constrain`), whether it stays on top (`top`), and a
  `fill`. With `w` / `h` the children get a tree of their own that fills the
  sheet, rooted the way the runner roots the app; without, they size the
  sheet, as in `<Window>`. The sheet swallows the presses that would reach
  what is under it.
- `use_animate` (`egui-react`): `ctx.animate_bool_with_time_and_easing`
  behind a hook keyed by the call site, so a component gets "0 when off, 1
  when on, in between on the way" without touching the `Context`.
- `<Frame shadow>` and `<Button padding corner_radius>`: the two props the
  floating button needed.
- `display="none"` on the taffy path: a hidden `<View>` takes no space, draws
  nothing and reports nothing to accesskit, while the components inside keep
  running so their hooks survive. The gallery then keeps the example mounted
  while its code is up.
- The gallery rewritten on top of these, with its tests unchanged (they query
  labels, not implementation).
- ARCHITECTURE: `Overlay` in the elements table, `use_animate` in the hooks
  list, `display="none"` under Layout.

### Out of scope

- `position: absolute` / `inset` on `ItemStyle`. taffy supports it, but an
  absolute node is still in the parent's egui layer: nothing guarantees it
  paints over an example's own `ScrollArea`, and a sheet sliding in from
  above the window has nowhere to be clipped. The `Area` is the right tool
  for both overlays; absolute positioning can come later for the "badge in
  the corner of a card" case, on its own task.
- Toggle buttons and galley labels (the older hatches above).
- Touch gestures (swipe to close the menu). When someone asks.

## Deliverables

- `crates/egui-react-elements/src/containers.rs`: `Overlay`, `Frame::shadow`;
  `widgets.rs`: `Button::padding` / `corner_radius`.
  `tests/overlay.rs` with kittest.
- `crates/egui-react/src/hooks.rs`: `use_animate`. `tests/animate.rs`.
- `crates/egui-react/src/engine/mod.rs`: hidden subtrees. A test next to
  `tests/lite_parity.rs`, so both paths agree on what "hidden" means.
- `examples/gallery/src/lib.rs` without the hatches.
- ARCHITECTURE sections 4 and 6.

Details and the order of work are in [plan.md](plan.md).

## Done criteria

- kittest: an `<Overlay anchor="right-bottom">` reports a rect inside the
  window's bottom-right quarter whatever the tree around it draws; an overlay
  with `w="100%" h="100%"` covers the window and a press on a button under it
  does not reach the button; children of an overlay that is not drawn are
  unmounted (their hooks are gone on the next pass).
- kittest: `use_animate` goes from 0 to 1 over `time` seconds of
  `Harness::step`s after the target flips, and is 1 at once when `time` is 0.
- kittest: a `<View display="none">` has zero size, its `<Text>` is not in
  the accesskit tree, and a `use_state` inside it keeps its value across the
  hide and the show.
- `cargo test -p gallery` passes unchanged, and `grep -c "egui::Area\|Cx::new" examples/gallery/src/lib.rs` is 0.
- The gallery on a phone (`trunk serve`, or the PR preview) looks and moves as
  it does at the end of PR #15: the toggle in the corner, the menu sliding
  down and back up, and the example still holding its state after a trip to
  the code pane.
