---
title: spreadsheet
---

<script setup>
import { data } from '../examples.data.js'
</script>

<ExamplePage
  name="spreadsheet"
  :has-plain="data.spreadsheet.hasPlain"
  :react-lines="data.spreadsheet.reactLines"
  :plain-lines="data.spreadsheet.plainLines"
>

<template v-slot:summary>

Formulas over 26 x 10,000 cells: two memo stages, and a draft that survives
scrolling out of view.

</template>

<template v-slot:code>

<div data-version="react">

::: example-source spreadsheet
:::

</div>

</template>

Type a number into a cell and every total that reads it moves. Type a formula
and the sheet is parsed again, ordered again, and evaluated again. Those are
two different jobs, and telling them apart is what this example is about: every
message bumps `structure_rev` (which cells are formulas, what they read, in
what order they can be evaluated) or `value_rev` (only the numbers changed),
and one [`use_memo`](/reference/hooks) hangs off each. The status bar prints
how often each has run, so "typing 12 into a literal cell does not re-parse the
sheet" is a number you can watch.

The other half is which state belongs to the item and which to the whole, and
[`<VirtualList>`](/reference/elements) is what forces the question: a row that
scrolls out of view is unmounted and its hooks are swept. So the document (cell
text, column widths) lives at the top in `use_persisted`; the selection, the
editor and its draft live in a reducer at the top too, because a draft that
vanished because you scrolled would be a bug; and only state that *should*
reset on remount — "focus me on my first frame", "flash, my value just changed"
— stays in the cell. Scroll a half-typed formula off the screen and back, and
it is still there.

The row closure borrows nothing it could not borrow twice: the two memo results
(both `&'s`, so they coexist with anything), `Copy` snapshots, and no `State`
guard at all. What a cell did goes up as an event, the row turns it into a
message, and the reducer is the only thing that writes. The frozen headers
follow the body through `<VirtualList on_scroll>`.

## Run it yourself

```sh
cargo run -p spreadsheet
trunk serve --config examples/spreadsheet/Trunk.toml
```

The source is [`examples/spreadsheet/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/spreadsheet/src/lib.rs).

</ExamplePage>
