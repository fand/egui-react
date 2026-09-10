---
title: clock
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="clock"
  :has-plain="data.clock.hasPlain"
  :react-lines="data.clock.reactLines"
  :plain-lines="data.clock.plainLines"
>

<template v-slot:summary>

自分で再描画を要求するストップウォッチと、後片付けをするエフェクト。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source clock
:::

</div>

</template>

ここでは再描画が明示的です。分かっておく価値があるのはそこです。egui は誰かが頼んだときにしか描かないので、止まっている時計は 1 秒に 1 フレームを頼み、動いているストップウォッチは次のフレームをすぐ頼みます。アプリのほかの部分は何も知らなくてよく、どちらも動いていなければ、アプリはコストゼロです。

`show ticker` のチェックボックスは子をマウントし、アンマウントします。このサンプルの本題はそこです。その子の [`use_effect`](/ja/reference/hooks) はクロージャを返し、それがクリーンアップです。子が消えたときに走ります。消えたことを見つけるのはパス終わりの掃除です。クリーンアップは保存されるので`'static` で、ログを借りられません。だから代わりに `Dispatch` で報告します。アンマウントされるコンポーネントがアプリのほかの部分に何かを伝えるときは、だいたいこの形になります。

時刻は `ui.input(|i| i.time)` から取っています。egui が数えている、アプリ開始からの秒数です。`std::time::Instant` はどこにも出てきません。`wasm32-unknown-unknown` で panic するからです。壁時計の時刻を `web_time` 経由にしているのも同じ理由です。ブラウザでも動かすアプリなら覚えておく価値のある、移植性の落とし穴です。

## 自分で動かす

```sh
cargo run -p clock
trunk serve --config examples/clock/Trunk.toml
```

ソースは [`examples/clock/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/clock/src/lib.rs) です。

</ExamplePage>
