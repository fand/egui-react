---
title: rsx!
---

# rsx!

`rsx! { .. }` returns an `impl View`, which is in practice a closure that draws
when it is shown. Everything inside it is real Rust: the macro expands to egui
calls in place, which is why control flow is written with `if` and `for` rather
than with helpers.

## Elements

```rust
rsx! {
    <View direction="row" gap={8} align="center">
        <Text strong>"Title"</Text>
        <Button on_click={|| *open = true}>"Open"</Button>
    </View>
}
```

Every element name is a Rust function component, so `<Button/>` needs `Button`
in scope. There are no lowercase HTML tags. A path works too:

```rust
rsx! { <elements::Separator vertical/> }
```

The usual import is the two preludes:

```rust
use egui_react::prelude::*;
use egui_react_elements::prelude::*;
```

## Text and expressions

Text must be a string literal. Unquoted words are a compile error, on purpose:
it keeps `<Text>` children unambiguous.

```rust
rsx! {
    <Text>"a literal"</Text>
    <Text>{format!("{} items", items.len())}</Text>
    <Text>{name.as_str()}</Text>
}
```

`{expr}` embeds anything that is a `View`: `&str`, `String`, `Option<V>`,
`Vec<V>`, an array, another `rsx!`, or a closure taking `&mut Cx`.

## Attributes

Three kinds, and they are told apart by name:

- `on_*={handler}` is an event. See
  [Components and events](/guide/components-and-events).
- Layout and paint attributes — `w` `h` `min_w` `grow` `shrink` `basis` `m*`
  `p*` `align_self` `col_span` `row_span`, and `bg` `border` `radius` `shadow`
  `opacity` — are collected into the element's `style` prop. Every element
  takes them.
- Everything else is a prop setter on that component.

An attribute with no value is `true`:

```rust
rsx! {
    <Text strong wrap>"a strong, wrapping label"</Text>
    <Separator vertical/>
}
```

Layout values take numbers or CSS-ish strings, and both spellings mean the same
thing:

```rust
rsx! {
    <View w={200.0} p={12} gap={8}>
        <Text w="50%">"half"</Text>
    </View>
}
```

If you pass `style={expr}` *and* shorthands, the shorthands chain off the
expression — `<Chip style={style} p={6}/>` becomes `.style((style).p(6))` — so
a wrapper component can take its caller's layout and add to it. The full list
is in [Layout attributes](/reference/layout-attributes).

## Children

Children are whatever sits between the tags:

```rust
rsx! {
    <View direction="column">
        <Text>"first"</Text>
        <Text>"second"</Text>
    </View>
}
```

With no children the element gets `()`. A single string literal or a single
`{expr}` is passed as that expression itself, which is what lets
`<Button>"OK"</Button>` (children: `impl Into<WidgetText>`) and
`<View>..</View>` (children: `impl View`) share one syntax.

## Control flow

Write it directly. `if`, `else`, `for` and `match` all work inside `rsx!`:

```rust
rsx! {
    <View direction="column" gap={4}>
        if todos.is_empty() {
            <Text>"nothing to do"</Text>
        } else {
            for (i, todo) in todos.iter().enumerate() {
                <View key={i} direction="row" gap={8}>
                    <Text>{todo.text.as_str()}</Text>
                    <Button on_click={|| dispatch.send(Msg::Remove(i))}>"x"</Button>
                </View>
            }
        }
        match status {
            Status::Idle => { <Text>"idle"</Text> }
            Status::Busy(n) => { <Text>{format!("{n} left")}</Text> }
        }
    </View>
}
```

Note the braces around each `match` arm's body: an arm holds element nodes, not
an expression.

Because expansion is direct, none of this needs the `items.iter().map(|i|
rsx!{..})` shape that other Rust frameworks use — and it avoids the borrow
errors that shape causes.

## `key`

`key={expr}` is mixed into the element's scope id. Inside a `for`, any element
that owns hooks needs one, or every iteration asks for the same slot and the
collision detector complains:

```rust
for card in cards.iter() {
    <Card key={card.id} card={card}/>
}
```

The expression must be `Hash + Debug`. Changing a key is how you deliberately
reset a subtree: the old id is not visited, the sweep drops its state, and the
new one mounts clean.

## Dropping into plain egui

`{view(|cx| ..)}` is ordinary code in the middle of a tree:

```rust
rsx! {
    <View direction="column" gap={8}>
        <Text>"above"</Text>
        {view(|cx| {
            cx.leaf(&ItemStyle::default(), |ui| {
                ui.color_edit_button_srgba(colour.bind());
            });
        })}
    </View>
}
```

Hooks work in there, because it is the same `Cx`. Draw through `cx.leaf`, not
through `cx.ui()` — see [Escape hatches](/guide/escape-hatches) for why.

## Rules worth remembering

- Text is always quoted; there is no bare text node.
- An element's children are `.children(..)`; a component that wants children
  declares `children: impl View`.
- Handlers are generated and called immediately, so borrowing state with `&mut`
  in two sibling handlers is fine.
- `key` is required inside loops over components with hooks.

## See it running

[layout](/examples/layout) is one section per layout attribute,
[styles](/examples/styles) is one row per paint attribute with the code that
draws it, and [showcase](/examples/showcase) uses the whole syntax at once.

The macro's own rules are in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#32-view-and-rsx)
section 3.2.
