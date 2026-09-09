---
title: fetch
---

<script setup>
import { data } from '../examples.data.js'
</script>

<ExamplePage
  name="fetch"
  :has-plain="data.fetch.hasPlain"
  :react-lines="data.fetch.reactLines"
  :plain-lines="data.fetch.plainLines"
>

<template v-slot:summary>

`use_future` runs the request; the nearest `<Suspense>` draws the spinner.

</template>

<template v-slot:code>

<div data-version="react">

::: example-source fetch
:::

</div>

</template>

Two components and about forty lines. `App` owns the URL and an attempt
counter; `Response` calls [`use_future`](/reference/hooks) with both as deps,
so editing the URL re-fetches on its own and the button re-fetches the same URL
by bumping the counter. The request is built in place, where `url` is still
borrowable, and the future that is returned owns it and is `'static`.

`Response` never draws a pending state. It ends with
`let Poll::Ready(response) = response else { return; };` and leaves the waiting
to the [`<Suspense>`](/reference/elements) above it, which counts the pending
futures below it and draws its `fallback` instead. That is the Rust answer to
React's `throw`, and it means a leaf that needs a value can simply stop.

The same code runs in both places: `ehttp` uses ureq on a thread of its own
natively and the browser's fetch API on the web, and none of that difference
reaches the types you write. The embed above is the wasm build, so what it
fetches is subject to the browser's usual cross-origin rules.
[Async](/guide/async) has the longer story, stale results included.

## Run it yourself

```sh
cargo run -p fetch
trunk serve --config examples/fetch/Trunk.toml
```

The source is [`examples/fetch/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/fetch/src/lib.rs).

</ExamplePage>
