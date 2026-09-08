# Task: one paint style for every element

## Background

Every element takes the same `style: ItemStyle` for layout (`w` `h` `min_*`
`max_*` `grow` `shrink` `basis` `align_self` `m*` `p*`), and `<View>` adds a
`ContainerStyle` on top. That is the whole shared vocabulary. Everything about
how a box *looks* is a prop of one element or another, and which props exist
is whatever the egui builder underneath happens to have:

| Look | Who has it | Spelled as |
|---|---|---|
| Background | `Frame`, `Overlay` | `fill: Option<Color32>` |
| Border | `Frame` only | `stroke: Option<Stroke>` |
| Corner radius | `Frame`, `Button` | `corner_radius: Option<f32>` (`Frame` casts it to `u8`) |
| Shadow | `Frame` only | `shadow: bool` + `custom_shadow: Option<Shadow>` (PR #16) |
| Padding | `Frame` (`inner_margin: f32`), `Button` (`padding: (f32, f32)`) | Two names, two types, and neither is `ItemStyle::p`, which is the layout padding taffy already resolves |

So a `<View>` cannot have a background, a `<Button>` cannot have a border, a
`<Text>` cannot cast a shadow, and the only way to get a painted box is to
wrap the thing in a `<Frame>`. Every example does that (`board`, `custom-hook`,
`gallery`, `patch`, `shell`, `theme`, eighteen `inner_margin` / `padding`
sites between them), and each `<Frame>` is one more leaf `Ui` between the tree
and the widget.

The engine already knows the rect of every node before the widget draws: a
leaf gets `content_rect(layout)`, its box minus taffy's padding and border, and
a container gets its box too. `<Overlay>` in sized mode paints its sheet from
exactly that knowledge (`rect_filled` on the rect, then the children). Nothing
stops the engine from doing the same for every node, on both paths.

## Goal

A box looks the same way it lays out: with props that every element takes.
`<View bg=.. border=.. radius=.. shadow>` paints, `<Button p={8} radius={24.0} shadow>`
is the gallery's floating button without a `<Frame>` around it, and
`ItemStyle::p` is the one padding.

## Scope

### In scope

- `PaintStyle` (`egui-react`, `layout.rs` or a `paint.rs` next to it): `bg`
  (`Color32`), `border` (`Stroke`), `radius` (`f32`, one value for now),
  `shadow` (`bool`, the theme's window shadow) and `custom_shadow`
  (`egui::Shadow`), `opacity` (`f32`, through `Ui::multiply_opacity`).
  Builder methods and `Default`, as `ItemStyle`. Every element that takes
  `style: ItemStyle` takes `paint: PaintStyle` the same way, through
  `#[component]`, so `bg=..` on any element is the same prop.
- The engine paints it, on the taffy path and the lite path alike: the
  shadow, then the background, on the node's border box (not the content
  rect), before the node's children or widget draw; the border after them.
  A shape slot claimed before the children (`Painter::add(Shape::Noop)` then
  `set`, as `<Overlay>` does unsized) where the rect is not known until they
  have drawn. Hidden nodes (`display="none"`, PR #16) paint nothing.
- `p` on a leaf means what it means on a container: the widget draws inside
  the padding, and the background covers it. `Button::padding` and
  `Frame::inner_margin` go; `Button::corner_radius` becomes `radius`.
- A widget that paints a box of its own (`Button`, `TextEdit`, `ComboBox`)
  hands that box over when `bg` or `border` is given: `Button::fill` /
  `stroke` to transparent / none, `TextEdit::frame(false)`, so the two boxes
  do not stack. Hover and press colours stay the widget's; `bg` is the
  resting colour.
- `<Frame>` stays as the escape hatch for `egui::Frame` itself (a painted
  frame around egui-native children in `Ui` mode). Inside a tree it is a
  `<View>` with paint, and the doc says so.
- The examples on top of it: no `<Frame>` inside a tree, no `inner_margin`.
  `cargo test --workspace` passes unchanged (the tests query labels and
  rects; a background does not move a rect).
- ARCHITECTURE: `PaintStyle` next to `ItemStyle` in section 6, the paint
  order, and the elements table without `fill` / `stroke` / `inner_margin` /
  `padding` per element.

### Out of scope

- Per-corner radii, per-side borders, gradients, background images. When
  someone asks; the struct can grow fields without moving anything.
- Hover / pressed / focused styling (`bg` that changes with the response).
  That needs the response before the paint, which is the wrong order for a
  background; a widget's own visuals do it today and keep doing it.
- Theming (`use_theme`, style tokens). A `PaintStyle` is plain values; where
  they come from is a separate task.
- `Overlay::fill`. It is the sheet's colour and already the sized default;
  it can become `bg` when the rest lands, on its own commit.

## Deliverables

- `crates/egui-react/src/layout.rs` (or `paint.rs`): `PaintStyle`,
  exported from `lib.rs` and `prelude`.
- `crates/egui-react/src/engine/mod.rs` and `engine/lite.rs`: paint on
  `container` and `leaf`; `Cx::leaf` / `leaf_fill` / `container` take the
  paint. A corpus case in `tests/lite_parity.rs` so both paths agree on the
  rects a painted node reports.
- `crates/egui-react/tests/paint.rs`: kittest over the core APIs.
- `crates/egui-react-elements`: every element takes `paint`; `Button`,
  `TextEdit`, `ComboBox` hand their box over; `Frame` re-documented;
  `Button::padding` / `corner_radius` and `Frame::inner_margin` /
  `fill` / `stroke` / `corner_radius` / `shadow` / `custom_shadow` removed.
- The six examples without `<Frame>` in a tree.
- ARCHITECTURE section 6.

Details and the order of work go in `plan.md`, which settles at least:
where `PaintStyle` lives; how a border of width `n` relates to taffy's
`border` (the layout has to reserve it, or the stroke is drawn over the
content); what `radius` does to a shadow (egui's `Shadow::as_shape` takes a
corner radius); and whether `#[component]` merges `paint` into `style` or
keeps two structs.

## Done criteria

- kittest: a `<View bg={RED} p={8}>` around a `<Label>` reports a rect
  8 points larger than the label on every side, on both paths; the same
  with `border={Stroke::new(2.0, ..)}` is 2 points larger again.
- Snapshot (`--features snapshot`): one image of a `<View>` with `bg`,
  `border`, `radius` and `shadow`, holding a `<Button>` with `bg` and
  `radius`; it is the shape the gallery's floating button has today.
- `<Button p={..} radius={..} shadow bg={..}>` in the gallery's `PaneToggle`,
  with no `<Frame>`, and `cargo test -p gallery` unchanged.
- `grep -rn "inner_margin\|padding=" examples/*/src` prints nothing, and no
  `<Frame` sits inside a `<View>` in the examples.
- `cargo test --workspace`, clippy and the wasm check pass.
