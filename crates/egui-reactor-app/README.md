# egui-reactor-app

The runner for [egui-reactor](https://crates.io/crates/egui-reactor) apps. `run(Options, |cx| rsx! { <App/> })` opens a window on native and takes over a `<canvas>` on wasm. Also owns fonts (`fonts`) and web accessibility (`a11y`).

```rust
use egui_reactor::prelude::*;
use egui_reactor_app::{Options, run};

fn main() -> eframe::Result {
    run(Options::default(), |_cx| rsx! { <App/> })
}
```

Features: `default_fonts` (on) keeps egui's built-in fonts; `woff2` (off) decodes WOFF/WOFF2 fonts fetched over HTTP.

Docs: https://fand.github.io/egui-reactor/getting-started
