---
title: custom-hook
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="custom-hook"
  :has-plain="data['custom-hook'].hasPlain"
  :react-lines="data['custom-hook'].reactLines"
  :plain-lines="data['custom-hook'].plainLines"
>

<template v-slot:summary>

自作フック 3 つを、それぞれ独自の状態を持つ 2 つのコンポーネントから呼ぶ。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source custom-hook
:::

</div>

</template>

自作フックは、`&mut Cx` を取ってほかのフックを呼ぶ、普通の関数です。再利用できるようにしているのは `#[hook]` です。**呼び出し位置** を鍵にしたスコープに入るので、2 か所からの呼び出しは ― 同じコンポーネントの中でも、別々のコンポーネントからでも ― それぞれの状態を持ちます。付けなければ、どの呼び出しも1 つのスロットを共有し、同じパスでの 2 回目の呼び出しは衝突として報告されます。機能はこれだけです。登録もトレイトも、ほかに覚えることもありません。自作フックは組み込みのフック **そのもの** だからです。

ここにある 3 つは `use_debounce`（値が一定時間変わらなくなったときのその値。1 打鍵ごとにリクエストを投げたくない検索ボックス向け）、`use_previous`（React の`usePrevious` を 1 行で）、`use_window_size`（ウィンドウに反応するレイアウト向け）です。どれもページの中の 2 つのコンポーネントから呼ばれていて、その組は状態を共有しません。片方に入力して、もう片方が動かないのを見てください。

`use_debounce` は、フックを書く人に付いてくる小さな責任も見せています。変化が落ち着くまでの間、それを落ち着かせるフレームを誰も要求しません。だからフック自身が `request_repaint_after` でその再描画を頼みます。時間を測るフックは、たいていそうする必要があります。

## 自分で動かす

```sh
cargo run -p custom-hook
trunk serve --config examples/custom-hook/Trunk.toml
```

ソースは [`examples/custom-hook/src/lib.rs`](https://github.com/fand/egui-reactor/blob/main/examples/custom-hook/src/lib.rs)、フック自体は [`src/hooks.rs`](https://github.com/fand/egui-reactor/blob/main/examples/custom-hook/src/hooks.rs) にあります。

</ExamplePage>
