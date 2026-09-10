---
title: custom-hook
---

<script setup>
import { data } from '../examples.data.js'
</script>

<ExamplePage
  name="custom-hook"
  :has-plain="data['custom-hook'].hasPlain"
  :react-lines="data['custom-hook'].reactLines"
  :plain-lines="data['custom-hook'].plainLines"
>

<template v-slot:summary>

Three hooks of your own, each called from two components that keep their own
state.

</template>

<template v-slot:code>

<div data-version="react">

::: example-source custom-hook
:::

</div>

</template>

A custom hook is an ordinary function that takes `&mut Cx` and calls other
hooks. `#[hook]` is what makes it reusable: it enters a scope keyed by the
*call site*, so two calls — in one component or in two — get their own state.
Leave it off and every call shares one slot, and the second call in a pass is
reported as a collision. That is the whole feature; there is no registration,
no trait, and nothing else to learn, because a custom hook *is* the built-in
hooks.

The three here are `use_debounce` (the value as it was once it stopped changing
for a delay — a search box that should not fire a request per keystroke),
`use_previous` (React's `usePrevious`, in one line), and `use_window_size` (for
layout that reacts to the window). Each is called from two different components
in the page, and the pairs do not share state — type into one and watch the
other stay put.

`use_debounce` also shows a small responsibility that comes with writing hooks:
while a change is still settling, nothing else would ask for the frame that
settles it, so the hook asks for that repaint itself with
`request_repaint_after`. A hook that measures time usually has to.

## Run it yourself

```sh
cargo run -p custom-hook
trunk serve --config examples/custom-hook/Trunk.toml
```

The source is [`examples/custom-hook/src/lib.rs`](https://github.com/fand/egui-reactor/blob/main/examples/custom-hook/src/lib.rs);
the hooks themselves are in [`src/hooks.rs`](https://github.com/fand/egui-reactor/blob/main/examples/custom-hook/src/hooks.rs).

</ExamplePage>
