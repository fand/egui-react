---
title: layout
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="layout"
  :has-plain="data.layout.hasPlain"
  :react-lines="data.layout.reactLines"
  :plain-lines="data.layout.plainLines"
>

<template v-slot:summary>

`<View>` が解釈する flex と grid の属性を、1 節につき 1 つずつ。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source layout
:::

</div>

<div data-version="plain">

::: example-source layout plain
:::

</div>

</template>

アプリというより、目で見る表です。ラベル付きの節が 7 つあり、それぞれが属性の1 グループの働きを描きます。`direction`、`justify` と `align` に `grow`、`wrap` と `gap`、入れ子、`display="grid"` と `cols`、そして葉としての egui 自身のコンテナ。どの節も箱で、どの子も塗りつぶしたチップです。要点は **箱がどこに落ち着くか** だからです。何も塗らなければ、`justify` も `grow` も見せるものがありません。散文での完全な一覧は[レイアウト属性](/ja/reference/layout-attributes) にあります。このページは、そのうち目で見える部分です。

同時に 2 つのことを見せています。flex と grid の節はレイアウトエンジンです。自前のエンジンの下に [taffy](https://github.com/DioxusLabs/taffy) がいます。`<Grid>`、`<Row>`、`<Vertical>`、`<Label>` の節は egui **自身の** コンテナで、そのほうが馴染みがあるか安い場面のために要素として残してあります。その子は素の `Ui` モードで描かれるので、`<View>` がまた木を始めるまで、中でレイアウト属性は何もしません。

素の egui のタブは、とくにこのページで開く価値があります。egui に flexbox はありません。だから同じ絵を、手で測って手で置いて作ることになります。行数の差はそこから来ますし、レイアウトこそ、このライブラリが存在する理由です。

## 自分で動かす

```sh
cargo run -p layout
cargo run -p layout --bin layout-plain    # 素の egui 版
trunk serve --config examples/layout/Trunk.toml
```

ソースは [`examples/layout/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/layout/src/lib.rs)と [`plain.rs`](https://github.com/fand/egui-react/blob/main/examples/layout/src/plain.rs) です。

</ExamplePage>
