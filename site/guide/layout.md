---
title: Layout
---

# Layout

Layout is Flexbox and Grid as attributes. `<View>` is a node in a small
layout engine built on [taffy](https://github.com/DioxusLabs/taffy). egui
widgets are the leaves.

```rust
rsx! {
    <View direction="row" justify="space-between" align="center" gap={8} p={12}>
        <Text grow={1.0}>"Title"</Text>
        <Button on_click={|| *open = true}>"Open"</Button>
    </View>
}
```

A `<View>` inside a `<View>` joins the same tree. A `<View>` inside a plain
`egui::Ui` starts a new one.

## The attributes

Container attributes go on `<View>` and place its children. Item attributes go
on any element and place it inside its parent. Full list:
[Layout attributes](/reference/layout-attributes).

**Size**: `w` `h` `min_w` `min_h` `max_w` `max_h`. Numbers are egui points.
Strings can be `"auto"`, `"50%"`, `"12px"` or `"12"`.

```rust
<View w="100%" max_w={720.0} min_h={0.0}>..</View>
```

**Flex, container**: `direction` (`"row"`, `"column"`, `"row-reverse"`,
`"column-reverse"`), `wrap`, `justify` (main axis), `align` (cross axis),
`align_content`.

**Flex, item**: `grow`, `shrink`, `basis`, `align_self`.

```rust
<View direction="row" gap={8}>
    <View w={200.0} shrink={0.0}>"sidebar"</View>
    <View grow={1.0} min_w={0.0}>"the rest"</View>
</View>
```

Put `min_w={0.0}` on the column that should give way. Otherwise a wide child
pushes its neighbour off the edge instead of being clipped.

**Spacing**: `gap` on the container (one number, or `(column, row)`). Margin
and padding on the item: `m` `mx` `my` `mt` `mr` `mb` `ml`, `p` `px` `py`
`pt` `pr` `pb` `pl`. The more specific one wins.

**Grid**: `display="grid"` with `cols={n}` gives `n` equal columns.
`col_span` and `row_span` widen a child.

```rust
<View display="grid" cols={3} gap={8}>
    <Text col_span={2}>"wide"</Text>
    <Text>"narrow"</Text>
</View>
```

## `<Text>` and `<Label>`

Both draw text. `<Text>` is a layout node with `size`, `color`, `strong`,
`selectable` and `font` props. `<Label>` is `egui::Label` in a leaf. Prefer
`<Text>` inside a `<View>`.

Both default to no wrapping, so text in a flex row reports its full width.
Pass `wrap` to wrap, and give it a width to wrap inside:

```rust
<Text wrap w="100%">{long_paragraph}</Text>
```

## Painting the box

Any element can paint a box: `bg`, `border`, `radius`, `shadow`,
`custom_shadow`, `opacity`.

```rust
<View p={12} gap={8} bg={egui::Color32::from_gray(30)} radius={8.0} shadow>
    <Text>"a card"</Text>
</View>
```

Shadow and background go behind the content, border in front, all with one
`radius`. A border adds to the padding, so children start inside the stroke.

A widget that already paints a box (`<Button>`, `<TextEdit>`, `<ComboBox>`)
hands its box over when you paint one, so they do not stack. `p` on a
`<Button>` becomes the button's own padding.

Outside a `<View>`, `style` paints nothing. Use `<Frame>` there.

## Three traps

### A `ScrollArea` needs a height

A `ScrollArea` fills what it is given, so the layout must know how much that
is. Give it `grow` inside a container with a definite height, or a height of
its own:

```rust
<View direction="column" grow={1.0} min_h={0.0}>
    <Text strong>"header"</Text>
    <ScrollArea grow={1.0}>
        {rows}
    </ScrollArea>
</View>
```

Inside a column with no definite height, `grow` is not enough: the scroll
area asks for the whole window height, and the screen grows taller than the
window.

### Wrapping needs a width

`wrap` on `<Text>`, `<Label>` or `<View>` needs a width. Pair it with `w`, or
with `grow` under a parent with a definite width.

### `display="none"` is not `if`

```rust
// Hidden, still mounted: hooks keep running, state survives.
<View display={if showing { "flex" } else { "none" }}>
    <Preview/>
</View>

// Unmounted: state is dropped and effect cleanups run.
if showing {
    <Preview/>
}
```

`display="none"` gives the subtree a zero box and hides it from assistive
technology, but keeps it in the tree. Use it for a pane that must keep its
state. Use `if` when the state should go.

## See it running

[layout](/examples/layout) walks every flex and grid attribute.
[styles](/examples/styles) does the same for paint. [board](/examples/board)
is a real screen built from them. The engine is described in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-reactor/blob/main/docs/ARCHITECTURE.md#6-layout)
section 6.
