---
title: font
---

<script setup>
import { data } from '../../examples.data.js'
</script>

<ExamplePage
  name="font"
  :has-plain="data.font.hasPlain"
  :react-lines="data.font.reactLines"
  :plain-lines="data.font.plainLines"
>

<template v-slot:summary>

CSS 風のフォントチェーン: 同梱・取得・インストール済みのフォントと、各項目の解決先。

</template>

<template v-slot:code>

<div data-version="react">

::: example-source font
:::

</div>

</template>

egui は渡されたフォントのバイト列からテキストを描きます。ブラウザのフォントもOS のフォントマッチングも一切関わりませんし、egui 自身の 4 フォントには CJK のグリフがないので、素の egui アプリでは日本語が豆腐になります。このサンプルはその答えです。名前付きのチェーンが 4 つ、ソースの種類ごとに 1 つずつあります。`bundled` は `include_bytes!` で焼き込んだ 433 KB のサブセット。`web` はアプリ自身のオリジンから 4.5 MB のフル版 Noto Sans JP を取得します。`system` はインストール済みのフォントをマシンに尋ねます。`code` は等幅のチェーンで、egui の Hack の後ろに、Hack に無いかなと漢字のためのサブセットを置いています。

見本の下のレポート表が本題です。どのチェーンのどの項目についても、それがどのフェイスに解決されたか ― そのフェイスが名乗るファミリ名つきで。`System` の項目に必要な綴りは、こうやって見つけます ― あるいは、なぜ解決されなかったかが出ます。ネイティブで `system` のスタックを選べば fontconfig の答えが見えます。ブラウザで選べば Local Font Access の道になりますが、これは Chromium 限定で、許可のプロンプトが要り、クリックから呼ぶ必要があります。だからそれ以外のブラウザでは、ボタンが理由つきで無効になっています。

**上の埋め込みは、要求に応じて 4.5 MB のフォントを取得します。** `web` のスタックを選ぶとダウンロードが始まります。届くまでは、チェーンでその後ろにいる同梱サブセットがテキストを描き、バイト列が届くと項目が pending から loaded に変わります。これが CSS の `swap` です。`font-display` の行でそれを `block` に切り替えられます。`block` は `Fonts::pending()` が false になるまでプレースホルダを描きます。ライブラリが報告するのは URL がまだ飛行中かどうかだけで、どちらの方針を採るかはアプリが決めることです。残りは[フォント](/ja/guide/fonts) にあります。

## 自分で動かす

```sh
cargo run -p font
trunk serve --config examples/font/Trunk.toml
```

Web フォントはコミットしていません。このサンプルの `Trunk.toml` にあるビルド前フックが、trunk の初回ビルド時にダウンロードします。ネイティブでは同じ相対 URL に指す先のサーバが無いので、その項目は失敗として報告され、チェーンはサブセットで描きます。レポートにはそれも出ます。

ソースは [`examples/font/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/font/src/lib.rs) です。

</ExamplePage>
