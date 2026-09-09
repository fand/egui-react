---
title: shader
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="shader"
  :has-plain="data.shader.hasPlain"
  :react-lines="data.shader.reactLines"
  :plain-lines="data.shader.plainLines"
>

<template v-slot:summary>

`<Canvas>` の中でレイトレースしたブラックホール。スライダーが uniform につながる。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source shader
:::

</div>

</template>

3 つの部品がここで出会い、それぞれ自分の側にとどまっています。パイプラインは起動時に `Options::setup` が 1 回だけ組み立てます。eframe はそこに、wgpu のレンダーステートを含む `CreationContext` を渡します。リソースは専用の型のもとで`callback_resources` に入るので、型で見つけ直せますし、ほかのアプリのものとぶつかりません。[`<Canvas>`](/ja/reference/elements) 要素はそれを何も知りません。レイアウトに矩形を求め、それを持って `paint` を呼ぶだけです。そして中身は、`egui_wgpu::Callback` をペインタに積む 1 行です。`shader.wgsl` の中身は、その上のRust からは見えません。

要点は状態です。`speed`、`mass`、`bloom`、`tilt`、`paused` は、ほかと同じ[`use_state`](/ja/reference/hooks) です。値はコールバックの構造体にコピーされ、uniform バッファに書かれ、WGSL が読みます。スライダーを動かすとフックの中の数値が変わり、絵が変わります。その間にあるものは、何も知らされる必要がありません。まず `mass` を触ってみてください。シェーダが光子をその周りでトレースしているシュワルツシルト半径です。

ブラウザでは、対応していれば WebGPU で動き、そうでなければ WebGL に落ちます。どちらも eframe の既定のままで、設定するものはありません。`setup` という穴を文脈つきで説明したものが [Web とネイティブ](/ja/guide/web-and-native) です。

## 自分で動かす

```sh
cargo run -p shader
trunk serve --config examples/shader/Trunk.toml
```

ソースは [`examples/shader/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/shader/src/lib.rs)、パイプラインは [`src/gpu.rs`](https://github.com/fand/egui-react/blob/main/examples/shader/src/gpu.rs)、シェーダは [`src/shader.wgsl`](https://github.com/fand/egui-react/blob/main/examples/shader/src/shader.wgsl) にあります。

</ExamplePage>
