# Plan: overlays without the escape hatch

The task is in [task.md](task.md). Everything here replaces something the
gallery does by hand today; the "before" is `examples/gallery/src/lib.rs` at
the end of PR #15 (`PaneToggle`, `Menu`, and the `match showing` in `App`).

## 0. What the hatches do, and what replaces each

| Today | Replacement | Section |
|---|---|---|
| `egui::Area::new(id).order(Foreground).anchor(RIGHT_BOTTOM, (-16, -16))` | `<Overlay anchor="right-bottom" offset={(-16.0, -16.0)}>` | 1 |
| `egui::Area::new(id).fixed_pos(..).constrain(false).default_size(screen)` + `ctx.move_to_top` + `rect_filled` + `allocate_rect(.., Sense::click())` + `Cx::new` + `root_container` | `<Overlay pos={..} constrain={false} top w="100%" h="100%" fill={..}>` | 1 |
| `ctx.animate_bool_with_time_and_easing(scope.with("slide"), open, 0.25, cubic_out)` | `use_animate(cx, open, 0.25)` | 2 |
| `egui::Frame::new().shadow(visuals.window_shadow).corner_radius(24)` | `<Frame shadow corner_radius={24.0}>` | 3 |
| `ui.spacing_mut().button_padding = ..; egui::Button::new(..).corner_radius(24)` + `accesskit_node_builder` | `<Button label=".." padding={(16.0, 12.0)} corner_radius={24.0}>` | 3 |
| `match showing { Example => <View key>.., Code => <Code> }` (unmounts the example) | both mounted, one under `<View display="none">` | 4 |

## 1. `<Overlay>`

### Shape

```rust
#[component]
pub fn Overlay(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,   // only `w` / `h` are read
    #[prop(into)] anchor: Option<Anchor>, // "left-top" .. "right-bottom", "center"
    #[prop(default)] offset: (f32, f32),  // from the anchor, in points
    pos: Option<egui::Pos2>,              // instead of `anchor`: the top-left corner
    #[prop(default, into)] order: Order,  // "background" | "middle" | "foreground" | "tooltip"; default "foreground"
    #[prop(default = true)] constrain: bool,
    #[prop(default)] top: bool,           // `move_to_top` every frame
    fill: Option<egui::Color32>,
    children: impl View,
)
```

`Anchor` and `Order` are `str_enum!`s in `egui_reactor::layout`, next to
`Direction`, so `rsx!` takes the string spellings. `Order` maps onto
`egui::Order`; `Anchor` onto `egui::Align2`.

### Behaviour

Same skeleton as `<Window>`: the `Area` is keyed by `cx.scope_id()`, shown
against `cx.ctx()`, and takes no room from the parent. Inside `show`:

- **Sized** (`w` and/or `h` given): the sheet is
  `Rect::from_min_size(ui.max_rect().min, size)`, where a `Percent` is of
  `ctx.content_rect()` and an `Auto` axis is the content's. `fill` paints the
  sheet first. `ui.allocate_rect(sheet, Sense::click())` makes the area that
  size and takes the presses. Then a child `Ui` with `max_rect(sheet)` and
  `Cx::new(store, ui, scope)` + `root_container(scope.with("overlay"),
  root_style(), ..)`, so the children see a column that fills the sheet and
  `grow` works in it. `root_style()` moves from `egui-reactor-app` to
  `egui_reactor::layout` (or is rebuilt inline: column, `w 100% h 100%`) so the
  elements crate does not depend on the app crate.
- **Unsized**: `Cx::new(store, ui, scope)` and `children.show(cx)` in plain
  `Ui` mode, as `<Window>` does. The area is as big as what it draws.

`top` calls `ctx.move_to_top(LayerId::new(order, id))` before `show`. egui
keeps areas in first-shown order within an `Order`, and a click brings one
forward, so an overlay that must stay above another one drawn in the same
frame has to ask every frame. The gallery's menu is the case: the toggle's
area exists first.

`constrain={false}` lets `pos` be above or beside the window, which is what
a slide from an edge is. egui clips the area to the window either way.

Not drawing an `<Overlay>` (the `if open { <Overlay> }` pattern) unmounts
its children through the ordinary sweep, since their scopes are not visited.
The `Area`'s own state (`AreaState`) is egui's and stays; that is fine, it is
only the last position and size.

### Tests (`crates/egui-reactor-elements/tests/overlay.rs`)

- Anchored: `<Overlay anchor="right-bottom" offset={(-16, -16)}><Button/>`
  in a 400x800 harness; the button's rect is inside the bottom-right quarter,
  and stays there when the tree around it draws a tall `<View>`.
- Sized and covering: a `<Button label="under">` in the tree and
  `<Overlay w="100%" h="100%" fill={..}>` over it; pressing at the button's
  centre (hover / drag / drop, as the gallery test does) fires nothing.
- Filling: with `w="100%" h="100%"`, a `<ScrollArea grow={1.0}>` inside the
  overlay reaches the bottom of the window (its rect's bottom is the window's).
- Unmount: a `use_state` counter inside an overlay that is drawn, then not,
  then drawn again starts over.
- `top`: two overlays in the same `Order`, the second with `top`; a press at a
  point they share reaches the second.

## 2. `use_animate`

```rust
#[track_caller]
pub fn use_animate(cx: &mut Cx<'_, '_>, on: bool, time: f32) -> f32
```

Keyed like `use_state` (`scope_id().with(location_key(location))`) and
implemented as `cx.ctx().animate_bool_with_time_and_easing(id, on, time,
easing::cubic_out)`. egui's `AnimationManager` owns the value and requests
the repaints while it moves, so nothing is stored in the `Store` and the
repaint policy (ARCHITECTURE 5.6) is untouched. A `use_animate_with(cx, on,
time, easing)` takes the easing for the callers that want another; the
default is the one the gallery uses.

No `Store` slot means no sweep either: an animation of a component that
stops being drawn stays in egui's memory at its last value, which is what
`animate_bool` does for everyone. Documented, not fixed.

Test (`crates/egui-reactor/tests/animate.rs`, kittest with `step_dt` 0.05):
the value is 0 on the first frame with `on = false`; after flipping, it is
strictly increasing over the next `time / step_dt` steps and 1 after; with
`time = 0` it is 1 on the frame after the flip.

## 3. `<Frame shadow>` and `<Button padding corner_radius>`

- `Frame` gets `#[prop(default)] shadow: bool`, which picks
  `visuals.window_shadow` inside the leaf, so `<Frame shadow>` needs no
  `Context` in the caller. A `custom_shadow: Option<egui::Shadow>` is there
  for anyone who wants their own; it wins over `shadow`.
- `Button` gets `padding: Option<(f32, f32)>` (sets
  `ui.spacing_mut().button_padding` on the leaf's `Ui`, which is a child `Ui`
  so nothing leaks) and `corner_radius: Option<f32>` (`egui::Button::corner_radius`).
  Text size is already the children's business (`RichText::new("</>").size(18.0)`
  is a `WidgetText`).

Tests in the existing `tests/widgets.rs` / `tests/containers.rs`: a padded
button is wider and taller than the same button without by twice the padding;
a shadowed frame's shapes include a shadow mesh (or, simpler, the frame's
outer rect is the same and the test only checks it still lays out). The
snapshot tests in `elements` are where the picture is checked, if a GPU is
around.

## 4. `display="none"` on the taffy path

### Today

`<View display="none">` sets `taffy::Display::None` on the node, and taffy
lays it out at zero. But `TreeCx::leaf` still opens a `Ui` at the zero rect
and runs the closure, so a widget draws (a label spills out of a zero rect;
egui does not clip to the `Ui`), registers with accesskit, and takes input if
the pointer is over its overflow. `TreeCx::text` paints its galley. The lite
path (`lite.rs`, `compute`) already short-circuits: `hide(node)` and
`Sz::ZERO`, and its `paint` skips hidden subtrees.

### Change

`TreeCx` gets a `hidden: bool`, set in `container` when
`style.display == taffy::Display::None`, inherited by children the way
`placed` is. While hidden:

- `container`: still adds the node (keys and indices must stay stable, or
  the show after a hide would be a "created this frame" pass) and runs `f`,
  so components run and hooks live.
- `leaf`: adds the node and runs `f` in a `Ui` built with
  `UiBuilder::new().sizing_pass().invisible()` at the zero rect, the way a
  first-frame measurement already runs. That draws nothing, registers no
  accesskit node, and takes no input (a sizing pass is not interactive), so a
  hidden button is not clickable. It still returns `R`, which is why the
  closure runs at all: `Cx::leaf` returns `R` and callers such as `Button`
  read it, and skipping `f` would mean changing that signature for every
  element.
- `text`: adds the node and skips both paint paths and the widget rect (no
  accesskit node, no selection).
- The measure of a hidden leaf is not written (taffy ignores it anyway).

`Tree::paint_texts` skips hidden nodes, which it can tell from the style.

### Parity

`tests/lite_parity.rs` gets a case with a hidden `<View>` in a row: both
paths give the row the same size and neither reports the hidden `<Text>`.
A test in `crates/egui-reactor/tests/` (`hidden.rs`): a `use_state` counter
inside a hidden `<View>` keeps its value after the view is hidden for a
frame and shown again; `query_by_label` on the hidden text is `None`; the
hidden view's siblings are laid out as if it were absent.

### Gallery

```rust
<View display={if showing == Pane::Example { "flex" } else { "none" }} key={current.name} ..>
    <Running ../>
</View>
<Code display=.. />   // `Code` takes `style`; `display` is a container prop,
                      // so `Code`'s outer `<View>` takes a `display` prop too
```

The example keeps running while the code is up: a `clock` keeps time, a
`todo` keeps its draft. That is what a phone user expects of a tab.

## 5. The gallery afterwards

```rust
if !open {
    <Overlay anchor="right-bottom" offset={(-16.0, -16.0)}>
        <Frame shadow corner_radius={24.0}>
            <Button label={label} padding={(16.0, 12.0)} corner_radius={24.0}
                    on_click={|| *pane = next}>
                {RichText::new(glyph).size(18.0)}
            </Button>
        </Frame>
    </Overlay>
}
```

```rust
let down = use_animate(cx, open, MENU_TIME);
if down > 0.0 {
    let screen = cx.ctx().content_rect();
    <Overlay pos={pos2(0.0, screen.top() - screen.height() * (1.0 - down))}
             constrain={false} top w="100%" h="100%" fill={panel_fill}>
        <View direction="column" gap={8} w="100%" h="100%" p={8}>
            ..header row.., <List w="100%" grow={1.0} min_h={0.0} ../>
        </View>
    </Overlay>
}
```

`content_rect()` stays: it is where the window size comes from, and the
compact/wide decision needs it too. `panel_fill` is
`cx.ctx().style().visuals.panel_fill`; an `<Overlay fill>` default of
`panel_fill` when `w`/`h` are given would remove that read as well.

The tests in `examples/gallery/tests/gallery.rs` do not change: they find
`menu`, `close menu`, `show code`, `show example`, `examples` by label and
press where the `new` button was. `the_menu_covers_the_pane` is exactly the
`<Overlay>` covering test, one level up.

## 6. Docs

- ARCHITECTURE 6, elements table, Containers: `Overlay` (`anchor` / `offset`
  / `pos` / `order` / `constrain` / `top` / `fill`; sized by `w` / `h`, or by
  its children).
- ARCHITECTURE 4, hooks list: `use_animate`.
- ARCHITECTURE 6, Layout: `display="none"` hides the subtree on both paths;
  the components inside still run.
- `docs/tasks/layout/task.md`: the result table, as the other tasks do.

## 7. Order

One PR, one commit per row; each row leaves `cargo test --workspace` green.

1. `use_animate` (no dependants, smallest).
2. `<Frame shadow>`, `<Button padding corner_radius>`.
3. `<Overlay>` and its tests.
4. Gallery: `PaneToggle` and `Menu` on top of 1-3. `grep` for `egui::Area`
   and `Cx::new` in `lib.rs` is empty after this commit.
5. `display="none"` on the taffy path, parity test, and the gallery keeping
   the example mounted.

Step 5 is the one that touches the engine. If it grows (the `leaf` return
value, or `paint_texts`), it moves to a PR of its own and steps 1-4 ship
without it; the gallery is no worse than today in the meantime.

## 8. Open questions

- `Anchor` spellings: `"right-bottom"` (egui's `Align2` order) or CSS-ish
  `"bottom-right"`? The rest of `layout.rs` follows CSS (`"space-between"`),
  so CSS-ish, with egui's order accepted as well.
- Should a sized `<Overlay>` default `fill` to `panel_fill`? A full-window
  sheet is always painted; a corner button never is. Default to "painted when
  sized", overridable with `fill={Color32::TRANSPARENT}`.
- Percent `w` / `h` on an overlay are of the window. Of the parent tree's
  root would be another reading; the window is what a floating thing is
  placed in, so the window.

## 9. Implementation plan (2026-09-08)

Sections 1-8 are the design. This section is the order of work, checked
against the code as it is at `76ac39f` (PR #15) and against egui 0.36.1,
egui_kittest 0.36.1 and accesskit 0.24.1 as pulled into `Cargo.lock`. Where it
differs from sections 1-8, this section wins; each difference is called out.

### 9.0 Decisions on the open questions, and what changed from 1-8

| Question | Decision | Why |
|---|---|---|
| `Anchor` spellings (8) | CSS-ish is the primary spelling (`"bottom-right"`, `"top"`, `"center"`); egui's `Align2` order is accepted too (`"right-bottom"`, `"center-top"`) | The rest of the props follow CSS. Aliases need a hand-written `parse`, so `Anchor` is not a `str_enum!` |
| Where `Anchor` and `Order` live (1) | `crates/egui-reactor-elements/src/containers.rs`, next to `Side`, not `egui_reactor::layout` | They are egui container concepts, not layout attributes; `#[prop(into)]` only needs `From<&str>`, which `Side` already shows |
| Default `fill` of a sized `<Overlay>` (8) | `visuals.panel_fill`; `fill={Color32::TRANSPARENT}` opts out. Unsized: nothing unless `fill` is given | A sheet is always painted; a corner button never is. The gallery then reads `cx.ctx()` for `content_rect()` only |
| Percent `w` / `h` (8) | Of `ctx.content_rect()`. In sized mode an axis that is not given is the window's on that axis | A sheet is measured against the window. A content-sized floating thing is the unsized mode (put a `<View w=..>` inside) |
| `pos` and `anchor` both given | `pos` wins; neither means egui's default (top-left, as `<Window>` without `default_pos`) | egui itself treats both as an error |
| `root_style()` (1) | Moves to `egui_reactor::layout::root_style`; `egui_reactor_app::root_style` becomes a re-export | The elements crate cannot depend on the app crate, and the two must not drift |
| Hidden leaves and accesskit (4) | egui registers an accesskit node for every widget in an invisible `Ui` (the limit 5.8 already documents for `<Suspense>`), so a hidden leaf cannot keep its widgets out of the tree. Instead the leaf's `Ui` gets a node of its own, role `GenericContainer`, marked `hidden`, and every widget under it hangs from that node. `accesskit_consumer::common_filter` excludes a hidden node *with its subtree* — native adapters and `accesskit-web` (`crates/accesskit-web/src/filters.rs`) both use it — so assistive technology never sees them. A hidden `<Text>` registers nothing at all | kittest does not filter hidden nodes, so a test about a hidden *widget* checks the widget's accesskit parent `is_hidden()` and that a press does nothing; a test about a hidden `<Text>` checks `query_by_label` is `None` |
| Trees under a hidden leaf (4) | A `<ScrollArea>` under a hidden `<View>` opens a tree of its own over an invisible `Ui`; that tree starts hidden too, through a counter in the `Store` that `Cx::leaf` holds while a hidden leaf runs. `Cx::is_hidden()` reads it on every surface | showcase's `"no notes"` sits in a `<ScrollArea>`; the gallery test asserts it is gone while the code is up |
| The code pane (4, 5) | Only the example is kept mounted under `display="none"`; `<Code>` stays conditional | Its state is a galley cache that rebuilds in a few ms, and its `hyperlink_to` would stay in the tree kittest queries (`source on GitHub` is asserted absent) |
| `<Window>`, `<Overlay>`, `<Panel>`, `<CentralPanel>` under a hidden view | Not drawn (`if cx.is_hidden() { return; }`); their children unmount as when `open` is false | An `Area` is a layer of its own and a docked panel draws into the tree's root `Ui`; neither is reached by an invisible leaf `Ui` |

Two facts the tests rely on, from the egui 0.36.1 source:

- `AnimationManager::animate_bool`: the first call inserts the id and returns
  the target as it is; each later call moves by at most `stable_dt /
  animation_time`, so with `step_dt` 0.05 and `time` 0.5 the value reaches 1 in
  ten steps; `animation_time == 0` divides by zero, the result is not finite,
  and the target is returned — 1 on the first step that sees `on = true`.
  `Context` asks for a repaint while `0 < v < 1`.
- `Area::show`: the first frame of an area is egui's own sizing pass
  (`sizing_pass = state.is_none()`), invisible and not interactable. Tests
  `harness.run()` before they press anything on or under an overlay.

### 9.1 Step 1: `use_animate`

`crates/egui-reactor/src/hooks.rs`:

```rust
/// egui's `animate_bool_with_time_and_easing` behind a hook: 0 while `on` is
/// false, 1 while it is true, and in between for `time` seconds after `on`
/// flips, eased with `cubic_out`.
///
/// egui's `AnimationManager` owns the value and requests the repaints while it
/// moves, so nothing is stored in the `Store` and the repaint policy (5.6) is
/// untouched. No `Store` slot means no sweep either: the animation of a
/// component that stops being drawn stays in egui's memory at its last value,
/// which is what `animate_bool` does for everyone.
#[track_caller]
pub fn use_animate(cx: &mut Cx<'_, '_>, on: bool, time: f32) -> f32 {
    use_animate_with(cx, on, time, egui::emath::easing::cubic_out)
}

/// [`use_animate`] with an easing of the caller's choosing (`egui::emath::easing`).
#[track_caller]
pub fn use_animate_with(cx: &mut Cx<'_, '_>, on: bool, time: f32, easing: fn(f32) -> f32) -> f32 {
    let id = cx.scope_id().with(location_key(Location::caller()));
    cx.ctx().animate_bool_with_time_and_easing(id, on, time, easing)
}
```

Both are `#[track_caller]`, so `Location::caller()` in the second is the
component's call site whichever one it called. Export from `lib.rs` (the
`pub use hooks::{..}` at the top and the one in `prelude`); update the module
doc line of `hooks.rs`.

Test `crates/egui-reactor/tests/animate.rs` (`mod common; use common::run_app;`,
`Harness::builder().with_step_dt(0.05).build_ui_state(..)`, an
`Rc<Cell<bool>>` for `on` and an `Rc<Cell<f32>>` the app writes the value to):

- `off_is_zero_and_on_rises_to_one_over_time` (`time` 0.5): one step with
  `on = false` reads 0.0. Flip `on`; the next step reads `0 < v < 1`. Step up
  to twelve more times, collecting the values: each is `>=` the previous, the
  last is exactly 1.0 (`cubic_out(1.0)` is `1 - 0`), and it took at least eight
  steps to get there. Flip `on` back; after one step `v < 1`, after twelve
  `v == 0.0` (`1.0 - easing(1.0 - 0.0)` is exactly 0).
- `zero_time_flips_at_once` (`time` 0.0): 0.0, flip, one step, 1.0.

### 9.2 Step 2: `<Frame shadow>` and `<Button padding corner_radius>`

`containers.rs`, `Frame`: two props, `#[prop(default)] shadow: bool` and
`custom_shadow: Option<egui::Shadow>`, and `#[allow(clippy::too_many_arguments)]`
(nine parameters). Inside the leaf:

```rust
if let Some(shadow) = custom_shadow {
    frame = frame.shadow(shadow);
} else if shadow {
    frame = frame.shadow(ui.visuals().window_shadow);
}
```

`widgets.rs`, `Button`: `padding: Option<(f32, f32)>` and
`corner_radius: Option<f32>`, same allow. Inside the leaf, before the widget:

```rust
if let Some((x, y)) = padding {
    // The leaf's own `Ui` in a tree, the scope's `push_id` child outside:
    // `spacing_mut` copies the style, so nothing leaks past this element.
    ui.spacing_mut().button_padding = egui::vec2(x, y);
}
let mut button = egui::Button::new(children).wrap_mode(egui::TextWrapMode::Extend);
if let Some(radius) = corner_radius {
    button = button.corner_radius(radius); // `CornerRadius: From<f32>`
}
```

Tests, in the existing files:

- `tests/widgets.rs`, `padding_widens_the_button`: `<Button label="plain">"x"</Button>`
  and `<Button label="padded" padding={(16.0, 12.0)}>"x"</Button>` in a
  `<View direction="column">`. With `base = harness.ctx.style().spacing.button_padding`
  (`vec2(4.0, 1.0)` by default), `padded.width() - plain.width()` is within 1
  point of `2.0 * (16.0 - base.x)` and the height difference within 1 point of
  `2.0 * (12.0 - base.y)`; the leaf measure is `ceil`ed, hence the tolerance.
  A `corner_radius={24.0}` button in the same tree still clicks.
- `tests/containers.rs`, `a_shadowed_frame_lays_out_like_a_plain_one`: a
  plain `<Frame inner_margin={4.0}>` and a `<Frame shadow inner_margin={4.0}>`
  in a column, each holding a `<Label>`; the two labels have the same size and
  the same `left`, and the second is below the first. A third frame with
  `custom_shadow={egui::Shadow::NONE}` compiles and hosts a counter that keeps
  its state.

### 9.3 Step 3: `root_style` into core

`crates/egui-reactor/src/layout.rs` gets `pub fn root_style() -> taffy::Style`
with the doc comment that is on `egui_reactor_app::root_style` today (the
`min_h` history included). `crates/egui-reactor-app/src/lib.rs` keeps the name:
`pub use egui_reactor::layout::root_style;` with a one-line doc pointing at the
core one. `use egui_reactor::layout::{ContainerStyle, ItemStyle}` in the app
crate becomes unused and goes. Everything that imports
`egui_reactor_app::root_style` (the gallery, `examples/*/tests`,
`crates/egui-reactor-app/tests/root_fill.rs`) keeps compiling; the three
`fn root_style()` copies in `crates/egui-reactor/tests/engine_*.rs` are left
alone (they are the tests' own minimal styles).

This is a commit of its own only if step 4 is not the same day; otherwise it
is the first hunk of step 4.

### 9.4 Step 4: `<Overlay>`

#### Enums (`containers.rs`, after `Side`)

```rust
/// Where an [`Overlay`] is pinned to the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Anchor { #[default] TopLeft, Top, TopRight, Left, Center, Right, BottomLeft, Bottom, BottomRight }
```

`Anchor::parse` takes both spellings: `"top-left" | "left-top"`,
`"top" | "center-top"`, `"top-right" | "right-top"`, `"left" | "left-center"`,
`"center" | "center-center"`, `"right" | "right-center"`,
`"bottom-left" | "left-bottom"`, `"bottom" | "center-bottom"`,
`"bottom-right" | "right-bottom"`. `From<&str>` panics on anything else,
naming the CSS spellings, as `Side` does. `Anchor::to_align2` maps onto
`LEFT_TOP .. RIGHT_BOTTOM`.

```rust
/// Which layer an [`Overlay`] is drawn in: `egui::Order`, with `From<&str>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Order { Background, Middle, #[default] Foreground, Tooltip }
```

`"background" | "middle" | "foreground" | "tooltip"`; `Order::to_egui`.
Both go into `prelude`.

#### The element

```rust
#[component]
#[allow(clippy::too_many_arguments)]
pub fn Overlay(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,   // only `w` / `h` are read
    #[prop(into)] anchor: Option<Anchor>,
    #[prop(default)] offset: (f32, f32),
    pos: Option<egui::Pos2>,
    #[prop(default, into)] order: Order,
    #[prop(default = true)] constrain: bool,
    #[prop(default)] top: bool,
    fill: Option<egui::Color32>,
    children: impl View,
) {
    let (store, scope) = (cx.store, cx.scope_id());
    let layout = cx.layout_id();
    let ctx = cx.ctx().clone();
    let screen = ctx.content_rect();

    // Sized: the sheet is known before anything is drawn, so it can take the
    // presses and root a tree. `Px` as is, `Percent` of the window, an axis
    // that is not given is the window's.
    let sized = style.w.is_some() || style.h.is_some();
    let resolve = |len: Option<Length>, full: f32| match len {
        Some(Length::Px(v)) => v,
        Some(Length::Percent(p)) => p * full,
        Some(Length::Auto) | None => full,
    };
    let sheet_size = egui::vec2(resolve(style.w, screen.width()), resolve(style.h, screen.height()));

    let mut area = egui::Area::new(scope)
        .order(order.to_egui())
        .constrain(constrain)
        .movable(false); // `Area::new` is movable by default; `anchor` / `fixed_pos` clear it, a bare overlay has to
    match (pos, anchor) {
        (Some(pos), _) => area = area.fixed_pos(pos),
        (None, Some(anchor)) => area = area.anchor(anchor.to_align2(), egui::vec2(offset.0, offset.1)),
        (None, None) => {}
    }
    if sized {
        area = area.default_size(sheet_size); // right on the first frame, before the sheet has been allocated once
    }
    if top {
        // Every frame: egui keeps the areas of an `Order` in first-shown
        // order and a click brings one forward.
        ctx.move_to_top(area.layer());
    }
    area.show(&ctx, move |ui| {
        if sized {
            let sheet = egui::Rect::from_min_size(ui.max_rect().min, sheet_size);
            let fill = fill.unwrap_or_else(|| ui.visuals().panel_fill);
            ui.painter().rect_filled(sheet, 0.0, fill);
            // The whole sheet, before the children: it makes the area the
            // sheet's size (that is what stops a press reaching the layer
            // beneath) and a later widget in the same layer is on top of it,
            // so the children's buttons still get theirs.
            ui.allocate_rect(sheet, egui::Sense::click());
            let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(sheet));
            let mut cx = Cx::new(store, &mut ui, scope);
            cx.with_layout_id(layout, |cx| {
                cx.root_container(scope.with("overlay"), root_style(), |cx| children.show(cx));
            });
        } else {
            // The background is painted after the children, into a slot
            // claimed before them, because the area's rect is what they drew.
            let background = fill.map(|_| ui.painter().add(egui::Shape::Noop));
            let mut cx = Cx::new(store, ui, scope);
            cx.with_layout_id(layout, |cx| children.show(cx));
            if let (Some(idx), Some(fill)) = (background, fill) {
                ui.painter().set(idx, egui::Shape::rect_filled(ui.min_rect(), 0.0, fill));
            }
        }
    });
}
```

`use egui_reactor::layout::{ItemStyle, Length, root_style};` at the top of the
file. Like `<Window>`, no `cx.leaf`: an area takes no room from the parent,
and in tree mode the component is ids only. Not drawing an `<Overlay>` (the
`if open { .. }` pattern) unmounts its children through the ordinary sweep;
egui keeps the `AreaState` (last position and size), which is fine. Step 6
adds `if cx.is_hidden() { return; }` at the top. Doc comment: the table in
section 0 in prose, the two modes, and that presses on the sheet go nowhere.

#### Tests: `crates/egui-reactor-elements/tests/overlay.rs`

`mod common; use common::run_app;`, a `Harness::builder().with_size(vec2(400.0, 800.0))`
harness; `Counter` as in `tests/containers.rs`. Every test does
`harness.run()` before it reads a rect or presses (9.0, the area's first frame).

1. `anchored_bottom_right_stays_in_the_corner`: a `<View direction="column" w="100%">`
   holding a `<Label>` and a `<View h={2000.0}/>`, and
   `<Overlay anchor="bottom-right" offset={(-16.0, -16.0)}><Button label="fab">"+"</Button></Overlay>`.
   `fab.right() <= 400 - 15`, `fab.bottom() <= 800 - 15`, `fab.left() > 200`,
   `fab.top() > 400`.
2. `a_sized_overlay_takes_the_presses`: an `Rc<Cell<u32>>` counted by
   `<Button label="under" on_click=..>` in the tree, and
   `<Overlay w="100%" h="100%"><Label>"sheet"</Label></Overlay>` after it.
   `get_by_label("under").click()` (kittest presses at the node's centre, the
   sheet's layer is on top there), `run()`, the count is 0. The same app with
   the overlay switched off through an `Rc<Cell<bool>>`: the count is 1.
3. `a_sized_overlay_roots_a_tree_that_fills_the_sheet`:
   `<Overlay w="100%" h="100%"><View direction="column" w="100%" h="100%" justify="space-between"><Label>"head"</Label><Label>"foot"</Label></View></Overlay>`;
   `head.top() < 30`, `foot.bottom() > 770`.
4. `an_overlay_not_drawn_unmounts_its_children`: `if show.get() { <Overlay anchor="center"><Counter name="sheet"/></Overlay> }`.
   `sheet: 0`; press `sheet +`; `sheet: 1`; `harness.state().len() == 1`;
   `show = false`, run: no `sheet: 1`, `state().len() == 0`; `show = true`,
   run: `sheet: 0`.
5. `top_keeps_an_overlay_above_a_later_one`: two overlays, both
   `pos={egui::pos2(50.0, 50.0)} w={200.0} h={200.0}`, each with one button at
   the same place (`first` / `second`) writing into an `Rc<Cell<&str>>`. With
   `top` on the first, `get_by_label("first").click()` records `"first"`.
   Without `top` (a second harness) the same press records `"second"`: the
   later area is above by egui's own rule, which is what `top` overrides.

### 9.5 Step 5: the gallery on top of 1-4

`examples/gallery/src/lib.rs`:

- `PaneToggle` becomes the `rsx!` in section 5. `on_click={|| on_toggle.emit(next)}`.
- `Menu`: `let down = use_animate(cx, open, MENU_TIME); if down <= 0.0 { return; }`,
  `let screen = cx.ctx().content_rect();`, then the `rsx!` in section 5 with
  `<Overlay pos={egui::pos2(screen.left(), top)} constrain={false} top w="100%" h="100%">`
  and the column `p={8}` (was `sheet.shrink(8.0)`). No `fill`: the sized
  default is `panel_fill`.
- `use egui_reactor_app::root_style;` goes. The doc comments of `PaneToggle`
  and `Menu` are rewritten: they name `egui::Area` today, and the done
  criterion is a `grep`, comments included.
- The module doc (lines 1-13) stays true and stays.

After this commit `grep -c "egui::Area\|Cx::new" examples/gallery/src/lib.rs`
prints 0 and `cargo test -p gallery` passes unchanged.

### 9.6 Step 6: `display="none"` on both paths

#### `Store` (`store.rs`)

```rust
/// How many hidden leaves are being drawn right now (nested leaves count up).
hidden: Cell<u32>,
```

`begin_pass` sets it to 0 (a panic inside a leaf must not leave it raised).
`pub(crate) fn enter_hidden(&self) -> HiddenGuard<'_>` increments and returns
a guard that decrements on drop; `pub fn in_hidden(&self) -> bool`.

#### `Cx` (`cx.rs`)

```rust
/// Is this `Cx` inside a `display="none"` subtree?
///
/// True inside a hidden `<View>` on either layout path, and inside anything
/// a leaf of one opens (a `<ScrollArea>`'s children, a tree of their own).
pub fn is_hidden(&self) -> bool {
    self.store.in_hidden() || match &self.surface {
        Surface::Ui(_) => false,
        Surface::Tree(tree) => tree.hidden(),
        Surface::Lite(lite) => lite.hidden(),
    }
}
```

- `leaf` and `leaf_fill`: `let _hidden = self.is_hidden().then(|| store.enter_hidden());`
  around the surface call, so a tree opened inside the leaf starts hidden.
- `text`, `Surface::Ui` arm: if `self.is_hidden()`, no `Label`:
  `let id = ui.next_auto_id(); ui.skip_ahead_auto_ids(1); ui.interact(Rect::NOTHING, id, Sense::hover())`.
- `container_taffy`: `engine::show` reads `store.in_hidden()` itself;
  `lite::show` gets a `hidden: bool` argument.

#### Taffy path (`engine/mod.rs`)

`TreeCx` gets `hidden: bool` (copied by `reborrow`, read by `pub(crate) fn hidden`);
`show` starts the root at `store.in_hidden()`.

- `container`: `let hidden = self.hidden || style.display == taffy::Display::None;`
  before the style is handed to `add_child_node`; the child `TreeCx` carries
  it. The node is still added and `f` still runs: keys and child indices stay
  stable (a show after a hide is not a "created" pass) and the components
  inside keep their hooks.
- `leaf`: `if first_frame || self.hidden { builder = builder.sizing_pass().invisible(); }`;
  `created_this_frame` is set only when `first_frame && !self.hidden` (a
  hidden leaf's draw is not a measurement anyone waits for). After `new_child`
  and before `f`, when hidden:

  ```rust
  // Every widget `f` registers hangs from the `Ui`'s own id
  // (`Ui::interact` -> `register_accesskit_parent`), so one hidden node
  // here hides the whole subtree from assistive technology:
  // `accesskit_consumer::common_filter` excludes a hidden node with
  // everything under it. egui registers the widgets regardless of
  // visibility (5.8), so this is the one way to keep them out.
  ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
      node.set_role(egui::accesskit::Role::GenericContainer);
      node.set_hidden();
  });
  ```

  `set_measure` is skipped when hidden (taffy lays a `Display::None`
  subtree out at zero whatever the measure says; the old measure stays on the
  node for the show).
- `text`: when hidden, `set_text` as usual (the galley cache and the node
  shape survive), then `return self.root_ui.interact(Rect::NOTHING, scope.with(index), Sense::hover());`
  — a widget rect nothing can hit, no `WidgetInfo` (so no accesskit node),
  no `LabelSelectionState`, no `Noop` shape, no `PendingText`. `paint_texts`
  needs no change.
- `finish`: nothing changes. The frame a subtree is hidden or shown is a
  "moved" pass (real rects to zero, or back), which is one discard, and the
  leaves' visible draw at a zero rect on the show frame is inside that
  discarded pass.

#### Lite path (`engine/lite.rs`)

`LiteCx` gets the same `hidden: bool` and the same three changes:
`container` (`Display::None` is already in the subset, 2073), `leaf` (the
builder, the accesskit node, no `created`, no measure write), `text` (push the
text and the node, then the `Rect::NOTHING` response). `hide()` and the
`compute` short-circuit stay as they are.

#### Elements

`Window`, `Overlay`, `Panel`, `CentralPanel`: `if cx.is_hidden() { return; }`
first thing, with a doc line each: an `Area` is a layer of its own and a
docked panel draws into the tree's root `Ui`, so an invisible leaf `Ui` does
not reach them; their children unmount as when `open` is false.

#### Tests

`crates/egui-reactor/tests/hidden.rs` (core APIs only: `cx.container`,
`cx.leaf`, `cx.text`, `use_state`, as `taffy_cx.rs` does; an
`Rc<Cell<bool>>` `shown`). The tree: a row with `gap` 8 holding leaf `a`
(a button), a container whose `display` is `"flex"` or `"none"` by `shown`,
and leaf `b`. Inside the container, under `cx.scope`s: a counter (`use_state`,
a `hidden +` button leaf, a `count: N` text), a `hidden text` text, and a
leaf that opens a tree of its own (`Cx::new` over the leaf's `Ui`, then
`cx.container`) with a `nested text` text in it.

- `a_hidden_view_takes_no_space_and_reports_no_text`: shown, run: `count: 0`,
  `hidden text` and `nested text` are found. Hidden, run twice: none of the
  three is; `b.left()` is within 1 point of `a.right() + 8`; `hidden +` is in
  kittest's tree and `get_by_label("hidden +").accesskit_node().parent()` is
  `is_hidden()`.
- `state_survives_a_hide_and_a_show`: shown, press `hidden +`, `count: 1`;
  hidden, run twice, `harness.state().len()` is what it was (one slot); press
  where `hidden +` reports itself (nothing happens); shown, run: `count: 1`.
- `crates/egui-reactor/tests/lite_parity.rs`: a corpus case `hidden_in_row`,
  the same row shape (leaf, hidden container with a leaf and a text, leaf).
  The harness already draws it on both paths and compares every rect; hidden
  leaves report a zero rect at the row's corner and hidden texts
  `Rect::NOTHING` on both.

### 9.7 Step 7: the gallery keeps the example, and the docs

`App`, compact branch:

```rust
<View direction="column" grow={1.0} min_h={0.0} gap={4}>
    <Text strong wrap>{current.summary}</Text>
    // Hidden, not unmounted, while the code is up: the example keeps
    // running (a clock keeps time, a todo keeps its draft), which is what
    // a tab is expected to do. The `key` still sweeps it on a switch.
    <View
        key={current.name}
        display={if showing == Pane::Example { "flex" } else { "none" }}
        direction="column"
        grow={1.0}
        min_h={0.0}
    >
        <Running name={current.name} plain={showing_plain}/>
    </View>
    if showing == Pane::Code {
        <Code w="100%" grow={1.0} min_h={0.0} meta={*current} plain={showing_plain} on_pick={..}/>
    }
</View>
```

`examples/gallery/tests/gallery.rs`: the assertions stay; the comment in
`a_phone_shows_one_pane_at_a_time` ("the example is gone with its hooks") is
rewritten. One test is added, `the_example_keeps_its_state_across_a_trip_to_the_code_pane`:
`<App start="counter"/>` on the phone harness, press `+` twice (`2` is
shown), `show code`, `show example`, `2` is still shown and `0` is not.

Docs:

- ARCHITECTURE 4: a row for `use_animate(cx, on, time) -> f32` (and
  `use_animate_with`): egui's `AnimationManager` owns the value, no slot, no
  sweep.
- ARCHITECTURE 6, elements table: `Overlay` in Containers with its props and
  the two modes; `Frame` gains `shadow` / `custom_shadow`; `Button` gains
  `padding` / `corner_radius`.
- ARCHITECTURE 6, Layout: a bullet for `display="none"` — zero size on both
  paths, components keep running, leaves draw into an invisible sizing `Ui`
  under a hidden accesskit node, a `<Text>` registers nothing, `Cx::is_hidden`,
  what is not drawn under it. A sentence in 5.8 pointing here for the
  accesskit limit it already names.
- ARCHITECTURE 3.1: `Cx::is_hidden` in the list of what `Cx` answers.
- `docs/tasks/layout/task.md`: a `## Result` table over the done criteria,
  as `docs/tasks/code-pane/task.md` has.

### 9.8 Order, and what every commit passes

One PR, one commit per step (3 folds into 4):

1. `use_animate` and `tests/animate.rs`.
2. `<Frame shadow>`, `<Button padding corner_radius>`, their tests.
3. + 4. `root_style` into core; `Anchor`, `Order`, `<Overlay>`, `tests/overlay.rs`.
5. Gallery `PaneToggle` and `Menu`; the `grep` is 0.
6. `display="none"`: `Store`, `Cx::is_hidden`, both engines, the four
   elements, `tests/hidden.rs`, the parity case.
7. Gallery keeps the example; the new gallery test; ARCHITECTURE; the result
   table.

Before each commit, what CI runs (`.github/workflows/ci.yml`):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace --target wasm32-unknown-unknown
```

plus, for 5 and 7, `grep -c "egui::Area\|Cx::new" examples/gallery/src/lib.rs`.
The snapshot tests (`--features snapshot`) need a GPU and are not run here;
the phone layout has no snapshot today, and the wide layout does not change.

### 9.9 If something in 6 does not hold

- If a widget under a hidden leaf still reaches kittest's `query_by_label`
  in a way a gallery assertion notices, the assertion names a `<Text>` or the
  test is wrong about what it asserts; `<Text>` registers nothing under
  `hidden`, on both paths and in `Ui` mode. Check `Cx::is_hidden` is true on
  the surface the text was drawn from before touching the test.
- If taffy does not zero a hidden node's descendants (`compute_hidden_layout`
  in 0.9 does), the `content_rect` a hidden leaf's `Ui` is built from is the
  stale one; the leaf is invisible either way, so only the parity case would
  see it, and `hide()` on the lite side would then need a taffy-side twin.
- If step 6 grows past this (the `leaf` signature, `paint_texts`), it ships
  as a PR of its own after 1-5, as section 7 already allows; the gallery is no
  worse than today in between, and `<Overlay>`'s `is_hidden` line goes in with it.
