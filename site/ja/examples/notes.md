---
title: notes
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="notes"
  :has-plain="data.notes.hasPlain"
  :react-lines="data.notes.reactLines"
  :plain-lines="data.notes.plainLines"
>

<template v-slot:summary>

メモアプリ: reducer、永続化、コンテキスト、メモ化、エディタをまとめて。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source notes
:::

</div>

</template>

ここに新しいものは何もありません。ほかのサンプルが 1 つずつ見せている部品を、アプリケーションがまとめるであろう形でまとめただけです。`use_reducer` がメモを持ち、`use_persisted` が再起動をまたいで保持し、`use_memo` が検索文字列を鍵にリストを絞り込んで並べ替え、`provide_context` がテーマを両方の列に渡します。間にある 2 つの `<View>` は、どちらもテーマを運びません。テーマは egui 自身のダーク / ライトです。プロバイダは木のてっぺんでそれを読み、変わったときだけハンドルに書きます。

立ち止まる価値のある細かい点が 2 つあります。1 つは、reducer がメモを持ち、永続化スロットがそれを写している点です。reducer は他人のスロット **に向かって**畳むことはできないので、片方をもう片方へコピーする 2 行が、[`use_reducer` と `use_persisted`](/ja/reference/hooks) を両方使うための代金です。もう 1 つは、メモのエディタがタイトルには `bind` を使い、`on_change` でメモのタイムスタンプを更新している点です。テキストへの唯一の `&mut` を握っているのはウィジェットなので、時計は同じ状態をもう一度借りるのではなく、メッセージで動かします。この制約は[状態とフック](/ja/guide/state-and-hooks) で説明しています。ここには手書きの葉が2 つあります。要素では埋められない理由が 1 つずつあるからです。`new` でメモを作ったとき、タイトルはテキストを選択した状態でフォーカスを取ります。本文は`egui::TextEdit` を `egui::ScrollArea` で包んであるので、長いメモはウィンドウの下へはみ出さず、列の中でスクロールします。

テキストは日本語の入ったフォントで描いています。egui は渡されたフォントのバイト列からラスタライズし、自前のフォントには CJK のグリフがありません。だから何もしなければ日本語のメモは豆腐になります。`Fonts` のスタックを 1 つ作り、Noto Sans JP のサブセットを egui のフォントの前に置いて、それを既定にしています。この話の長い版が [font](/ja/examples/font) です。

サンプルを順に読んでいるなら、まずここから始めて、あとは欲しい部品を 1 つずつ見ていってください。

## 自分で動かす

```sh
cargo run -p notes
trunk serve --config examples/notes/Trunk.toml
```

ソースは [`examples/notes/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/notes/src/lib.rs) です。

</ExamplePage>
