---
title: styles
---

<script setup>
import { data } from '../examples.data.js'
</script>

<ExamplePage
  name="styles"
  :has-plain="data.styles.hasPlain"
  :react-lines="data.styles.reactLines"
  :plain-lines="data.styles.plainLines"
>

<template v-slot:summary>

Every `style` attribute in a table: the name, the code that uses it, and what it
draws.

</template>

<template v-slot:code>

<div data-version="react">

::: example-source styles
:::

</div>

</template>

One row per attribute: its name, the code, and the picture that code produces.
Every attribute goes through the one `style` prop that every element takes —
the layout half (`w`, `grow`, `p` and the rest) resolved by the layout engine,
and the paint half (`bg`, `border`, `radius`, `shadow`, `custom_shadow`,
`opacity`) painted by it on the node's own box.

The middle column is the detail worth noticing. It is not typed in by hand:
each row is written once, as the element it draws, and a macro `stringify!`s
the same tokens for the code column — so the text and the picture cannot drift
apart. It is also a small demonstration of something in the macro's own rules,
since an element handed through a `macro_rules!` still resolves to the `rsx!`
closure it sits in.

Two paint behaviours are easiest to understand here rather than from prose. A
border is layout: its width goes onto the padding edge, so children start
inside the stroke and it is drawn exactly in the band they were kept out of. And
a widget that paints a box of its own — a `<Button>`, a `<TextEdit>` — hands
that box over as soon as you paint one, so the two never stack.
[Layout attributes](/reference/layout-attributes) has the tables.

## Run it yourself

```sh
cargo run -p styles
trunk serve --config examples/styles/Trunk.toml
```

The source is [`examples/styles/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/styles/src/lib.rs).

</ExamplePage>
