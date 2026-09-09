---
title: todo
---

# todo

A reducer drives the list; `use_persisted` keeps it across restarts.

<script setup>
import { data } from '../examples.data.js'
</script>

<ExampleEmbed
  name="todo"
  :has-plain="data.todo.hasPlain"
  :react-lines="data.todo.reactLines"
  :plain-lines="data.todo.plainLines"
>

<div data-version="react">

::: example-source todo
:::

</div>

<div data-version="plain">

::: example-source todo plain
:::

</div>

</ExampleEmbed>

Everything that can change the list is a message — `Add`, `Toggle`, `Remove`,
`ClearDone` — and one `reduce` function applies them.
[`use_reducer`](/reference/hooks) hands back the state and a `Dispatch`, so a
button deep in the list can say what happened without a chain of callbacks
above it.

The data itself comes from `use_persisted`, keyed by the explicit string
`"todo/todos"`. Persisted slots are identified by that key rather than by the
call site, precisely so that inserting a line above does not make saved data
unreadable — and the example prefixes the key with its own name because
everything an app persists shares one namespace. Run it natively, quit, and
start it again: the list is still there, in eframe's storage.

The non-obvious bit is inside the loop. `todos` is borrowed by the `for`, so a
handler there cannot touch it: the remove button sends `Msg::Remove(i)` and the
checkbox binds to a scratch copy, with the real change going through the
`Dispatch` and landing on the next frame. A reducer is one way out of that
borrow; [`update_later`](/guide/state-and-hooks) is the other.

Watch the two tabs. The plain egui version keeps the same `Vec<Todo>` and does
the same work; what it also keeps is the bookkeeping — the messages, the
persistence, and the deletions it has to defer by hand.

## Run it yourself

```sh
cargo run -p todo
cargo run -p todo --bin todo-plain    # the plain egui version
trunk serve --config examples/todo/Trunk.toml
```

The source is [`examples/todo/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/todo/src/lib.rs)
and [`plain.rs`](https://github.com/fand/egui-react/blob/main/examples/todo/src/plain.rs).
