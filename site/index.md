---
layout: home

hero:
  name: egui-react
  tagline: Write egui apps the way you write React
  actions:
    - theme: brand
      text: Get Started
      link: /getting-started
    - theme: alt
      text: Examples
      link: /examples/counter
    - theme: alt
      text: GitHub
      link: https://github.com/fand/egui-react
---

<script setup>
import { data } from './examples.data.js'
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

<div data-version="plain">

::: example-source counter plain
:::

</div>

</ExampleEmbed>

<div class="home-features">
  <div class="feature">
    <h3>Components and hooks</h3>
    <p>
      A JSX-like <code>rsx!</code> macro, function components with
      <code>#[component]</code>, and hooks such as <code>use_state</code>,
      <code>use_effect</code> and <code>use_future</code> — event handlers
      simply borrow local state with <code>&amp;mut</code>, with no
      <code>.clone()</code> ceremony.
    </p>
  </div>
  <div class="feature">
    <h3>Flexbox and Grid, first class</h3>
    <p>
      <code>&lt;View&gt;</code> is a node in a small layout engine of our own,
      written over <a href="https://github.com/DioxusLabs/taffy">taffy</a>:
      direction, gap, grow, wrap and grid areas are attributes, not manual
      measurement.
    </p>
  </div>
  <div class="feature">
    <h3>Native and web</h3>
    <p>
      One <code>run(Options, ..)</code> call opens a window natively and takes
      over a <code>&lt;canvas&gt;</code> in the browser. Every example on this
      site is running the wasm build.
    </p>
  </div>
</div>
