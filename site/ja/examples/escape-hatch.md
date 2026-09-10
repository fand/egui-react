---
title: escape-hatch
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="escape-hatch"
  :has-plain="data['escape-hatch'].hasPlain"
  :react-lines="data['escape-hatch'].reactLines"
  :plain-lines="data['escape-hatch'].plainLines"
>

<template v-slot:summary>

素の egui へ降りる 4 つの道: クロージャ、リーフ、ペインタ、入れ子の Cx。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source escape-hatch
:::

</div>

</template>

egui-reactor が包んでいるのは egui の便利な一部分で、全部ではありませんし、全部である必要もありません。最後はすべて `&mut egui::Ui` なので、egui にできることは呼び出し 1 回の距離にあります。4 つの節が 4 つの道筋です。木の途中に普通のコードを書く `{view(|cx| ..)}`、いまいる場所に egui を描く`cx.leaf(&style, |ui| ..)`、自分で描くための「確保して描く」、そして egui のコンテナのクロージャの中でフックを使うための、内側の `Ui` に対する新しい `Cx`。

落とし穴は 1 節目にあり、それが `leaf` の存在理由です。`cx.ui()` はどこからでも安全に **読め** ますが、`<View>` の中では、それはレイアウト木全体が始まった場所の `Ui` であって、いまいる位置ではありません。これを通して描くと、ウィジェットはレイアウトの外、木の左上隅に出ます。`cx.leaf` は代わりにノードを足し、そのノードの `Ui` を渡します。

細かい点が 2 つあります。プログレスバーは `leaf` ではなく `leaf_fill` を使い、両方の軸に大きさを与えています。広がるウィジェットは、描いたものではなくスタイルで大きさが決まり、`auto` のままの軸はウィンドウ全部を取ります。測るものが無いときの「埋める」は、そういう意味だからです。それからスパークラインは`&mut egui::Ui` を取る普通の関数です。このライブラリのことを何も知りません。それがまさに要点です。

これは回避策ではありません。`egui-reactor-elements` の要素も同じ呼び出しで書かれています。裏口はありません。同じ 4 つをもう少し言葉で説明したものが[エスケープハッチ](/ja/guide/escape-hatches) です。

## 自分で動かす

```sh
cargo run -p escape-hatch
trunk serve --config examples/escape-hatch/Trunk.toml
```

ソースは [`examples/escape-hatch/src/lib.rs`](https://github.com/fand/egui-reactor/blob/main/examples/escape-hatch/src/lib.rs) です。

</ExamplePage>
