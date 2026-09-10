# Plan: one paint style for every element

The task is in [task.md](task.md). This document records the decisions and
the order of work. If we depart from it during implementation, we update it,
and ARCHITECTURE.md too when the change has design meaning.

Written 2026-09-08 against egui / epaint 0.36.1, taffy 0.9.

## 0. Overview

A box gets its look the way it gets its layout: from the one `style` prop
every element already takes. `PaintStyle` is a new struct, but it rides
inside `ItemStyle` as a field, so no element signature changes, `rsx!` needs
six more shorthand names and nothing else, and every `Cx::leaf` /
`leaf_fill` / `container` / `text` call already carries it. The engine paints
it on both layout paths from the *final* layout of the frame, into shape
slots claimed in draw order, exactly as `<Text>` is painted today.

| Step | Where | What |
|---|---|---|
| 1 | `egui-react` | `PaintStyle`, `ItemStyle::paint`, `to_taffy` reserves the border, `rsx!` shorthands |
| 2 | `engine/mod.rs` | Paint slots on container / leaf / text, `paint_boxes`, opacity |
| 3 | `engine/lite.rs` | Same, plus the border in the solver; parity corpus case |
| 4 | `egui-react-elements` | `Button` / `TextEdit` / `ComboBox` hand their box over; `Frame` re-done; old props removed; tests |
| 5 | `examples/` | No `<Frame>` inside a tree |
| 6 | `docs/ARCHITECTURE.md` | Section 6, 5.3, the elements table |
| 7 | verify | `cargo test --workspace`, clippy, wasm check, snapshots |

## 1. Decisions

### 1.1 Where `PaintStyle` lives, and one struct or two

`crates/egui-react/src/paint.rs`, a module next to `layout.rs`:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PaintStyle {
    pub bg: Option<Color32>,
    pub border: Option<Stroke>,
    pub radius: Option<f32>,
    pub shadow: bool,
    pub custom_shadow: Option<Shadow>,
    pub opacity: Option<f32>,
}
```

with builder setters (`bg`, `border(impl Into<Stroke>)`, `radius(f32)`,
`shadow(bool)`, `custom_shadow(Shadow)`, `opacity(f32)`) and
`PaintStyle::is_none()` (nothing to paint, so no slot is claimed).
Re-exported from `layout`, `lib.rs` and the prelude.

**`ItemStyle` gains `pub paint: PaintStyle`** and forwarding setters with the
same six names. That is the "merge" answer to the question in task.md, for
these reasons:

- `Cx::leaf(&ItemStyle, ..)`, `leaf_fill`, `text` and `container` keep their
  signatures. Every element, every test and every example call site is
  untouched except the ones the task rewrites anyway.
- `rsx!` already routes shorthand attributes into `.style(..)`; `bg` and
  friends join `LAYOUT_ATTRS` and nothing else in the macro moves. A wrapper
  component that takes `style: ItemStyle` (`<Chip style={style} bg=..>`)
  passes paint through for free.
- `ItemStyle::to_taffy` is the one place layout is converted, and the border
  width has to reach taffy there (1.2).

`PaintStyle` still exists as its own type, so an element or a hook can build
one and hand it around, and the field can grow (per-corner radii, per-side
borders) without moving anything.

`ItemStyle` stays `Copy + PartialEq`: `Color32`, `Stroke`, `Shadow` are all
both. The lite path's `same_node` compares the whole item, so a `bg` that
changes re-solves the row; a solve is cheap and the alternative is a second
comparison, so we accept it.

### 1.2 A border of width `n` and taffy's `border`

The layout reserves it. `ItemStyle::to_taffy` sets
`style.border = Rect::length(border.width)` when `paint.border` is `Some`.
`content_rect` already subtracts `layout.border`, so a leaf draws inside the
stroke and a container's children start inside it. On the lite path
`item_padding` adds the border width to each padding edge: taffy's flex
algorithm only ever reads `padding + border`, so the two paths agree. The
stroke is painted on the border box with `StrokeKind::Inside`, so it sits
exactly in the band the layout reserved.

The done criterion "border 2 makes the rect 2 larger again" follows.

### 1.3 `radius` and the shadow

One radius for all three shapes, on the node's **border box**:

- shadow: `shadow.as_shape(border_box, radius)` (`epaint::Shadow::as_shape`
  takes the corner radius and adds the spread itself);
- background: `RectShape::filled(border_box, radius, bg)`;
- border: `RectShape::stroke(border_box, radius, stroke, StrokeKind::Inside)`.

`custom_shadow` wins over `shadow`; `shadow` alone is
`ui.visuals().window_shadow`. `radius` defaults to `0.0`.

### 1.4 Paint order and the slots

Per node, in draw order:

1. **before** the children (container) or the widget (leaf): one slot
   `Painter::add(Shape::Noop)` for shadow + background, filled later with a
   `Shape::Vec([shadow, bg])`;
2. the children / the widget / the galley;
3. **after** them: one slot for the border, if there is one.

Both slots are filled after `Tree::finish` (taffy) / `LiteTree::finish`
(lite), from the layout computed in *this* frame, by a new `paint_boxes`
next to `paint_texts`. This is the `<Overlay>` unsized pattern generalised.
Always deferring, rather than painting placed nodes immediately, means one
code path, and the box is right in the very pass it is created in: a node
created this frame has no layout yet, and a container that grew around
children that stayed put gets its new size without a discard.

Consequences:

- The moved rule (5.3) does not change. A container is still compared by its
  location alone, because its own paint is never a frame behind.
- A hidden node (`display="none"`, and everything under one) claims no slot.
- Nothing is painted in `Ui` mode (outside a tree): `Cx::leaf` there is
  `f(ui)` and the rect is not known until after. That is what `<Frame>` is
  for (1.6), and the doc says so next to "`style` is ignored outside a
  container".

`PendingPaint` holds the node, the two `ShapeIdx`, the `PaintStyle`, and the
absolute opacity at claim time (1.5). The taffy path finds the border box
with a `border_box_of(node, root_min)` walk like `content_rect_of`; the lite
path keeps a `boxes: Vec<Rect>` beside `rects`, filled in `round`.

### 1.5 `opacity`

`Ui::multiply_opacity` on the leaf's own `Ui`; for a container, on the
tree's `root_ui` around the children, restored with `set_opacity(prev)`
after. A leaf `Ui` is `new_child` of the root `Ui`, so it inherits the
factor, and a tree opened inside a leaf starts from that leaf's `Ui`.

`Painter::set` applies the painter's opacity *at set time*, and by then the
root `Ui` is back at the outer value. So a pending paint (and a pending
text) records `root_ui.opacity()` when it claims its slot, and `paint_boxes`
/ `paint_texts` set it on a cloned painter before `set`. A placed `<Text>`
paints through `label_text_selection` in draw order and needs nothing.

### 1.6 Widgets that paint a box of their own

- **`Button`**: `radius` goes to `Button::corner_radius` whenever given. When
  `bg` or `border` is given, `frame_when_inactive(false)`: egui then draws
  only the padding at rest, and its own hovered / pressed frame on top of our
  background, which is exactly "bg is the resting colour, hover stays the
  widget's". `Button::fill(TRANSPARENT)` would kill the hover fill too, so
  it is not used. `p` (see 1.7) becomes `button_padding` so the hover frame
  covers the whole box.
- **`TextEdit`**: when `bg` or `border` is given,
  `.frame(egui::Frame::new().inner_margin(Margin::symmetric(4, 2)))`, the
  same margin egui uses; a custom frame is drawn with no fill and no stroke
  (egui 0.36 has no `frame(bool)`).
- **`ComboBox`** has no fill / stroke builder. On the leaf's own `Ui`
  (`visuals_mut`, a copy), `widgets.inactive.weak_bg_fill = TRANSPARENT` when
  `bg` is given, `widgets.inactive.bg_stroke = NONE` when `border` is, and
  `widgets.*.corner_radius = radius` when `radius` is.
- **`Frame`**: keeps `style` only. In `Ui` mode it builds an `egui::Frame`
  from `style.paint` (fill, stroke, corner radius, shadow) and `style.p*`
  (`inner_margin`, `Px` only), around egui-native children. Inside a tree
  it is a leaf whose look the engine paints and whose children draw in the
  leaf's `Ui` (like `<Vertical>`); the doc says "inside a tree, use `<View>`
  with paint". `fill` / `stroke` / `inner_margin` / `corner_radius` /
  `shadow` / `custom_shadow` props go.
- `Button::padding` and `Button::corner_radius` go. `Overlay::fill` stays
  (out of scope).

### 1.7 `p` on a `Button`

`p` / `px` / `py` (and `pt` `pr` `pb` `pl` when they come out symmetric),
given in `Px`, become `ui.spacing_mut().button_padding` and are removed from
the taffy padding of the leaf. The border box is the same size either way;
what differs is that the widget's rect, hover frame and click area cover the
whole box rather than a smaller pill inside it, which is what the gallery's
floating button needs. Asymmetric or percent padding stays layout padding.
Every other leaf keeps `p` as layout padding: the widget draws inside it and
the background covers it.

## 2. Work steps

### Step 1: `PaintStyle` and the shorthands (`egui-react`, `egui-react-macros`)

- `src/paint.rs`: the struct, setters, `is_none`, and three shape helpers
  (`shadow_shape`, `bg_shape`, `border_shape`) used by both paths.
- `src/layout.rs`: `ItemStyle::paint`, forwarding setters, `to_taffy` sets
  `border`. Module doc of `layout.rs` mentions it.
- `src/lib.rs`: `pub mod paint`, re-export in `layout`, `lib.rs`, prelude.
- `macros/src/rsx/mod.rs`: `LAYOUT_ATTRS` += `bg border radius shadow
  custom_shadow opacity`; `shadow` alone is `true` as today.
- Test: `tests/layout.rs` gets a case that `border` reaches `taffy::Style`
  and `bg` leaves it alone; `tests/rsx_*` compile-only case for
  `<View bg={..} shadow radius={4.0}>`.

### Step 2: the taffy path (`engine/mod.rs`)

- `Tree::paints: Vec<PendingPaint>`, `paint_boxes(ui, root_min)`, run in
  `show` right after `finish`, before `paint_texts`.
- `TreeCx::container(key, style, paint, f)`: claim, opacity, children, claim
  border, restore opacity. `TreeCx::leaf(.., paint, ..)` and `TreeCx::text`
  the same around the widget / galley. `engine::show` takes the root's paint
  too; `Cx::container_taffy` passes `item.paint`, `Cx::root_container`
  passes `PaintStyle::default()`.
- `PendingText` records opacity (1.5).
- Test: `tests/paint.rs`, core APIs only (as `hidden.rs`):
  - `<View bg p={8}>` around a leaf label: label rect is 8 in from the
    container's box on every side; with `border` 2.0 it is 10; both on the
    taffy path (a `<View>` over a plain `Ui`) and the lite path
    (`with_root_size`);
  - the frame's `FullOutput.shapes` contain a `RectShape` with the `bg`
    colour covering the label, a stroke with the border, and none when the
    node is `display="none"`;
  - a `bg` that changes between frames costs no discard
    (`ctx.output().num_completed_passes == 1`).

### Step 3: the lite path (`engine/lite.rs`)

- `item_padding` adds the border width; `boxes: Vec<Rect>` from `round`;
  `LiteTree::paints` + `paint_boxes`; `LiteCx::container` / `leaf` / `text`
  claim slots and handle opacity as in step 2.
- `tests/lite_parity.rs`: corpus case `painted_boxes`: a row with `bg`,
  `border` 2, `p`, `radius` on the container and on one leaf, one text with
  `bg`, one hidden painted child. Rects must agree exactly.

### Step 4: elements (`egui-react-elements`)

- `widgets.rs`: `Button` per 1.6 / 1.7 (`padding`, `corner_radius` removed),
  `TextEdit`, `ComboBox`. Module doc updated.
- `containers.rs`: `Frame` per 1.6.
- Tests: `tests/containers.rs` (`Frame inner_margin` cases → `<View p>` /
  `<Frame p>`; the shadow layout test → `<View shadow p>`),
  `tests/widgets.rs` (`padding_widens_the_button` → `p={..}` and
  `radius={24.0}`), new `tests/paint.rs` with the done-criteria layouts
  through `rsx!`, and `tests/snapshots.rs` `painted_box`: a `<View bg border
  radius shadow>` holding a `<Button bg radius p>` (`--features snapshot`;
  image committed if a GPU is at hand, otherwise noted in the PR).

### Step 5: examples

| Example | Today | After |
|---|---|---|
| `board` | `<Frame w mt inner_margin fill corner_radius>` around `<TitleEdit>` | `<View w mt p bg radius>` |
| `board` `Card` | `egui::Frame` inside the card's own leaf, under `with_visual_transform` | stays: it is egui code in a leaf `Ui`, and the lift transform has to move the background with the content |
| `custom-hook` | `<Frame key inner_margin={8.0}>` | `<View key p={8}>` |
| `gallery` `PaneToggle` | `<Frame shadow corner_radius>` + `<Button padding corner_radius>` | `<Button p={..} radius={24.0} shadow bg={..}>` alone |
| `patch` | `<Frame fill stroke corner_radius inner_margin={0.0}>` | `<View bg border radius>` |
| `shell` | `<Frame inner_margin>` inside `<Window>` (Ui mode) | `<View p={4}>` (starts a tree; same picture) |
| `theme` | `<Frame inner_margin={12.0}>`, `<Frame fill inner_margin corner_radius>` | `<View p>`, `<View bg p radius>` |

`bg` for the gallery button is the theme's `widgets.inactive.weak_bg_fill`,
read from `cx.ui().visuals()`, so the resting pill looks as it does today.

The `grep` criterion in task.md is read as "no `inner_margin=` / `padding=`
*attribute* in an `rsx!` tree": the `plain.rs` comparison apps and the board
card use `egui::Frame::inner_margin(..)` on purpose and stay.

### Step 6: ARCHITECTURE

- Section 6: a "Paint" block after the engine's five lines: `PaintStyle`
  inside `ItemStyle`, the paint order and the slots, the border reserve, why
  nothing paints in `Ui` mode, opacity. "egui-react paints nothing on a
  `<View>`" in the second bullet and in 5.3 becomes "paints only what
  `PaintStyle` says, after the layout".
- Elements table: `Button` (`enabled` / `label`, `on_click`), `Frame` (no
  props of its own), and one line: "every element takes `bg border radius
  shadow custom_shadow opacity` through `style`".
- Layout attributes: `PaintStyle` listed next to `ItemStyle`.
- Decision log entry (2026-09-08).

### Step 7: verify

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace --target wasm32-unknown-unknown
cargo test -p egui-react-elements --features snapshot   # if a GPU is at hand
cargo test -p gallery --features snapshot               # same
grep -rn "inner_margin=\|padding=" examples/*/src        # prints nothing
```

## 3. Out of scope, unchanged from task.md

Per-corner radii, per-side borders, gradients, hover / pressed `bg`,
theming, `Overlay::fill`.

## 4. As built

Where the implementation departed from the plan above, and why.

- **One helper for the four claim sites.** `BoxSlots` (claim the slot behind
  the node, push the opacity) and `PendingPaint<N>` (close the slots, restore
  the opacity, fill both once the layout is final) live in `engine/mod.rs` and
  are used by the container, the leaf, the `<Text>` and the root on *both*
  paths. `N` is the node id — `taffy::NodeId` on one path, an index on the
  other — which is the only thing the two do differently.
- **`PaintStyle::is_none` ignores `radius`.** A radius alone rounds nothing, so
  a node that has only one claims no slot. `opacity` does count, even though it
  draws nothing itself: the engine still has to set it around the children.
- **Two helpers on `ItemStyle`** the plan did not name: `padding_px()` and
  `without_padding()`, so `<Button>` and `<Frame>` can take the padding over
  from the layout without duplicating the shorthand resolution (1.7).
- **`patch` merged the frame and the view it wrapped into one node**, rather
  than putting paint on a `<View>` around the old one: same props, same
  picture, one node fewer. Its 1pt border is now reserved by the layout, so the
  node's content sits 1pt in from each edge where egui's frame stroke used to
  overlap it; every patch test still passes.
- **Parity is checked on the shapes too.** Besides the `painted_boxes` corpus
  case, `lite_parity.rs` picks the marker-coloured `RectShape`s out of
  `FullOutput.shapes` and compares the two paths rect for rect, so a node's box
  is checked as well as the rects its children were given.
- **`tests/paint.rs` in `egui-react` runs over `Context::run_ui`**, not a
  kittest harness: what is under test is the list of shapes the frame came out
  with.
- **The gallery's `board` snapshots were already missing** before this task
  (only the `.new.png` artifacts are in git), so `cargo test -p gallery
  --features snapshot` fails those two whatever the paint does. Every other
  gallery snapshot is byte-identical after the examples were rewritten.
