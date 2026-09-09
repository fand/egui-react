---
title: Getting Started
---

# Getting Started

A counter in a window, then the same counter in a browser tab.

## Prerequisites

Stable Rust, nothing else. The repository pins a toolchain for its own CI, but
nothing in the library needs it: any recent stable compiler with the 2024
edition will do.

```sh
rustup update stable
```

For the web build you also need [trunk](https://trunkrs.dev/) and the wasm
target:

```sh
cargo install --locked trunk
rustup target add wasm32-unknown-unknown
```

## Add the dependencies

The crates are not on crates.io yet, so take them from git:

```toml
[package]
name = "my-app"
version = "0.1.0"
edition = "2024"

[dependencies]
egui-react = { git = "https://github.com/fand/egui-react" }
egui-react-elements = { git = "https://github.com/fand/egui-react" }
egui-react-app = { git = "https://github.com/fand/egui-react" }
egui = "0.36.1"
eframe = "0.36.1"
```

Three crates, because they are three jobs. `egui-react` is the core: `Cx`,
hooks, `rsx!`, the layout engine. `egui-react-elements` is what you write
inside `rsx!`: `<View>`, `<Text>`, `<Button>` and the rest.
`egui-react-app` is the runner that opens a window or takes over a canvas.
`egui` and `eframe` come in directly because your own code names types from
them (`egui::Color32`, `eframe::Result`).

## The counter

Everything below is `examples/counter` in the repository, unchanged.

### The component

```rust
use egui_react::prelude::*;
use egui_react_elements::prelude::*;

#[component]
pub fn App(cx: &mut Cx) {
    let mut count = use_state(cx, || 0i32);

    rsx! {
        <View direction="column" align="center" justify="center" gap={12} grow={1.0}>
            <Text size={32.0} strong>{format!("{}", *count)}</Text>
            <View direction="row" gap={8}>
                <Button on_click={|| *count -= 1}>"-"</Button>
                <Button on_click={|| *count = 0}>"reset"</Button>
                <Button on_click={|| *count += 1}>"+"</Button>
            </View>
        </View>
    }
}
```

`#[component]` turns a function into something `rsx!` can write as `<App/>`.
Its first argument is always `cx: &mut Cx`; every other argument becomes a prop.

### The state

`use_state(cx, || 0i32)` returns a guard onto a slot in the hook store. It
derefs, so `*count += 1` and `*count = 0` are the whole API. The initialiser
runs only the first time this component instance is drawn; after that the value
comes back from the store, which is keyed by where the component sits in the
tree.

There is no `set_count`. Three handlers each take `count` as `&mut`, one after
another, and the borrow checker is satisfied by ordinary scoping — because the
handlers run *during* this pass and are then thrown away. Nothing here is
`'static`, cloned or `Rc`.

### The markup

`rsx!` is not a template that gets interpreted later. It expands into egui
calls in place, so `<Button on_click={..}>` really is "draw a button now, and
if it was clicked run this closure now".

- `"-"` is a string literal: text inside `rsx!` is always quoted.
- `{format!("{}", *count)}` embeds an expression.
- `direction`, `gap`, `grow` are layout attributes; they go to the flexbox
  engine, which is [taffy](https://github.com/DioxusLabs/taffy) under a small
  engine of our own.

### The runner

```rust
use egui_react::prelude::*;
use egui_react_app::{Options, run};

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-react: counter"),
            ..Default::default()
        },
        // The root closure gets a `Cx` it rarely needs: hooks belong in
        // components, so that a view can borrow their guards.
        |_cx| rsx! { <App/> },
    )
}
```

`run` owns the hook store, opens a pass around your root view each frame, and
absorbs the difference between native and wasm. Keep the root closure as
`|_cx| rsx! { <App/> }`: a hook called *in* the root would hand out a guard
that the returned view cannot outlive.

## Run it natively

```sh
cargo run
```

## Run it in a browser

Put an `index.html` next to `Cargo.toml`. The one thing that matters is the
canvas id: the runner looks up `Options::canvas_id`, which defaults to
`egui_react_canvas`.

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1, user-scalable=no" />
    <title>counter</title>
    <link data-trunk rel="rust" href="Cargo.toml" data-wasm-opt="z" />
    <style>
      html,
      body {
        margin: 0;
        height: 100%;
        overflow: hidden;
        background: #101010;
      }
      canvas {
        display: block;
        width: 100%;
        height: 100%;
      }
    </style>
  </head>
  <body>
    <canvas id="egui_react_canvas"></canvas>
  </body>
</html>
```

Then:

```sh
trunk serve
```

`data-wasm-opt="z"` is worth keeping: an egui app is a few megabytes of wasm
before optimisation. More on the web build in
[Web and native](/guide/web-and-native).

## Features

`egui-react-app` has two cargo features.

**`default_fonts`** (on by default) keeps the four fonts epaint compiles in —
Ubuntu-Light, Hack and two emoji fonts, about 1.4 MB together. Turn it off and
the bytes go away, but so does every glyph: nothing panics, and nothing is
drawn either, because there is no built-in face left to stand behind a font
chain. An app that turns it off has to hand the runner a bundled face and apply
it before the first frame.

```toml
egui-react-app = { git = "https://github.com/fand/egui-react", default-features = false }
```

**`woff2`** (off by default) decodes WOFF and WOFF2 for fonts fetched over
HTTP. It is off because the Brotli tables are not small and a font served next
to your `index.html` is normally a TTF; without it a `.woff2` response is
reported as failed rather than panicking. See [Fonts](/guide/fonts).

## Where to go next

- [Thinking in egui-react](/guide/thinking-in-egui-react) — what immediate mode
  changes about React habits. Read this one first.
- [rsx!](/guide/rsx) and [Components and events](/guide/components-and-events) —
  the syntax in full.
- [State and hooks](/guide/state-and-hooks) — guards, `bind`, reducers,
  persistence.
- [Hooks reference](/reference/hooks) and
  [Elements reference](/reference/elements) — the lists.
- [counter](/examples/counter) and the rest of the
  [examples](/examples/showcase) — every one of them runs on its page.
