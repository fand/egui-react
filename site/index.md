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

## The counter, running

The example below is the wasm build of `examples/counter`, next to the source
it was built from. Switch to the plain egui version to see the same program
without the library.

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
    <h3>No reconciler</h3>
    <p>
      egui is immediate mode, so there is no retained tree to diff. Event
      handlers run where they are written and borrow local state with
      <code>&amp;mut</code> — none of the <code>'static</code> closures,
      <code>Rc&lt;RefCell&lt;_&gt;&gt;</code> or <code>.clone()</code> ceremony
      that retained-mode Rust UI frameworks require.
    </p>
  </div>
  <div class="feature">
    <h3>Components and hooks</h3>
    <p>
      A JSX-like <code>rsx!</code> macro, function components with
      <code>#[component]</code>, and hooks such as <code>use_state</code>,
      <code>use_effect</code> and <code>use_future</code> — including hooks of
      your own, written with <code>#[hook]</code>.
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
