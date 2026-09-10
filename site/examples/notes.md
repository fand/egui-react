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

A notes app: reducer, persistence, context, memo and an editor, together.

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
columns without either of the `<View>`s in between carrying it. The theme is
egui's own, dark or light: the provider reads it at the top of the tree and
writes the handle only when it changed.

Two details are worth stopping on. The reducer holds the notes and the
persisted slot mirrors them — a reducer cannot reduce *into* someone else's
slot, so the two lines that copy one to the other are the price of having both
[`use_reducer` and `use_persisted`](/reference/hooks). And the note editor uses
`bind` for the title while `on_change` updates the note's timestamp: the widget
holds the only `&mut` to the text, so the clock is moved by a message rather
than by a second borrow of the same state. That constraint is explained in
[State and hooks](/guide/state-and-hooks). Two things here are hand-written
leaves for the one reason an element cannot cover: the title takes the focus
with its text selected when `new` makes a note, and the body is an
`egui::ScrollArea` around an `egui::TextEdit`, so a long note scrolls inside
the column instead of growing past the bottom of the window.

The text is drawn with a font that has Japanese in it. egui rasterizes from
font bytes it was handed and its own fonts have no CJK glyphs, so a note in
Japanese would otherwise be boxes; one `Fonts` stack puts a subset of Noto
Sans JP in front of egui's font and makes it the default. [font](/examples/font)
is the long version of that story.

If you are reading the examples in order, start here and then take whichever
piece you want one at a time.

## Run it yourself

```sh
cargo run -p notes
trunk serve --config examples/notes/Trunk.toml
```

The source is [`examples/notes/src/lib.rs`](https://github.com/fand/egui-reactor/blob/main/examples/notes/src/lib.rs).

</ExamplePage>
