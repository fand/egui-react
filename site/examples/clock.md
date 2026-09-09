---
title: clock
---

<script setup>
import { data } from '../examples.data.js'
</script>

<ExamplePage
  name="clock"
  :has-plain="data.clock.hasPlain"
  :react-lines="data.clock.reactLines"
  :plain-lines="data.clock.plainLines"
>

<template v-slot:summary>

A stopwatch that asks for its own repaints, and an effect that cleans up after
itself.

</template>

<template v-slot:code>

<div data-version="react">

::: example-source clock
:::

</div>

</template>

Repainting is explicit here, which is the part worth understanding. egui only
draws when something asks it to, so a stopped clock asks for one frame a second
and a running stopwatch asks for the next frame immediately. Nothing else in
the app has to know, and when neither is running the app costs nothing.

The `show ticker` checkbox mounts and unmounts a child, which is what the
example is really about. That child's [`use_effect`](/reference/hooks) returns
a closure, and that closure is the cleanup: it runs when the child goes away,
detected by the end-of-pass sweep. Because the cleanup is stored, it is
`'static` and cannot borrow the log — so it reports through a `Dispatch`
instead. That is the general shape for anything an unmounting component has to
tell the rest of the app.

Time comes from `ui.input(|i| i.time)`, seconds since the app started as egui
counts them. `std::time::Instant` appears nowhere, because it panics on
`wasm32-unknown-unknown`; the wall clock goes through `web_time` for the same
reason. That is a portability trap worth remembering for any app that will also
run in a browser.

## Run it yourself

```sh
cargo run -p clock
trunk serve --config examples/clock/Trunk.toml
```

The source is [`examples/clock/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/clock/src/lib.rs).

</ExamplePage>
