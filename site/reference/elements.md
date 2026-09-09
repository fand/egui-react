---
title: Elements
---

# Elements

Everything on this page comes from `egui_react_elements::prelude`, which also
exports the generated event enums — you need `ButtonEvent` in scope wherever
you write `<Button on_click=../>`.

Every element takes a `style` prop (except `<Window>` and `<Row>`, which have
no box in the layout), and `rsx!` fills it from the layout and paint attributes
listed in [Layout attributes](/reference/layout-attributes). Those are not
repeated in the tables below.

## Layout

### `<View>`

The flex or grid container: one layout node, with every child a node.

| Prop | Type | Meaning |
|---|---|---|
| `display` | `Display` | `"flex"` (default), `"grid"`, `"block"`, `"none"` |
| `direction` | `Direction` | `"row"` (default), `"column"`, `"row-reverse"`, `"column-reverse"` |
| `wrap` | `bool` | Wrap children onto more lines |
| `justify` | `Justify` | Main axis: `"start"`, `"center"`, `"end"`, `"space-between"`, `"space-evenly"`, `"space-around"`, `"stretch"` |
| `align` | `Align` | Cross axis: `"start"`, `"center"`, `"end"`, `"baseline"`, `"stretch"` |
| `align_content` | `Option<Justify>` | Cross-axis distribution of wrapped lines |
| `gap` | `Gap` | One number, or `(column, row)` |
| `cols` | `Option<u16>` | Equal-width columns; only with `display="grid"` |
| `children` | `impl View` | |

```rust
<View direction="row" justify="space-between" align="center" gap={8} p={12}>
    <Text grow={1.0}>"Title"</Text>
    <Button on_click={|| *open = true}>"Open"</Button>
</View>
```

Used in: every example.

### `<Text>`

A text node the layout engine measures and paints itself — no `Ui` and no
`Label` of its own, which makes it the cheapest way to put text in a tree.

| Prop | Type | Meaning |
|---|---|---|
| `size` | `Option<f32>` | Font size in points |
| `color` | `Option<egui::Color32>` | |
| `strong` | `bool` | |
| `wrap` | `bool` | Off by default, so a text reports its full width instead of collapsing; needs a width to wrap inside |
| `selectable` | `Option<bool>` | Overrides the style's `interaction.selectable_labels` |
| `font` | `Option<&str>` | The name of a registered font stack, or `"proportional"` / `"monospace"` |
| `children` | `impl Into<egui::WidgetText>` | |

```rust
<Text size={32.0} strong>{format!("{}", *count)}</Text>
<Text wrap w="100%" font="ui">{paragraph}</Text>
```

Used in: [counter](/examples/counter), [board](/examples/board),
[font](/examples/font), and most others.

## Widgets

Each of these draws one egui widget as a layout leaf.

### `<Button>`

| Prop | Type | Meaning |
|---|---|---|
| `enabled` | `bool` | `true` by default |
| `label` | `Option<&str>` | The accessible name, when the children are an icon or a symbol |
| `children` | `impl Into<egui::WidgetText>` | What is drawn |
| `on_click` | event `()` | |

```rust
<Button label="close menu" on_click={|| *open = false}>"×"</Button>
```

`label` does not change what is drawn; it replaces the name in the
accessibility tree. A button whose child is `"×"` would otherwise be read out
as "times", so pass it. `p` on a `<Button>` becomes the widget's own padding
when it is symmetric and in points, so the box that takes the press is the box
you painted.

Used in: [counter](/examples/counter), [todo](/examples/todo),
[form](/examples/form), and most others.

### `<Label>`

`egui::Label` in a leaf. `<Text>` is usually the better choice inside a
`<View>`; `<Label>` is what you want inside egui's own containers.

| Prop | Type | Meaning |
|---|---|---|
| `wrap` | `bool` | Off by default, as for `<Text>` |
| `children` | `impl Into<egui::WidgetText>` | |

Used in: [layout](/examples/layout).

### `<TextEdit>`

| Prop | Type | Meaning |
|---|---|---|
| `bind` | `&mut String` | The text. Pass `state.bind()` |
| `multiline` | `bool` | |
| `hint` | `Option<&str>` | Placeholder text |
| `desired_width` | `Option<f32>` | Overrides the width the layout would give it |
| `rows` | `Option<usize>` | Rows, for a multiline edit |
| `clear_on_submit` | `bool` | Empty the field after `on_submit` |
| `on_change` | event `()` | The text changed this frame |
| `on_submit` | event `String` | Enter was pressed; the payload is the text |

```rust
<TextEdit grow={1.0} bind={draft.bind()} hint="what needs doing" clear_on_submit
    on_submit={|text: String| dispatch.send(Msg::Add(text))}/>
```

Inside a `<View>` it fills its node: single-line takes the node's width,
multiline fills both ways, unless `desired_width` or `rows` says otherwise.

Used in: [todo](/examples/todo), [form](/examples/form),
[fetch](/examples/fetch), [board](/examples/board),
[spreadsheet](/examples/spreadsheet).

### `<Checkbox>`

| Prop | Type | Meaning |
|---|---|---|
| `bind` | `&mut bool` | |
| `label` | `Option<&str>` | |
| `on_change` | event `bool` | The new value |

```rust
<Checkbox bind={done.bind()} label="done"/>
```

Used in: [form](/examples/form), [todo](/examples/todo),
[clock](/examples/clock), [shader](/examples/shader).

### `<Slider>`

Generic over `T: egui::emath::Numeric` — the integer and float types — and `T`
is inferred from what you bind.

| Prop | Type | Meaning |
|---|---|---|
| `bind` | `&mut T` | |
| `range` | `RangeInclusive<T>` | |
| `label` | `Option<&str>` | |
| `on_change` | event `()` | |

```rust
<Slider bind={amount.bind()} range={0.0..=1.0} label="amount"/>
```

Used in: [form](/examples/form), [shader](/examples/shader),
[list-10k](/examples/list-10k), [escape-hatch](/examples/escape-hatch).

### `<ComboBox>`

| Prop | Type | Meaning |
|---|---|---|
| `bind` | `&mut usize` | The index of the selected option |
| `options` | `&[S]` where `S: AsRef<str>` | |
| `label` | `Option<&str>` | |
| `on_change` | event `usize` | The new index |

Used in: [form](/examples/form), [patch](/examples/patch).

### `<Image>`

| Prop | Type | Meaning |
|---|---|---|
| `source` | `egui::ImageSource<'_>` | `egui::include_image!(..)`, a URI, or bytes |
| `fit` | `Option<egui::Vec2>` | The size to fit into |
| `alt` | `Option<&str>` | The accessible name; also drawn beside the warning sign when loading fails |

`alt` is worth filling in for the same reason as `label` on `<Button>`.

### `<Separator>`

| Prop | Type | Meaning |
|---|---|---|
| `vertical` | `bool` | Horizontal by default |

Used in: [todo](/examples/todo), [font](/examples/font),
[theme](/examples/theme), and others.

## Containers

### `<ScrollArea>`

| Prop | Type | Meaning |
|---|---|---|
| `horizontal` | `bool` | Scroll sideways too |
| `vertical` | `bool` | `true` by default |
| `max_h` | `Option<f32>` | A ceiling on the height it asks for |
| `children` | `impl View` | |

```rust
<ScrollArea grow={1.0}>
    {rows}
</ScrollArea>
```

It fills the space it is given rather than reporting a size, so it needs
`grow` under a parent with a definite height, or a height of its own — see the
trap in [Layout](/guide/layout). For long lists use `<VirtualList>`.

Used in: [board](/examples/board), [notes](/examples/notes),
[fetch](/examples/fetch), [font](/examples/font), [patch](/examples/patch).

### `<VirtualList>`

Draws only the rows in view and reserves the height of the rest, so frame time
does not depend on the row count.

| Prop | Type | Meaning |
|---|---|---|
| `rows` | `usize` | How many rows there are |
| `row_h` | `f32` | Every row is laid out into exactly this height |
| `row_w` | `Option<f32>` | The width each row is laid out into, and the sideways scroll range. Defaults to the viewport width |
| `horizontal` | `bool` | Scroll sideways |
| `render` | `impl for<'a, 's, 'u> FnMut(&'a mut Cx<'s, 'u>, usize)` | Draws row `i` |
| `on_scroll` | event `egui::Vec2` | The list's scroll offset, after the rows are drawn |

```rust
<VirtualList
    grow={1.0}
    rows={filtered.len()}
    row_h={24.0}
    render={|cx: &mut Cx<'_, '_>, row: usize| {
        rsx! { <Text>{filtered[row].as_str()}</Text> }.show(cx);
    }}
/>
```

Rows are drawn inside a scope of their own, so a row may have hooks, like
`for` plus `key={i}`. Every row must be the same height. Spell the `render`
bound out as above rather than eliding it: an elided lifetime in a prop is
rewritten to the props struct's own, which does not compile here.

`on_scroll` fires every frame, so a handler that stores the offset should write
only when it differs — a write per frame asks for a repaint per frame.

Used in: [list-10k](/examples/list-10k), [spreadsheet](/examples/spreadsheet).

### `<Collapsing>`

| Prop | Type | Meaning |
|---|---|---|
| `header` | `&str` | |
| `default_open` | `bool` | |
| `children` | `impl View` | |

Used in: [todo](/examples/todo), [form](/examples/form),
[shell](/examples/shell).

### `<Frame>`

| Prop | Type | Meaning |
|---|---|---|
| `children` | `impl View` | |

No props of its own. Inside a tree it is a `<View>` with paint plus one `Ui`;
over a plain `Ui` it builds an `egui::Frame` from `style` — fill, stroke, corner
radius, shadow, and `p` as the inner margin — which is the one thing a `<View>`
cannot do there.

### `<Window>`

A floating egui window. It has no `style`: it is a layer of its own and takes
no space in the layout.

| Prop | Type | Meaning |
|---|---|---|
| `title` | `&str` | |
| `open` | `Option<&mut bool>` | Adds a close button and writes `false` into it |
| `resizable` | `bool` | `true` by default |
| `default_pos` | `Option<egui::Pos2>` | |
| `default_size` | `Option<egui::Vec2>` | |
| `children` | `impl View` | |

Not drawing a `<Window>` — or `open={false}` — unmounts its children.

Used in: [notes](/examples/notes), [shell](/examples/shell).

### `<Overlay>`

An `egui::Area` as an element: its own layer, over everything below it, taking
no space in the surrounding layout.

| Prop | Type | Meaning |
|---|---|---|
| `anchor` | `Option<Anchor>` | `"top-left"`, `"top"`, `"top-right"`, `"left"`, `"center"`, `"right"`, `"bottom-left"`, `"bottom"`, `"bottom-right"` (egui's `"right-bottom"` order is accepted too) |
| `offset` | `(f32, f32)` | Moves it off that anchor |
| `pos` | `Option<egui::Pos2>` | Places it by hand; wins over `anchor` |
| `order` | `Order` | `"background"`, `"middle"`, `"foreground"` (default), `"tooltip"` |
| `constrain` | `bool` | Keep it inside the window; `true` by default |
| `top` | `bool` | Lift it above the other overlays every frame |
| `fill` | `Option<egui::Color32>` | The sheet colour |
| `children` | `impl View` | |

With `w` and/or `h` it is **sized**: it paints a sheet, takes every press that
lands on it, and roots a layout tree of its own, so a `<View w="100%" h="100%">`
inside fills it. With neither it is **unsized**: as big as its children, paints
nothing unless `fill` is given, and lets every press beside them through.

```rust
<Overlay anchor="bottom-right" offset={(-16.0, -16.0)}>
    <Button label="new note" px={16.0} py={12.0} radius={24.0} shadow
        on_click={|| *composing = true}>"+"</Button>
</Overlay>
```

### `<Panel>` and `<CentralPanel>`

Docked panels. A panel carves its space out of the nearest egui `Ui` — the one
that started the current tree, which under the runner is the window itself — so
a `<Panel>` written deep inside a `<View>` still flies to the window edge. That
is what docking means.

| Prop | Type | Meaning |
|---|---|---|
| `side` | `Side` | `"left"` (default), `"right"`, `"top"`, `"bottom"` |
| `default_size` | `Option<f32>` | Width or height, depending on the side |
| `resizable` | `bool` | `true` by default |
| `children` | `impl View` | |

`<CentralPanel>` takes `style` and `children` only, and fills what the docked
panels left.

Used in: [shell](/examples/shell), which is the one example that does not run
in a browser tab here for exactly this reason.

### `<Vertical>`, `<Horizontal>`, `<Grid>` and `<Row>`

egui's own containers, kept as leaves for the places where they are cheaper or
more familiar. Their children are drawn in plain `Ui` mode, not in the layout
tree.

| Element | Props |
|---|---|
| `<Vertical>` | `children` |
| `<Horizontal>` | `children` |
| `<Grid>` | `cols: Option<usize>`, `striped: bool`, `children` |
| `<Row>` | `children` — one row of a `<Grid>` |

```rust
<Grid cols={2}>
    <Row>
        <Label>"grid a1"</Label>
        <Label>"grid b1"</Label>
    </Row>
</Grid>
```

Used in: [layout](/examples/layout).

## Drawing

### `<Canvas>`

A leaf that draws nothing itself: it reserves the rect the layout gave it and
hands it to you.

| Prop | Type | Meaning |
|---|---|---|
| `sense` | `egui::Sense` | `Sense::hover()` by default; `on_drag` needs `Sense::drag()` |
| `paint` | `impl for<'a> FnOnce(&'a mut egui::Ui, egui::Rect)` | Draws into the rect |
| `on_drag` | event `egui::Vec2` | The drag delta |
| `on_hover` | event `egui::Pos2` | The pointer position within the rect |

```rust
<Canvas w="100%" h={240.0} paint={move |ui: &mut egui::Ui, rect: egui::Rect| {
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(rect, callback));
}}/>
```

The prop is `paint`, not `on_paint`: `rsx!` treats every attribute starting with
`on_` as an event. Spell the bound out rather than eliding it, for the same
reason as `render` on `<VirtualList>`.

Used in: [shader](/examples/shader), [patch](/examples/patch).

## Async

### `<Suspense>`

| Prop | Type | Meaning |
|---|---|---|
| `fallback` | `impl View` | Drawn while anything below is pending |
| `children` | `impl View` | |

```rust
<Suspense fallback={view(|cx| { cx.ui().spinner(); })}>
    <Response url={url.as_str()} attempt={*attempt}/>
</Suspense>
```

Draws `fallback` instead of its children while even one `use_future` below it
is pending. See [Async](/guide/async).

Used in: [fetch](/examples/fetch), [patch](/examples/patch).

## Accessibility

`label` on `<Button>` and `alt` on `<Image>` are the names assistive technology
reads. `label` does not change what is drawn — the children do that — so an
icon button needs one. They work on native today, and on the web through the
DOM mirror described in [Web and native](/guide/web-and-native).

## More

The elements list with the reasoning behind each wrapper is in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#elements-list-egui-react-elements)
section 6.
