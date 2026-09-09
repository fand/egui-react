---
title: board
---

<script setup>
import { data } from '../examples.data.js'
</script>

<ExamplePage
  name="board"
  :has-plain="data.board.hasPlain"
  :react-lines="data.board.reactLines"
  :plain-lines="data.board.plainLines"
>

<template v-slot:summary>

Cards that keep the title being typed into them while they are dragged between
columns.

</template>

<template v-slot:code>

<div data-version="react">

::: example-source board
:::

</div>

<div data-version="plain">

::: example-source board plain
:::

</div>

</template>

Every other example holds its state in one place and draws it. This one is
about state that belongs to the *items*: the half-typed title someone is in the
middle of, and what happens to it when the cards are reordered, moved to
another column, filtered away and brought back. Start a card's title, then drag
that card to another column — the draft goes with it.

That works because the draft is keyed by the card's identity rather than by
where the card is drawn. `use_identity`, one of the example's own
[custom hooks](/guide/state-and-hooks), is what makes the difference; `key=`
alone would not, because moving a card changes its parent. A card that is
deleted — or filtered out of view, which unmounts it just the same — has its
state dropped by the end-of-pass sweep, which is the housekeeping the plain
egui version has to write out by hand.

The rest is composition. `<Toolbar>`, `<Column>`, `<Card>`, `<TitleEdit>`,
`<Chip>` and `<IconButton>` each take a `style: ItemStyle` so the caller
decides where they sit, and report what happened through
[event props](/guide/components-and-events). Undo/redo, the search debounce and
the whole drag session live in custom hooks written against the same public API
an application has — none of them needed a change to the library. Which way
something travels is a decision made twice here: what a component *did* goes up
as an event, and what the whole tree shares (the theme, the drag in progress,
the `Dispatch`) comes down through
[context](/reference/hooks).

Compare the tab labels. The plain egui version does the same job; the
difference is where the per-card state has to live.

## Run it yourself

```sh
cargo run -p board
cargo run -p board --bin board-plain    # the plain egui version
trunk serve --config examples/board/Trunk.toml
```

The source is [`examples/board/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/board/src/lib.rs)
and [`plain.rs`](https://github.com/fand/egui-react/blob/main/examples/board/src/plain.rs).

</ExamplePage>
