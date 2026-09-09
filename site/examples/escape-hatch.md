---
title: escape-hatch
---

# escape-hatch

Four ways down to plain egui: a closure, a leaf, a painter, and a nested Cx.

<script setup>
import { data } from '../examples.data.js'
</script>

<ExampleEmbed
  name="escape-hatch"
  :has-plain="data['escape-hatch'].hasPlain"
  :react-lines="data['escape-hatch'].reactLines"
  :plain-lines="data['escape-hatch'].plainLines"
>

<div data-version="react">

::: example-source escape-hatch
:::

</div>

</ExampleEmbed>

egui-react wraps a useful subset of egui, not all of it, and it never has to.
Everything is `&mut egui::Ui` in the end, so anything egui can do is one call
away. The four sections are the four routes: `{view(|cx| ..)}` for ordinary
code in the middle of a tree, `cx.leaf(&style, |ui| ..)` to draw egui where you
are, allocate-and-paint for drawing of your own, and a new `Cx` around an inner
`Ui` for hooks inside an egui container's closure.

The trap is in section 1, and it is the reason `leaf` exists. `cx.ui()` is safe
to *read* from anywhere, but inside a `<View>` it is the `Ui` the whole layout
tree was started in, not the position you are at — draw through it and the
widget lands outside the layout, in the tree's top-left corner. `cx.leaf` adds a
node and hands you the `Ui` for that node instead.

Two smaller things to notice. The progress bar uses `leaf_fill` rather than
`leaf`, with both axes given a size: a filling widget is sized by its style
instead of by what it drew, and an axis left `auto` claims the whole window,
because that is what "fill" means with nothing to measure. And the sparkline is
a plain function taking `&mut egui::Ui` — it knows nothing about this library,
which is exactly the point.

None of this is a workaround. The elements in `egui-react-elements` are written
with the same calls; there is no private door.
[Escape hatches](/guide/escape-hatches) walks through the same four with more
prose.

## Run it yourself

```sh
cargo run -p escape-hatch
trunk serve --config examples/escape-hatch/Trunk.toml
```

The source is [`examples/escape-hatch/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/escape-hatch/src/lib.rs).
