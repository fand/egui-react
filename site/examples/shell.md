---
title: shell
---

# shell

Docked panels, a floating window, and an editor in what is left.

## No live version

This is the one example that does not run on its page. A docked `<Panel>`
carves its space out of the nearest enclosing egui `Ui` — under the runner,
that is the window — so it wants to be at the root of an app, not inside a box
on a documentation page. The embed that runs every other example here draws it
into a column of a larger app, and there is nothing sensible a panel can do
with that.

It runs perfectly well on its own, natively or in a browser tab of its own; the
commands are at the bottom of this page.

::: example-source shell
:::

Two rules are the whole trick. Panels and `<CentralPanel>` are written as
**siblings**, not nested: each takes a bite out of what is left, in the order
they appear, and the central panel gets the rest. Nesting them would put a
panel inside a panel. And a panel docks in the nearest enclosing egui `Ui`, so
any `<View>` in between is skipped — it makes no difference whether the panels
are at the root of `App` or further in.

The second rule has a consequence worth remembering: everything inside a panel
is drawn in plain `Ui` mode, so layout attributes like `grow` and `justify` do
nothing there until a `<View>` starts a layout tree of its own, as the toolbar
and the inspector here do. The `<Window>` is the other half of the shell —
floating, with `open` bound to a `bool`, so not drawing it unmounts its
children.

`Panel`, `CentralPanel` and `Row` are the three elements written with
`#[component(shares_ui)]`, which is exactly what lets them draw into their
parent's surface instead of a child `Ui` of their own. See
[Components and events](/guide/components-and-events) for what that attribute
means, and [Elements](/reference/elements) for the props.

## Run it yourself

```sh
cargo run -p shell
trunk serve --config examples/shell/Trunk.toml
```

The source is [`examples/shell/src/lib.rs`](https://github.com/fand/egui-react/blob/main/examples/shell/src/lib.rs).
