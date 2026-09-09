---
title: patch
---

# patch

A node editor that generates, validates and previews its own WGSL shader.

<script setup>
import { data } from '../examples.data.js'
</script>

<ExampleEmbed
  name="patch"
  :has-plain="data.patch.hasPlain"
  :react-lines="data.patch.reactLines"
  :plain-lines="data.patch.plainLines"
>

<div data-version="react">

::: example-source patch
:::

</div>

</ExampleEmbed>

The picture is not drawn by the editor; it is drawn by a fragment shader that
the patch on the left *is*. Wire two nodes together and a new WGSL program is
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

The preview draws through [`<Canvas>`](/reference/elements) and an
`egui_wgpu` paint callback. In a browser that is WebGPU where the browser has
it, and eframe's WebGL fallback where it does not.

## Run it yourself

```sh
cargo run -p patch
trunk serve --config examples/patch/Trunk.toml
```

The source is [`examples/patch/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/patch/src/lib.rs).
