---
title: rsx!
---

# rsx!

`rsx! { .. }` returns an `impl View`: a closure that draws when shown. The
macro expands into egui calls in place, so control flow is plain `if` and
`for`.

## Elements

```rust
rsx! {
    <View direction="row" gap={8} align="center">
        <Text strong>"Title"</Text>
        <Button on_click={|| *open = true}>"Open"</Button>
    </View>
}
```

Every element is a Rust function component, so `<Button/>` needs `Button` in
scope. There are no HTML tags. Paths work too: `<elements::Separator vertical/>`.

The usual imports:

```rust
use egui_react::prelude::*;
use egui_react_elements::prelude::*;
```

## Text and expressions

Text must be a string literal. Bare words are a compile error.

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

Three kinds, told apart by name:

- `on_*={handler}` is an event. See
  [Components and events](/guide/components-and-events).
- Layout and paint attributes (`w`, `h`, `grow`, `p`, `m`, `bg`, `border`,
  `radius`, ...) go into the element's `style` prop. Every element takes them.
  Full list: [Layout attributes](/reference/layout-attributes).
- Everything else is a prop of that component.

An attribute with no value is `true`:

```rust
rsx! {
    <Text strong wrap>"a strong, wrapping label"</Text>
    <Separator vertical/>
}
```

Layout values take numbers or CSS-like strings:

```rust
rsx! {
    <View w={200.0} p={12} gap={8}>
        <Text w="50%">"half"</Text>
    </View>
}
```

`style={expr}` plus shorthands chain: `<Chip style={style} p={6}/>` becomes
`.style((style).p(6))`. A wrapper component can take its caller's layout and
add to it.

## Children

Children are whatever sits between the tags. No children means `()`. A single
literal or `{expr}` is passed as that expression itself. That is why
`<Button>"OK"</Button>` (children: `impl Into<WidgetText>`) and
`<View>..</View>` (children: `impl View`) look the same.

## Control flow

`if`, `else`, `for` and `match` work inside `rsx!`:

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

Each `match` arm body needs braces: an arm holds elements, not an expression.

No `items.iter().map(|i| rsx!{..})` is needed, and the borrow errors that
shape causes do not come up.

## `key`

`key={expr}` is mixed into the element's id. Inside a `for`, any element with
hooks needs one. Without it, every iteration hits the same slot and the
collision detector complains.

```rust
for card in cards.iter() {
    <Card key={card.id} card={card}/>
}
```

The key must be `Hash + Debug`. Changing a key resets the subtree: the old id
is dropped, the new one mounts fresh.

## Dropping into plain egui

`{view(|cx| ..)}` is plain code in the middle of a tree:

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

Hooks work in there, since it is the same `Cx`. Draw through `cx.leaf`, not
`cx.ui()`. See [Escape hatches](/guide/escape-hatches).

## See it running

[layout](/examples/layout) and [styles](/examples/styles) show one attribute
at a time. [notes](/examples/notes) uses the whole syntax. The macro's rules
are in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#32-view-and-rsx)
section 3.2.
