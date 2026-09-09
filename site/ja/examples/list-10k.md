---
title: list-10k
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="list-10k"
  :has-plain="data['list-10k'].hasPlain"
  :react-lines="data['list-10k'].reactLines"
  :plain-lines="data['list-10k'].plainLines"
>

<template v-slot:summary>

1 万行を、全部描くとどれだけかかるか。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source list-10k
:::

</div>

<div data-version="plain">

::: example-source list-10k plain
:::

</div>

</template>

[`<VirtualList>`](/ja/reference/elements) はビューポートに見えている行だけを描き、残りの高さは確保だけします。だからフレーム時間は行数に依存しません。`render` は画面にある各行のインデックスとともに呼ばれます。react-virtualized が`rowRenderer` を呼ぶのと同じです。アプリが持つのはデータで、`filtered` をメモ化しているのは、毎フレーム 1 万個の `String` を作るほうが、描くより高くつくからです。

これは正直なサンプルでもあります。`<VirtualList>` が包んでいるのは`egui::ScrollArea::show_rows` で、隣の素の egui 版が呼んでいるのもまさにそれです。だから 2 つのタブは同じくらいの長さになりますし、仮想化はライブラリの機能ではありません。ここでライブラリが足しているのは、行がフックを持てることです。行はそれぞれ専用のスコープの中で描かれます。`for` に `key={i}` を付けたのと同じです。全行を描いた場合のこのリポジトリ自身の計測は、`tests/scenarios.rs` と`docs/tasks/list-perf/` にあります。

見ているものについて 2 つ。埋め込み版は 1,000 行から始まります。隣の素の egui 版に合わせるためです。単体のバイナリは 10 万行で開きます。行は仮想化されているので、どちらでもスライダーはそこまで届きます。それから、どの行も同じ高さでなければいけません。行が何を描こうと、リストはきっかり `row_h` だけ進みます。それが、自分で要求した範囲と歩調を合わせている仕組みです。

## 自分で動かす

```sh
cargo run -p list-10k
cargo run -p list-10k --bin list-10k-plain    # 素の egui 版
trunk serve --config examples/list-10k/Trunk.toml
```

ソースは [`examples/list-10k/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/list-10k/src/lib.rs)と [`plain.rs`](https://github.com/fand/egui-react/blob/main/examples/list-10k/src/plain.rs) です。

</ExamplePage>
