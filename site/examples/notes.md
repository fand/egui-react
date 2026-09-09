---
title: notes
---

<script setup>
import { data } from '../examples.data.js'
</script>

<ExamplePage
  name="notes"
  :has-plain="data.notes.hasPlain"
  :react-lines="data.notes.reactLines"
  :plain-lines="data.notes.plainLines"
>

<template v-slot:summary>

A notes app: reducer, persistence, context, memo and a settings window, together.

</template>

<template v-slot:code>

<div data-version="react">

::: example-source notes
:::

</div>

</template>

Nothing here is new. It is the pieces the other examples show one at a time,
put together the way an application would put them: `use_reducer` owns the
notes, `use_persisted` keeps them across restarts, `use_memo` filters and sorts
the list keyed on the search text, and `provide_context` hands a theme to both
columns without either of the `<View>`s in between carrying it. Settings live
in a `<Window>` opened by a button, with a two-step confirm that is just an
`if` in the middle of the tree.

Two details are worth stopping on. The reducer holds the notes and the
persisted slot mirrors them — a reducer cannot reduce *into* someone else's
slot, so the two lines that copy one to the other are the price of having both
[`use_reducer` and `use_persisted`](/reference/hooks). And the note editor uses
`bind` for the text while `on_change` updates the note's timestamp: the widget
holds the only `&mut` to the text, so the clock is moved by a message rather
than by a second borrow of the same state. That constraint is explained in
[State and hooks](/guide/state-and-hooks).

If you are reading the examples in order, start here and then take whichever
piece you want one at a time.

## Run it yourself

```sh
cargo run -p notes
trunk serve --config examples/notes/Trunk.toml
```

The source is [`examples/notes/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/notes/src/lib.rs).

</ExamplePage>
