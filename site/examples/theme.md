---
title: theme
---

# theme

Two values provided at the top and read three levels down, with nothing in
between.

<script setup>
import { data } from '../examples.data.js'
</script>

<ExampleEmbed
  name="theme"
  :has-plain="data.theme.hasPlain"
  :react-lines="data.theme.reactLines"
  :plain-lines="data.theme.plainLines"
>

<div data-version="react">

::: example-source theme
:::

</div>

</ExampleEmbed>

`Themed` owns a `Theme` and a `Locale` and provides both to its children.
`Page` and `Card` sit in between and take no props at all — they do not know a
theme or a locale exists. The leaves three levels down read them with
[`use_context`](/reference/hooks), and `Toggles`, a child of the provider
itself, writes them back through the same `Handle`. `Orphan` at the bottom is
drawn outside `Themed`, so its `use_context` returns `None`: a binding lasts
exactly as long as the children it was provided to.

Note the shape of the provider, because it is the shape you will copy.
`provide_context` takes its children as a closure, and the `Handle` it
publishes borrows the store, so the handle has to be created in the same
component body that provides it — there is no `<Provide value={handle}>`
element, and cannot be, because a props type may not name the store's lifetime.
The provider is also `#[component(shares_ui)]`: it draws nothing of its own, so
its children should become nodes of the parent's layout tree rather than of a
tree of their own.

The value is keyed by type, which is why `Theme` and `Locale` are two distinct
types rather than two fields — `use_context::<Theme>` finds this and nothing
else. And `use_handle`, not `use_state`: a `Handle` is `Copy` and holds no
borrow, so a subtree can write back through it while the provider is still
running.

## Run it yourself

```sh
cargo run -p theme
trunk serve --config examples/theme/Trunk.toml
```

The source is [`examples/theme/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/theme/src/lib.rs).
