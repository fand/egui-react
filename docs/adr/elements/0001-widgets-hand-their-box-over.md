# 0001: A widget with a box of its own hands it over; `p` on `<Button>` is its padding

Date: 2026-09-08 · Status: accepted

## Context

With [layout/0003](../layout/0003-paint-style-inside-item-style.md) the engine paints a box on any node. `Button`, `TextEdit` and `ComboBox` paint a box of their own as well, so a `<Button bg>` would have shown two boxes, and the gallery's floating button (a round pill with padding, a shadow and a resting colour) needed the two to be one rect: the box the layout reserved, the pill egui draws and hovers, and the area that takes the press.

## Decision

When `bg` or `border` is given, the widget gives its resting box up: `Button::frame_when_inactive(false)`, a `TextEdit` frame with no fill and no stroke, a transparent `inactive.weak_bg_fill` and no `bg_stroke` on the `ComboBox`'s leaf visuals. `radius` reaches the widget too. egui's hovered and pressed frames stay on top, so `bg` is the resting colour and hover stays the widget's.

`p` on a `<Button>`, when it is symmetric and in points, becomes `Spacing::button_padding` and is taken out of the layout padding. The node is the same size either way; the widget's rect now covers all of it.

## Rejected

- **`Button::fill(TRANSPARENT)`.** egui applies a `fill` override in every state, so the hovered fill would go too.
- **Keeping `p` as layout padding on a `<Button>`.** The literal reading of the task: the widget draws inside the padding and the background covers it. The hover highlight is then the smaller inner button, not the whole painted pill.
- **Keeping `Button::padding` / `corner_radius` and `Frame`'s own props.** Two names and two types for the same thing as `p` and `radius`, which the task set out to remove.

## Consequences

- Asymmetric or percentage padding on a `<Button>` stays layout padding, where the widget draws inside it.
- `ItemStyle::padding_px()` / `without_padding()` exist for the hand-over and for `<Frame>` in `Ui` mode.
- `<Frame>` has no props of its own left: inside a tree it is a `<View>` with paint plus one `Ui`; over a plain `Ui` it builds an `egui::Frame` from `style`.
- Hover / pressed / focused `bg` is out of scope; a widget's own visuals do it.

## Links

- [ARCHITECTURE section 6, elements list](../../ARCHITECTURE.md#elements-list-egui-reactor-elements)
- `docs/tasks/paint-style/plan.md`, 1.6 and 1.7
