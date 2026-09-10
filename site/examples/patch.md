---
title: patch
---

<script setup>
import { data } from '../examples.data.js'
</script>

<ExamplePage
  name="patch"
  :has-plain="data.patch.hasPlain"
  :react-lines="data.patch.reactLines"
  :plain-lines="data.patch.plainLines"
>

<template v-slot:summary>

A node editor that generates, validates and previews its own WGSL shader.

</template>

<template v-slot:code>

<div data-version="react">

::: example-source patch
:::

</div>

</template>

The picture in the corner is not drawn by the editor; it is drawn by a
fragment shader that the patch above it *is*. Wire two nodes together and a new WGSL program is
generated, validated by naga on the CPU, and compiled. Move a slider and
nothing is generated at all: four floats reach a uniform buffer and the same
program draws a different picture.

That difference is the whole example, and it is one decision made in one place.
Every message bumps either `topology_rev` (the program could change) or
`param_rev` (only the numbers could), and two
[`use_memo`](/reference/hooks) calls hang off those counters — one producing
the WGSL, one producing the uniform block. Nothing watches for "did the source
change"; it falls out of what the deps of each memo are, and a test pins it.
Validation happening inside a memo is the point: an invalid graph is reported
in the editor rather than at the GPU.

Nodes sit at absolute positions, which `ItemStyle` has no attribute for and
does not need. The canvas is a single leaf that allocates its rectangle and, for
each node, opens a child `Ui` at that node's coordinates and builds a `Cx`
around it — the [escape hatch](/guide/escape-hatches) one level up. Inside a
node it is ordinary `<View>` flexbox again. The nodes are keyed by hand with
`cx.scope(node.id, ..)`, which is exactly what `key=` does, so a node's own
state survives deleting or reordering another node.

The picture is the background: the canvas draws the output shader across
itself, contained rather than cropped, and the patch sits on top of it. Every
parameter is edited in the node that owns it, so nothing has to be kept in
step with the selection; what floats over the canvas in an
[`<Overlay>`](/reference/elements) is only what belongs to no node — the node
count in one corner, and naga's complaint in the other when there is one. The
view fits itself to the patch the first time it is drawn, and `Recenter` puts
it back there. The menu bar is one leaf of plain egui, because a menu opens as
a layer of its own and takes no room in the layout; it holds the clock as
well, a play button and a timeline over the one-minute loop the picture is
drawn at. The picture itself is an `egui_wgpu` paint callback: WebGPU in a
browser that has it, and eframe's WebGL fallback where it does not.

## Run it yourself

```sh
cargo run -p patch
trunk serve --config examples/patch/Trunk.toml
```

The source is [`examples/patch/src/lib.rs`](https://github.com/fand/egui-reactor/blob/main/examples/patch/src/lib.rs).

</ExamplePage>
