---
layout: home

hero:
  name: egui-react
  image:
    src: /atomic-ferris.svg
    alt: atomic ferris
  tagline: React を書くように egui アプリを書く
  actions:
    - theme: brand
      text: はじめる
      link: /ja/getting-started
    - theme: alt
      text: サンプル
      link: /ja/examples/
    - theme: alt
      text: GitHub
      link: https://github.com/fand/egui-react
---

<script setup>
import { data } from '../examples.data.js'
</script>

<ExampleEmbed
  name="counter"
  :has-plain="data.counter.hasPlain"
  :react-lines="data.counter.reactLines"
  :plain-lines="data.counter.plainLines"
>

<div data-version="react">

::: example-source counter
:::

</div>

</ExampleEmbed>

<div class="home-features">
  <div class="feature">
    <h3>コンポーネントとフック</h3>
    <p>JSX 風の <code>rsx!</code> マクロ、<code>#[component]</code> による関数コンポーネント、<code>use_state</code> / <code>use_effect</code> / <code>use_future</code> などのフック。イベントハンドラはローカルな状態を <code>&amp;mut</code> でそのまま借りるだけで、<code>.clone()</code> のお作法は要りません。</p>
  </div>
  <div class="feature">
    <h3>Flexbox と Grid が一級市民</h3>
    <p><code>&lt;View&gt;</code> は <a href="https://github.com/DioxusLabs/taffy">taffy</a> の上に書いた小さな自前レイアウトエンジンのノードです。direction、gap、grow、wrap、グリッド領域は属性であって、手で測る対象ではありません。</p>
  </div>
  <div class="feature">
    <h3>ネイティブと Web</h3>
    <p><code>run(Options, ..)</code> を 1 回呼べば、ネイティブではウィンドウが開き、ブラウザでは <code>&lt;canvas&gt;</code> を乗っ取ります。このサイトのサンプルはすべて wasm ビルドが動いています。</p>
  </div>
</div>
