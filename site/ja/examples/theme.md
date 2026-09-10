---
title: theme
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="theme"
  :has-plain="data.theme.hasPlain"
  :react-lines="data.theme.reactLines"
  :plain-lines="data.theme.plainLines"
>

<template v-slot:summary>

一番上で渡した 2 つの値を、3 階層下で読む。間のコンポーネントは何もしない。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source theme
:::

</div>

</template>

`Themed` が `Theme` と `Locale` を持ち、どちらも子に配ります。間にいる `Page`と `Card` は props を 1 つも取りません。テーマやロケールの存在すら知りません。3 階層下の葉が [`use_context`](/ja/reference/hooks) でそれを読み、プロバイダ自身の子である `Toggles` が、同じ `Handle` を通して書き戻します。いちばん下の`Orphan` は `Themed` の外で描かれるので、その `use_context` は `None` を返します。束縛が効くのは、それを配った子が走っている間だけです。

プロバイダの形に注目してください。あなたが写すことになるのはこの形です。`provide_context` は子をクロージャとして取り、公開する `Handle` はストアを借りています。だからハンドルは、それを配るのと同じコンポーネントの本体の中で作らなければいけません。`<Provide value={handle}>` のような要素はありませんし、作れません。props の型はストアのライフタイムを名指しできないからです。プロバイダはまた `#[component(shares_ui)]` です。自分では何も描かないので、その子は自前の木ではなく、親のレイアウト木のノードになるべきだからです。

値の鍵は型です。だから `Theme` と `Locale` は 1 つの構造体の 2 フィールドではなく、別々の型になっています。`use_context::<Theme>` はこれだけを見つけます。そして `use_state` ではなく `use_handle` です。`Handle` は `Copy` で借用を持たないので、プロバイダが走っている最中でも、部分木がそれを通して書き戻せます。

## 自分で動かす

```sh
cargo run -p theme
trunk serve --config examples/theme/Trunk.toml
```

ソースは [`examples/theme/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/theme/src/lib.rs) です。

</ExamplePage>
