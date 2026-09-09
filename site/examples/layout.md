---
title: layout
---

# layout

Every flex and grid attribute `<View>` understands, one section each.

<script setup>
import { data } from '../examples.data.js'
</script>

<ExampleEmbed
  name="layout"
  :has-plain="data.layout.hasPlain"
  :react-lines="data.layout.reactLines"
  :plain-lines="data.layout.plainLines"
>

<div data-version="react">

::: example-source layout
:::

</div>

<div data-version="plain">

::: example-source layout plain
:::

</div>

</ExampleEmbed>

A visual table rather than an app: seven labelled sections, each drawing what
one group of attributes does — `direction`, `justify`, `align` with `grow`,
`wrap` with `gap`, nesting, `display="grid"` with `cols`, and egui's own
containers as leaves. The
[Layout attributes](/reference/layout-attributes) reference is the full list in
prose; this page is the part of it you can see.

Two things are being shown at once. The flex and grid sections are the layout
engine, which is [taffy](https://github.com/DioxusLabs/taffy) under an engine of
our own. The `<Grid>`, `<Row>`, `<Vertical>` and `<Label>` sections are egui's
*own* containers, kept as elements for the places where they are more familiar
or cheaper: their children are drawn in plain `Ui` mode, so layout attributes do
nothing inside them until a `<View>` starts a tree again.

The plain egui tab is worth opening on this one in particular. There is no
flexbox in egui, so the same pictures are produced by measuring and placing
things by hand — which is where the line-count difference comes from, and why
layout is the feature this library exists for.

## Run it yourself

```sh
cargo run -p layout
cargo run -p layout --bin layout-plain    # the plain egui version
trunk serve --config examples/layout/Trunk.toml
```

The source is [`examples/layout/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/layout/src/lib.rs)
and [`plain.rs`](https://github.com/fand/egui-react/blob/main/examples/layout/src/plain.rs).
