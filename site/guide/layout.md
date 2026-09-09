---
title: Layout
---

# Layout

Layout is Flexbox and Grid, written as attributes. `<View>` is a node in a small
layout engine of our own built on
[taffy](https://github.com/DioxusLabs/taffy); egui widgets are the leaves.

```rust
rsx! {
    <View direction="row" justify="space-between" align="center" gap={8} p={12}>
        <Text grow={1.0}>"Title"</Text>
        <Button on_click={|| *open = true}>"Open"</Button>
    </View>
}
```

A `<View>` inside another `<View>` is a node in the same tree; a `<View>`
inside a plain `egui::Ui` starts a new tree there.

## The attributes, by group

Container attributes go on `<View>` and describe how its children are placed.
Item attributes go on *any* element and describe that element inside its
parent. Both lists are exhaustive in
[Layout attributes](/reference/layout-attributes); this is what you reach for
day to day.

**Size** — `w` `h` `min_w` `min_h` `max_w` `max_h`. Numbers are egui points;
strings can be `"auto"`, `"50%"`, `"12px"` or `"12"`.

```rust
<View w="100%" max_w={720.0} min_h={0.0}>..</View>
```

**Flex, on the container** — `direction` (`"row"`, `"column"`, and the
`-reverse` pair), `wrap`, `justify` (main axis: `"start"`, `"center"`,
`"space-between"`, `"space-evenly"`, `"space-around"`, `"stretch"`, `"end"`),
`align` (cross axis: `"start"`, `"center"`, `"end"`, `"baseline"`,
`"stretch"`), `align_content`.

**Flex, on the item** — `grow`, `shrink`, `basis`, `align_self`.

```rust
<View direction="row" gap={8}>
    <View w={200.0} shrink={0.0}>"sidebar"</View>
    <View grow={1.0} min_w={0.0}>"the rest"</View>
</View>
```

`min_w={0.0}` on the column that should give way is worth knowing: the layout
may otherwise take a flex item's content as its automatic minimum, and a wide
child pushes its neighbour off the edge instead of being clipped itself.

**Spacing** — `gap` on the container (one number, or `(column, row)`), and
margin and padding on the item: `m` `mx` `my` `mt` `mr` `mb` `ml`, `p` `px`
`py` `pt` `pr` `pb` `pl`. The shorthands go all → axis → side, and the more
specific one wins.

**Grid** — `display="grid"` with `cols={n}` gives `n` equal-width columns;
`col_span` and `row_span` widen a child.

```rust
<View display="grid" cols={3} gap={8}>
    <Text col_span={2}>"wide"</Text>
    <Text>"narrow"</Text>
</View>
```

## `<Text>` and `<Label>`

Both draw text. `<Text>` is a layout node holding a galley — the engine
measures it and paints it, with no `Ui` of its own — and it has `size`,
`color`, `strong`, `selectable` and `font`. `<Label>` is `egui::Label` in a
leaf. Prefer `<Text>` inside a `<View>`; it is the cheaper of the two and the
one with the props.

Both default to *not* wrapping, so that a text inside a flex row reports its
full width instead of collapsing to one character per line. Pass `wrap` to get
egui's usual wrapping — and give it a width to wrap inside, with `w` or with
`grow` inside a container that has a definite width:

```rust
<Text wrap w="100%">{long_paragraph}</Text>
```

## Painting the box

Any element can be a painted box, through the same `style` prop: `bg`,
`border`, `radius`, `shadow`, `custom_shadow`, `opacity`.

```rust
<View p={12} gap={8} bg={egui::Color32::from_gray(30)} radius={8.0} shadow>
    <Text>"a card"</Text>
</View>
```

The shadow and background go behind the content, the border in front, all three
on the node's border box and sharing one `radius`. A border is also layout: its
width is added to the padding edge, so children start inside the stroke rather
than under it.

A widget that already paints a box of its own — `<Button>`, `<TextEdit>`,
`<ComboBox>` — hands its box over as soon as you paint one, so the two do not
stack. `p` on a `<Button>` becomes the button's own padding when it is
symmetric and in points, so the pill it draws, the area that takes the press
and the box the layout reserved are one rect.

Outside a `<View>` (in plain `Ui` mode) nothing is painted from `style`;
`<Frame>` is the escape hatch there, building an `egui::Frame` out of the same
attributes.

## Three traps

### A `ScrollArea` needs a height

A `ScrollArea` fills the space it is given rather than reporting a size, so the
layout has to be told what that space is. Give it `grow` inside a container
whose height is definite, or a height of its own:

```rust
<View direction="column" grow={1.0} min_h={0.0}>
    <Text strong>"header"</Text>
    <ScrollArea grow={1.0}>
        {rows}
    </ScrollArea>
</View>
```

Inside a `<View direction="column">` with no definite height, neither `grow`
nor `basis={0}` is enough: a filling leaf's max-content is "the height of this
tree's root rect", so the scroll area asks for the whole window height and the
screen becomes taller than the window. Either give it a definite height, or
make sure nothing sits below it that would be pushed out.

### Wrapping needs a width

`wrap` on `<Text>` or `<Label>`, and `wrap` on a `<View>`, all need something
to wrap inside. Pair them with `w`, or with `grow` under a parent that has a
definite width.

### `display="none"` is not `if`

```rust
// Hidden, still mounted: hooks keep running, state survives.
<View display={if showing { "flex" } else { "none" }}>
    <Preview/>
</View>

// Unmounted: the sweep drops its state and runs its effect cleanups.
if showing {
    <Preview/>
}
```

`display="none"` gives the node and everything under it a zero box and hides it
from assistive technology, but keeps it in the tree, so keys and child indices
do not move and every component inside keeps its hooks. Use it for a pane that
must keep its state while something else is on screen; use `if` when the state
should go.

## See it running

[layout](/examples/layout) walks every flex and grid attribute one section at a
time, [styles](/examples/styles) does the same for the paint attributes, and
[board](/examples/board) is a real screen built from them, scroll areas
included.

The engine itself — one tree per root `<View>`, why a node is a rect and not a
`Ui`, and when a frame costs a second pass — is in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#6-layout)
section 6.
