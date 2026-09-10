---
title: form
---

<script setup>
import { data } from '../examples.data.js'
</script>

<ExamplePage
  name="form"
  :has-plain="data.form.hasPlain"
  :react-lines="data.form.reactLines"
  :plain-lines="data.form.plainLines"
>

<template v-slot:summary>

Every bound widget, a change log, and settings that survive a restart.

</template>

<template v-slot:code>

<div data-version="react">

::: example-source form
:::

</div>

<div data-version="plain">

::: example-source form plain
:::

</div>

</template>

One settings screen with every bound element in it:
[`<TextEdit>`, `<Checkbox>`, `<Slider>` and `<ComboBox>`](/reference/elements).
All of them bind into fields of a single `Settings` struct held by
`use_persisted`, so what you change here is still here after a restart; the
change log next to them is an ordinary `use_state`.

`bind` is the thing to look at. It hands the widget `&mut` the state, so the
widget writes into it directly and nothing has to be copied back — and, unlike
`&mut *state`, it does not mark the state dirty, which is what stops a widget
that writes every frame from asking for a repaint every frame. The cost is a
rule: `on_change` on the same element may not touch the same state, because the
widget already holds the only `&mut` to it and a second borrow would not
compile. So `on_change` here carries whatever the widget can hand over — the
new `bool`, the new index — and the change log is somewhere else entirely.
[State and hooks](/guide/state-and-hooks) has the longer version.

Compare the tabs: the plain egui version has the same widgets and the same
persistence, written as explicit reads and writes around each one.

## Run it yourself

```sh
cargo run -p form
cargo run -p form --bin form-plain    # the plain egui version
trunk serve --config examples/form/Trunk.toml
```

The source is [`examples/form/src/lib.rs`](https://github.com/fand/egui-reactor/blob/main/examples/form/src/lib.rs)
and [`plain.rs`](https://github.com/fand/egui-reactor/blob/main/examples/form/src/plain.rs).

</ExamplePage>
