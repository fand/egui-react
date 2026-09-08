# 0003: Paint lives in `ItemStyle` and is drawn from the final layout

Date: 2026-09-08 · Status: accepted

## Context

How a box looked was a prop of one element or another: a background only on `<Frame>` and `<Overlay>`, a border only on `<Frame>`, a corner radius on `<Frame>` and `<Button>` under different types, and two spellings of padding beside `ItemStyle::p`. The only way to get a painted box was to wrap the thing in a `<Frame>`, one more leaf `Ui` between the tree and the widget, and every example did. The engine already knew the rect of every node before the widget drew, so nothing stopped it from painting the box itself. The task is `docs/tasks/paint-style/`.

## Decision

`PaintStyle` (`bg` `border` `radius` `shadow` `custom_shadow` `opacity`) is a field of `ItemStyle`, with forwarding setters and `rsx!` shorthands, so every element takes it through the one `style` prop it already has. The engine paints it on both layout paths from *slots claimed in draw order and filled after the layout is final*: the shadow and the background behind the node, the border in front. A border's width is reserved by the layout (taffy's `border`; the lite solver's padding edges) and drawn `StrokeKind::Inside`.

## Rejected

- **A second `paint: PaintStyle` prop on every element.** Cleaner separation, but every element signature, every `Cx::leaf` / `container` / `text` call and the `rsx!` macro would have changed, and a wrapper component that takes `style` would have had to take `paint` as well. Inside `ItemStyle` none of that moves, and `to_taffy` is the one place the border width has to reach taffy anyway. `PaintStyle` stays its own type so it can grow per-corner radii or per-side borders.
- **Painting a node right away when it already has a layout, deferring only new ones.** Two code paths, and a container that grew this frame around children that stayed put would have been painted a frame behind. Deferring always costs one `Vec` per tree and leaves the moved rule (ARCHITECTURE 5.3) as it was.
- **Painting in `Ui` mode too.** Outside a tree `cx.leaf` is `f(ui)` and the rect is not known until the widget has drawn. `<Frame>` stays as the escape hatch there, building an `egui::Frame` from the same `style`.

## Consequences

- The lite path's `same_node` compares the whole `ItemStyle`, paint included, so a background that changes re-solves the row. A solve is cheap; the alternative was a second comparison.
- `Painter::set` applies the painter's opacity at set time, so a deferred shape records the opacity in force where its slot was claimed and is set through a painter of the engine's own.
- A hidden node (`display="none"`) claims no slot. Hover and pressed colours stay the widget's; `bg` is the resting colour (see [elements/0001](../elements/0001-widgets-hand-their-box-over.md)).
- Every new paint attribute needs a case in the lite parity corpus, as every layout attribute does.

## Links

- [ARCHITECTURE section 6, Paint](../../ARCHITECTURE.md#paint)
- `docs/tasks/paint-style/plan.md`
