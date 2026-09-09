---
title: shader
---

# shader

A ray-traced black hole in a `<Canvas>`, with sliders wired to its uniform.

<script setup>
import { data } from '../examples.data.js'
</script>

<ExampleEmbed
  name="shader"
  :has-plain="data.shader.hasPlain"
  :react-lines="data.shader.reactLines"
  :plain-lines="data.shader.plainLines"
>

<div data-version="react">

::: example-source shader
:::

</div>

</ExampleEmbed>

Three pieces meet here, and each stays on its own side of the fence. The
pipeline is built once at startup by `Options::setup`, which eframe hands a
`CreationContext` with a wgpu render state in it; the resources go into
`callback_resources` under a type of their own, so they can be found again by
type and cannot clash with another app's. The [`<Canvas>`](/reference/elements)
element knows none of that: it asks the layout for a rect and calls `paint`
with it, and what goes in there is one line pushing an
`egui_wgpu::Callback` onto the painter. Everything in `shader.wgsl` is
invisible to the Rust above it.

State is the point. `speed`, `mass`, `bloom`, `tilt` and `paused` are
[`use_state`](/reference/hooks) like anywhere else; the values are copied into
the callback struct, written to a uniform buffer, and read by the WGSL. Move a
slider and a number in a hook changes, and the picture changes — nothing in
between has to be told. `mass` is the one to try first: it is the
Schwarzschild radius the shader traces photons around.

In a browser this runs on WebGPU where the browser has it, and falls back to
WebGL where it does not; both come from eframe's defaults, with nothing to
configure. [Web and native](/guide/web-and-native) has the `setup` hole in
context.

## Run it yourself

```sh
cargo run -p shader
trunk serve --config examples/shader/Trunk.toml
```

The source is [`examples/shader/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/shader/src/lib.rs);
the pipeline is in [`src/gpu.rs`](https://github.com/fand/egui-react/blob/main/examples/shader/src/gpu.rs)
and the shader in [`src/shader.wgsl`](https://github.com/fand/egui-react/blob/main/examples/shader/src/shader.wgsl).
