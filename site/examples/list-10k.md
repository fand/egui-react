---
title: list-10k
---

<script setup>
import { data } from '../examples.data.js'
</script>

<ExamplePage
  name="list-10k"
  :has-plain="data['list-10k'].hasPlain"
  :react-lines="data['list-10k'].reactLines"
  :plain-lines="data['list-10k'].plainLines"
>

<template v-slot:summary>

Ten thousand rows, and what drawing all of them costs.

</template>

<template v-slot:code>

<div data-version="react">

::: example-source list-10k
:::

</div>

<div data-version="plain">

::: example-source list-10k plain
:::

</div>

</template>

[`<VirtualList>`](/reference/elements) draws only the rows the viewport can see
and reserves the height of the rest, so the frame time does not depend on the
count. `render` is called with the index of each row that is on screen, the way
react-virtualized calls `rowRenderer`; what the app owns is the data, and
`filtered` is memoised because building ten thousand `String`s every frame
would cost more than drawing them.

This is also the honest example. `<VirtualList>` wraps
`egui::ScrollArea::show_rows`, which is exactly what the plain egui version
next to it calls — so the two tabs come out at the same length, and nothing
about virtualization is a library feature. What the library adds here
is that a row can have hooks: rows are drawn inside a scope of their own, like
`for` plus `key={i}`. The repository's own measurements for drawing every row
instead are in `tests/scenarios.rs` and `docs/tasks/list-perf/`.

Two notes about what you are looking at. The embedded version starts at 1,000
rows, to match the plain egui version beside it; the standalone binary opens
with a hundred thousand, and the slider reaches that either way, since the rows
are virtualised. And every row must be the same height — the list moves on by
exactly `row_h` whatever a row draws, which is what keeps it in step with the
range it asked for.

## Run it yourself

```sh
cargo run -p list-10k
cargo run -p list-10k --bin list-10k-plain    # the plain egui version
trunk serve --config examples/list-10k/Trunk.toml
```

The source is [`examples/list-10k/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/list-10k/src/lib.rs)
and [`plain.rs`](https://github.com/fand/egui-react/blob/main/examples/list-10k/src/plain.rs).

</ExamplePage>
