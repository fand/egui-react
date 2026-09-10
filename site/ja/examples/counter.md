---
title: counter
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="counter"
  :has-plain="data.counter.hasPlain"
  :react-lines="data.counter.reactLines"
  :plain-lines="data.counter.plainLines"
>

<template v-slot:summary>

1 つの状態を、3 つのハンドラが順に借りる。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source counter
:::

</div>

</template>

3 つのハンドラは、それぞれ `count` を `&mut` で順に取ります。クローンはありませんし、`'static` なものもありません。クロージャは自分を作ったパスの中で走るので、借用チェッカは普通のスコープだけで納得します。

外側の `<View>` の `align="center" justify="center"` が、この塊を中央に置いています。素の egui でこれをやるには、まず塊を測り、その周りの空間を手で確保することになります。

## 自分で動かす

```sh
cargo run -p counter
trunk serve --config examples/counter/Trunk.toml
```

ソースは [`examples/counter/src/lib.rs`](https://github.com/fand/egui-reactor/blob/main/examples/counter/src/lib.rs) です。

</ExamplePage>
