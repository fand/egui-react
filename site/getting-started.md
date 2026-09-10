---
title: Getting Started
---

# Getting Started

A counter in a window, then the same counter in a browser tab.

## Prerequisites

Stable Rust with the 2024 edition.

```sh
rustup update stable
```

For the web build, also install [trunk](https://trunkrs.dev/) and the wasm
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
egui-reactor = { git = "https://github.com/fand/egui-react" }
egui-reactor-elements = { git = "https://github.com/fand/egui-react" }
egui-reactor-app = { git = "https://github.com/fand/egui-react" }
egui = "0.36.1"
eframe = "0.36.1"
```

- `egui-reactor` is the core: `Cx`, hooks, `rsx!`, the layout engine.
- `egui-reactor-elements` is what you write inside `rsx!`: `<View>`, `<Text>`,
  `<Button>` and the rest.
- `egui-reactor-app` opens a window or takes over a canvas.
- `egui` and `eframe` are needed because your code names their types.

## The counter

This is `examples/counter` from the repository.

### The component

```rust
use egui_reactor::prelude::*;
use egui_reactor_elements::prelude::*;

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

`#[component]` makes a function usable as `<App/>`. The first argument is
always `cx: &mut Cx`. Every other argument is a prop.

`use_state(cx, || 0i32)` returns a guard onto a slot in the hook store. It
derefs, so `*count += 1` is the whole API. There is no `set_count`. The three
handlers each borrow `count` as `&mut`, one after another. That works because
handlers run during this frame and are dropped right after.

`rsx!` expands into egui calls in place. `<Button on_click={..}>` means "draw a
button now, and if it was clicked, run this closure now".

- Text is always a quoted string.
- `{expr}` embeds an expression.
- `direction`, `gap` and `grow` are layout attributes. See [Layout](/guide/layout).

### The runner

```rust
use egui_reactor::prelude::*;
use egui_reactor_app::{Options, run};

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-reactor: counter"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
```

`run` owns the hook store and draws your root view each frame, on native and
wasm alike. Keep hooks out of the root closure: a guard created there cannot
outlive it. Put them in a component.

## Run it natively

```sh
cargo run
```

## Run it in a browser

Put an `index.html` next to `Cargo.toml`. The canvas id must match
`Options::canvas_id`, which defaults to `egui_reactor_canvas`.

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
    <canvas id="egui_reactor_canvas"></canvas>
  </body>
</html>
```

Then:

```sh
trunk serve
```

Keep `data-wasm-opt="z"`. An egui app is a few megabytes of wasm before
optimisation. See [Web and native](/guide/web-and-native).

## Cargo features

`egui-reactor-app` has two.

- **`default_fonts`** (on) keeps egui's four built-in fonts, about 1.4 MB.
  Turn it off to save the bytes, but then you must bundle a font of your own
  and apply it before the first frame. Otherwise nothing is drawn.
- **`woff2`** (off) decodes WOFF and WOFF2 fonts fetched over HTTP. Without
  it, such a fetch fails instead of panicking.

See [Fonts](/guide/fonts).

## Next

- [How it works](/how-it-works): what immediate mode changes about React
  habits.
- [rsx!](/guide/rsx), [Components and events](/guide/components-and-events),
  [State and hooks](/guide/state-and-hooks): the syntax.
- [Hooks](/reference/hooks) and [Elements](/reference/elements): the lists.
- [Examples](/examples/counter): every one runs on its page.
