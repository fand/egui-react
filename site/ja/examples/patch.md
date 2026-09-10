---
title: patch
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="patch"
  :has-plain="data.patch.hasPlain"
  :react-lines="data.patch.reactLines"
  :plain-lines="data.patch.plainLines"
>

<template v-slot:summary>

自分で WGSL シェーダを生成し、検証し、プレビューするノードエディタ。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source patch
:::

</div>

</template>

隅の絵はエディタが描いているのではありません。その上のパッチ **そのもの** であるフラグメントシェーダが描いています。ノードを 2 つつなぐと、新しい WGSL のプログラムが生成され、CPU 上で naga が検証し、コンパイルされます。スライダーを動かしたときは、何も生成されません。4 つの float が uniform バッファに届き、同じプログラムが別の絵を描きます。

この違いがこのサンプルのすべてで、それは 1 か所での 1 つの決定です。どのメッセージも `topology_rev`（プログラムが変わりうる）か `param_rev`（変わりうるのは数値だけ）のどちらかを進め、その 2 つのカウンタに[`use_memo`](/ja/reference/hooks) が 1 つずつぶら下がります。片方が WGSL を、もう片方が uniform ブロックを作ります。「ソースが変わったか」を見張っているものはありません。それぞれの memo の deps が何かということから、自然にそうなります。そしてテストがそれを固定しています。検証が memo の中で起きることにも意味があります。不正なグラフは GPU ではなく、エディタの中で報告されます。

ノードは絶対座標に置かれます。`ItemStyle` にそのための属性はありませんし、必要でもありません。キャンバスは 1 つの葉で、自分の矩形を確保し、ノードごとにその座標で子の `Ui` を開き、その周りに `Cx` を組み立てます。[エスケープハッチ](/ja/guide/escape-hatches) の 1 段上です。ノードの中に入れば、また普通の `<View>` の flexbox です。ノードには `cx.scope(node.id, ..)` で手で鍵を付けています。`key=` がしているのとまったく同じことなので、別のノードを消したり並べ替えたりしても、そのノード自身の状態は残ります。

絵は背景です。キャンバスは出力シェーダを自分いっぱいに、切り取らずに収めて描き、パッチはその上に乗ります。パラメータはどれも、それを持つノードの中で編集されるので、選択状態と歩調を合わせておくべきものはありません。[`<Overlay>`](/ja/reference/elements) でキャンバスの上に浮いているのは、どのノードにも属さないものだけです。片隅にノード数、もう片隅に、あれば naga の苦情。ビューは最初に描かれるときパッチに合わせて収まり、`Recenter` がそこへ戻します。メニューバーは素の egui の葉が 1 つです。メニューは自分のレイヤーとして開き、レイアウトの場所を取らないからです。そこには時計、再生ボタン、それに絵が描かれる 1 分ループのタイムラインも入っています。絵そのものは `egui_wgpu`の描画コールバックです。対応しているブラウザでは WebGPU、そうでなければeframe の WebGL フォールバックです。

## 自分で動かす

```sh
cargo run -p patch
trunk serve --config examples/patch/Trunk.toml
```

ソースは [`examples/patch/src/lib.rs`](https://github.com/fand/egui-reactor/blob/main/examples/patch/src/lib.rs) です。

</ExamplePage>
