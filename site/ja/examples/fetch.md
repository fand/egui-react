---
title: fetch
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="fetch"
  :has-plain="data.fetch.hasPlain"
  :react-lines="data.fetch.reactLines"
  :plain-lines="data.fetch.plainLines"
>

<template v-slot:summary>

`use_future` がリクエストを走らせ、一番近い `<Suspense>` がスピナーを描く。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source fetch
:::

</div>

</template>

コンポーネントは 2 つです。`App` が、選ばれている都市と試行回数のカウンタを持ちます。`Weather` はその両方を deps にして[`use_future`](/ja/reference/hooks) を呼びます。だから別の都市を選べば新しいリクエストになり、`refresh` はカウンタを進めて同じものをもう一度頼みます。リクエストは、座標がまだ借りられるその場で組み立てます。返る future はそれを所有し、`'static` です。返ってくる答えは本物です。[Open-Meteo](https://open-meteo.com) の 3 日分の天気で、鍵は要りません。`serde` は本文から、画面に出すフィールドだけを読みます。

`Weather` は保留中の表示を一切描きません。`let Poll::Ready(response) = response else { return; };` で終わり、待つ仕事は上の [`<Suspense>`](/ja/reference/elements) に任せます。そちらは下にある保留中のfuture を数え、代わりに `fallback` を描きます。React の `throw` に対する Rust の答えがこれで、値が必要な葉は単にやめられる、ということです。

リクエストが失敗する 3 つの道 ― そもそも届かなかった、サーバが拒んだ、本文が期待した JSON でなかった ― は、画面上の 1 行に落ち着きます。`parse` はそのためのものです。同じコードが両方で走ります。`ehttp` はネイティブでは専用スレッドのureq を、Web ではブラウザの fetch API を使いますが、その違いは書く型には届きません。上の埋め込みは wasm ビルドなので、取得先はブラウザの通常のクロスオリジンの規則に従います。古い結果の扱いも含めた長い話は[非同期](/ja/guide/async) にあります。

## 自分で動かす

```sh
cargo run -p fetch
trunk serve --config examples/fetch/Trunk.toml
```

ソースは [`examples/fetch/src/lib.rs`](https://github.com/fand/egui-reactor/blob/main/examples/fetch/src/lib.rs) です。

</ExamplePage>
