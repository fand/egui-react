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

</ExampleEmbed>

The three handlers each take `count` as `&mut`, one after another. Nothing is
cloned and nothing is `'static`: the closures run during the pass that built
them, so the borrow checker is satisfied by ordinary scoping.

`align="center" justify="center"` on the outer `<View>` is what centres the
block; in plain egui, that means measuring the block first and allocating
the space around it by hand.

## Run it yourself

```sh
cargo run -p counter
trunk serve --config examples/counter/Trunk.toml
```

The source is [`examples/counter/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/counter/src/lib.rs).
