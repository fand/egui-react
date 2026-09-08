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

`Anchor` and `Order` are `str_enum!`s in `egui_react::layout`, next to
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
  `grow` works in it. `root_style()` moves from `egui-react-app` to
  `egui_react::layout` (or is rebuilt inline: column, `w 100% h 100%`) so the
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

### Tests (`crates/egui-react-elements/tests/overlay.rs`)

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

Test (`crates/egui-react/tests/animate.rs`, kittest with `step_dt` 0.05):
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
A test in `crates/egui-react/tests/` (`hidden.rs`): a `use_state` counter
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
