---
title: board
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="board"
  :has-plain="data.board.hasPlain"
  :react-lines="data.board.reactLines"
  :plain-lines="data.board.plainLines"
>

<template v-slot:summary>

カードをドラッグして列の間を移動しても、入力中のタイトルはそのまま残る。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source board
:::

</div>

<div data-version="plain">

::: example-source board plain
:::

</div>

</template>

ほかのサンプルは状態を 1 か所に持ち、それを描きます。これは **アイテム** に属する状態の話です。誰かが入力しかけたタイトルと、カードが並べ替えられ、別の列へ移され、フィルタで消え、また戻ってきたときに、それがどうなるか。カードのタイトルを打ちかけて、そのカードを別の列へドラッグしてみてください。下書きは一緒に付いていきます。

そうなるのは、下書きの鍵がカードの描かれる場所ではなく、カードの同一性だからです。違いを生んでいるのは、このサンプル自身の[自作フック](/ja/guide/state-and-hooks) の 1 つ、`use_identity` です。`key=`だけでは足りません。カードを動かすと親が変わってしまうからです。削除されたカード ― フィルタで視界から外れたカードも、同じようにアンマウントされます ―の状態は、パス終わりの掃除が落とします。素の egui 版が手で書かなければならない後始末が、これです。

あとは組み立ての話です。`<Toolbar>`、`<Column>`、`<Card>`、`<TitleEdit>`、`<Chip>`、`<IconButton>` はどれも `style: ItemStyle` を取るので、どこに座るかは呼び出し側が決めます。そして何が起きたかを[イベントの props](/ja/guide/components-and-events) で知らせます。undo / redo、検索のデバウンス、ドラッグの一連の流れは、アプリケーションが使えるのと同じ公開API に対して書かれた自作フックの中にあります。どれもライブラリの変更を必要としませんでした。何がどちら向きに流れるかは、ここでは 2 回決められています。コンポーネントが **したこと** はイベントとして上がり、木全体で共有するもの（テーマ、進行中のドラッグ、`Dispatch`）は[コンテキスト](/ja/reference/hooks) で下ります。テーマは egui 自身のものです。ボードは自前のテーマを持たず、ページの外観の切り替えに従います。

タブのラベルを見比べてください。素の egui 版も同じ仕事をします。違うのは、カードごとの状態がどこに置かれることになるかです。

## 自分で動かす

```sh
cargo run -p board
cargo run -p board --bin board-plain    # 素の egui 版
trunk serve --config examples/board/Trunk.toml
```

ソースは [`examples/board/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/board/src/lib.rs)と [`plain.rs`](https://github.com/fand/egui-react/blob/main/examples/board/src/plain.rs) です。

</ExamplePage>
