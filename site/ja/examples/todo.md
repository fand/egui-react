---
title: todo
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="todo"
  :has-plain="data.todo.hasPlain"
  :react-lines="data.todo.reactLines"
  :plain-lines="data.todo.plainLines"
>

<template v-slot:summary>

reducer がリストを動かし、`use_persisted` が再起動をまたいで保持する。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source todo
:::

</div>

<div data-version="plain">

::: example-source todo plain
:::

</div>

</template>

リストを変えうるものはすべてメッセージです ― `Add`、`Toggle`、`Remove`、`ClearDone` ― そして 1 つの `reduce` 関数がそれを適用します。[`use_reducer`](/ja/reference/hooks) は状態と `Dispatch` を返すので、リストの奥にあるボタンも、上にコールバックの鎖を通さずに「何が起きたか」を言えます。

データ自体は `use_persisted` から来ます。鍵は明示的な文字列 `"todo/todos"`です。永続化スロットを決めるのが呼び出し位置ではなくこの鍵なのは、まさに、上に1 行足しただけで保存データが読めなくなっては困るからです。このサンプルが鍵に自分の名前を前置しているのは、アプリが永続化するものがすべて 1 つの名前空間を共有するからです。ネイティブで動かし、終了して、もう一度立ち上げてみてください。リストは eframe のストレージの中に残っています。

分かりにくいのはループの中です。`todos` は `for` に借りられているので、そこにあるハンドラはそれを触れません。削除ボタンは `Msg::Remove(i)` を送り、チェックボックスは作業用のコピーに束縛して、本当の変更は `Dispatch` を通して次のフレームで着地します。この借用から抜ける道の 1 つが reducer で、もう 1 つが[`update_later`](/ja/guide/state-and-hooks) です。

2 つのタブを見比べてください。素の egui 版も同じ `Vec<Todo>` を持ち、同じ仕事をします。加えて持っているのが帳簿です。メッセージ、永続化、そして手で遅らせる削除。

## 自分で動かす

```sh
cargo run -p todo
cargo run -p todo --bin todo-plain    # 素の egui 版
trunk serve --config examples/todo/Trunk.toml
```

ソースは [`examples/todo/src/lib.rs`](https://github.com/fand/egui-reactor/blob/main/examples/todo/src/lib.rs)と [`plain.rs`](https://github.com/fand/egui-reactor/blob/main/examples/todo/src/plain.rs) です。

</ExamplePage>
