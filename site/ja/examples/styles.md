---
title: styles
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="styles"
  :has-plain="data.styles.hasPlain"
  :react-lines="data.styles.reactLines"
  :plain-lines="data.styles.plainLines"
>

<template v-slot:summary>

`style` 属性を表に全部: 名前、使うコード、描かれる姿。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source styles
:::

</div>

</template>

属性 1 つにつき 1 行。名前、コード、そのコードが作る絵です。どの属性も、すべての要素が取る 1 つの `style` prop を通ります。レイアウト側（`w`、`grow`、`p` など）はレイアウトエンジンが解決し、描画側（`bg`、`border`、`radius`、`shadow`、`custom_shadow`、`opacity`）はそのノード自身のボックスに描かれます。

注目に値するのは真ん中の列です。手で打ち込んだものではありません。各行は「描かれる要素」として 1 回だけ書かれ、マクロが同じトークンを `stringify!` してコード列にします。だからテキストと絵がずれることはありません。これはマクロ自身の規則の小さな実演でもあります。`macro_rules!` を通して渡された要素も、それが座っている `rsx!` のクロージャに解決されるからです。

描画の挙動が 2 つ、散文で読むよりここで見るほうが分かりやすいです。1 つ、枠線はレイアウトです。その太さはパディングの縁に乗るので、子はストロークの内側から始まり、枠線は子が入らなかったその帯にちょうど描かれます。2 つ、自分でボックスを描くウィジェット ― `<Button>` や `<TextEdit>` ― は、こちらがボックスを描いた時点で自分のボックスを譲るので、二重になりません。表は[レイアウト属性](/ja/reference/layout-attributes) にあります。

## 自分で動かす

```sh
cargo run -p styles
trunk serve --config examples/styles/Trunk.toml
```

ソースは [`examples/styles/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/styles/src/lib.rs) です。

</ExamplePage>
