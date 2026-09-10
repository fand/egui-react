---
title: form
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="form"
  :has-plain="data.form.hasPlain"
  :react-lines="data.form.reactLines"
  :plain-lines="data.form.plainLines"
>

<template v-slot:summary>

束縛できるウィジェットを全部、変更ログ付きで。設定は再起動しても残る。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source form
:::

</div>

<div data-version="plain">

::: example-source form plain
:::

</div>

</template>

束縛できる要素を全部入れた設定画面です。[`<TextEdit>`、`<Checkbox>`、`<Slider>`、`<ComboBox>`](/ja/reference/elements)。どれも `use_persisted` が持つ 1 つの `Settings` 構造体のフィールドに束縛されているので、ここで変えたものは再起動してもここにあります。隣の変更ログは普通の`use_state` です。

見どころは `bind` です。ウィジェットに状態の `&mut` を渡すので、ウィジェットが直接そこに書き込み、コピーして戻すものは何もありません。しかも `&mut *state`と違って dirty の印を付けないので、毎フレーム書くウィジェットが毎フレーム再描画を要求することもありません。代わりに規則が 1 つ付きます。同じ要素の`on_change` から同じ状態を触ることはできません。唯一の `&mut` はすでにウィジェットが握っていて、二度目の借用はコンパイルが通らないからです。だからここでの `on_change` は、ウィジェットが渡せるものを運びます。新しい `bool`、新しいインデックス。変更ログはまったく別の場所にあります。長い版は[状態とフック](/ja/guide/state-and-hooks) にあります。

タブを見比べてください。素の egui 版も同じウィジェットと同じ永続化を持ち、それぞれの周りに読み書きを明示的に書いてあります。

## 自分で動かす

```sh
cargo run -p form
cargo run -p form --bin form-plain    # 素の egui 版
trunk serve --config examples/form/Trunk.toml
```

ソースは [`examples/form/src/lib.rs`](https://github.com/fand/egui-reactor/blob/main/examples/form/src/lib.rs)と [`plain.rs`](https://github.com/fand/egui-reactor/blob/main/examples/form/src/plain.rs) です。

</ExamplePage>
