---
title: Layout attributes
---

# Layout attributes

Every element takes a `style` prop of type `ItemStyle`, and `rsx!` fills it
from the attributes below. `<View>` additionally takes the container
attributes, which describe how its *children* are placed. All of it lives in
`egui_reactor::layout` and is re-exported from the prelude.

```rust
<View direction="column" gap={8} p={12} bg={egui::Color32::from_gray(30)} radius={8.0}>
    <Text grow={1.0} w="100%">"item"</Text>
</View>
```

## Lengths

Any size, margin or padding is a `Length`:

| Written as | Means |
|---|---|
| `{12}` / `{12.0}` | 12 egui points |
| `"12"` / `"12px"` | the same |
| `"50%"` | half of the containing block |
| `"auto"` | sized by the layout algorithm |

Anything else in a string panics with a message naming what it expected, at the
moment the style is built. `From<f32>` and `From<i32>` give points, which is
why number literals need no unit.

## Item attributes (`ItemStyle`)

Accepted by every element.

| Attribute | Type | Meaning |
|---|---|---|
| `w` `h` | `Length` | Width, height |
| `min_w` `min_h` | `Length` | Minimum width, height |
| `max_w` `max_h` | `Length` | Maximum width, height |
| `grow` | `f32` | `flex-grow`: share of the leftover space |
| `shrink` | `f32` | `flex-shrink`: share of the overflow to give up |
| `basis` | `Length` | `flex-basis`: the size to start from |
| `align_self` | `Align` | Overrides the parent's `align` for this child |
| `m` `mx` `my` `mt` `mr` `mb` `ml` | `Length` | Margin: all, x, y, top, right, bottom, left |
| `p` `px` `py` `pt` `pr` `pb` `pl` | `Length` | Padding, same shape |
| `col_span` `row_span` | `u16` | How many grid tracks this child covers |

The margin and padding shorthands go all → axis → side, and the more specific
one wins: `p={8} pt={0}` is eight points everywhere but the top.

`min_w={0.0}` is worth knowing by name. A flex item's automatic minimum is its
content, so a wide child pushes its neighbour off the edge instead of being
clipped; setting the minimum to zero on the item that should give way is the
fix.

## Container attributes (`ContainerStyle`)

Accepted by `<View>` only.

| Attribute | Type | Values |
|---|---|---|
| `display` | `Display` | `"flex"` (default), `"grid"`, `"block"`, `"none"` |
| `direction` | `Direction` | `"row"` (default), `"column"`, `"row-reverse"`, `"column-reverse"` |
| `wrap` | `bool` | Wrap children onto more lines |
| `justify` | `Justify` | `"normal"` (default), `"start"`, `"end"`, `"flex-start"`, `"flex-end"`, `"center"`, `"stretch"`, `"space-between"`, `"space-evenly"`, `"space-around"` |
| `align` | `Align` | `"normal"` (default), `"start"`, `"end"`, `"flex-start"`, `"flex-end"`, `"center"`, `"baseline"`, `"stretch"` |
| `align_content` | `Justify` | The same values, distributing wrapped lines on the cross axis |
| `gap` | `Gap` | One number for both axes, or `(column, row)` |
| `cols` | `u16` | Equal-width grid columns; only with `display="grid"` |

`justify` and `align` default to `Normal`, which means "unspecified" and leaves
the underlying engine's own default in place — it is not the same as `"start"`.

An invalid string panics with the list of accepted spellings, so a typo shows
up on the first frame that draws the element rather than as a silent
mislayout.

## Paint attributes (`PaintStyle`)

Carried inside `ItemStyle`, so every element accepts them too.

| Attribute | Type | Meaning |
|---|---|---|
| `bg` | `egui::Color32` | Background, behind the content |
| `border` | `egui::Stroke` | Border, in front of the content, inside the box |
| `radius` | `f32` | Corner radius, shared by background, border and shadow |
| `shadow` | `bool` | The theme's window shadow |
| `custom_shadow` | `egui::Shadow` | A shadow of your own; wins over `shadow` |
| `opacity` | `f32` | Multiplies the opacity of everything the node draws |

All three shapes sit on the node's border box — its whole box, not the rect its
content draws in. **A border is also layout**: its width is added to the padding
edge, so children start inside the stroke and it is painted exactly in the band
the layout kept them out of. Nothing else about the paint reaches the layout.

Painting happens after the frame's layout is solved, so a container that grew
around children that stayed put is painted at its new size in the same pass.
Outside a `<View>` (in plain `Ui` mode) nothing is painted from `style`;
`<Frame>` is the escape hatch there.

## `style=` and the shorthands together

`style={expr}` and the shorthand attributes fill the same prop, and if both are
present the shorthands chain off the expression:

```rust
// becomes .style((style).p(6))
<Chip style={style} p={6}/>
```

That is what lets a wrapper component take `#[prop(default)] style: ItemStyle`,
receive its caller's layout untouched, and add its own on top:

```rust
#[component]
fn Chip(cx: &mut Cx, #[prop(default)] style: ItemStyle, label: &str) {
    let clicked = cx.leaf(&style.shrink(0.0), |ui| ui.button(label).clicked());
    // ..
}
```

The same types are usable by hand — `ItemStyle::default().w(220.0).h(20.0)`,
`ContainerStyle::default().direction("column")` — which is what the escape
hatches take.

## More

The full attribute list, the taffy mapping, and what happens to each attribute
inside a `<VirtualList>` row are in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-reactor/blob/main/docs/ARCHITECTURE.md#layout-attributes)
section 6.
