---
title: counter
---

# counter

One piece of state, three handlers that borrow it in turn.

<script setup>
import { data } from '../examples.data.js'
</script>

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

The three handlers each take `count` as `&mut`, one after another. Nothing is
cloned and nothing is `'static`: the closures run during the pass that built
them, so the borrow checker is satisfied by ordinary scoping.

The plain egui version keeps the same state in a struct of its own and centres
the block by measuring it first, because egui lays out from the top left and
has no notion of free space.

## Run it yourself

```sh
cargo run -p counter
cargo run -p counter --bin counter-plain    # the plain egui version
trunk serve --config examples/counter/Trunk.toml
```

The source is [`examples/counter/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/counter/src/lib.rs)
and [`plain.rs`](https://github.com/fand/egui-react/blob/main/examples/counter/src/plain.rs).
