---
title: Web and native
---

# Web and native

One `run` call opens a window natively and takes over a `<canvas>` in the
browser. The runner is `egui-react-app`, the only crate that knows about
eframe.

## `run(Options, root)`

```rust
fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-react: counter"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
```

Each frame the runner wraps everything in a `CentralPanel` and draws your
root view inside a root layout node that fills the window (column, width
`100%`, min height `100%`). So a child of the root can use `grow` and
`justify` right away.

Keep hooks out of the root closure. A guard created there cannot outlive it.
Put hooks in a component and keep the root as `|_cx| rsx! { <App/> }`.

## `Options`

| Field | Meaning |
|---|---|
| `title` | Native window title. Ignored on wasm |
| `max_passes` | Passes egui may run per frame, 3 by default. The layout asks for another pass when a node is added, removed or moved |
| `persist` | Whether `use_persisted` reads and writes eframe's storage |
| `canvas_id` | The `<canvas>` to attach to. wasm only. `"egui_react_canvas"` by default |
| `native` | `eframe::NativeOptions`. Native only |
| `setup` | Called once, as soon as eframe has a window and a render backend |

```rust
Options {
    title: String::from("my app"),
    #[cfg(not(target_arch = "wasm32"))]
    native: eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        ..Default::default()
    },
    ..Default::default()
}
```

## `setup`, and wgpu

`setup` runs on native and web alike, before anything is drawn. No wgpu type
appears in the hook API, so an app that does not use wgpu never sees one.

```rust
Options {
    setup: Some(Box::new(move |cc| {
        // Build the pipeline and put it in
        // `cc.wgpu_render_state`'s `callback_resources`, keyed by type.
        shader::gpu::setup(cc);
        cc.egui_ctx.add_plugin(WebA11y::new(canvas_id));
    })),
    ..Default::default()
}
```

Drawing goes through `<Canvas>`. Its `paint` closure gets a rect and pushes
an `egui_wgpu::Callback::new_paint_callback(rect, ..)` onto the painter.
[shader](/examples/shader) is the whole round trip. [patch](/examples/patch)
generates the WGSL it runs.

## The web build

You need an `index.html` with a canvas whose id matches `canvas_id`:

```html
<link data-trunk rel="rust" href="Cargo.toml" data-wasm-opt="z" />
<canvas id="egui_react_canvas"></canvas>
```

plus CSS to make the canvas fill the page. Then `trunk serve`, or
`trunk build --release` for a `dist/` to deploy.

The renderer is wgpu with a WebGL fallback, from eframe's defaults.

### Size

An egui app is a few megabytes of wasm. Keep `data-wasm-opt="z"`, build with
`--release`, and turn off `default_fonts` if you ship your own font (about
1.4 MB). See [Fonts](/guide/fonts).

### Accessibility on the web

egui builds an AccessKit tree every frame. Natively, eframe hands it to the
OS and screen readers work. On the web, eframe drops it. `egui-react-app`
ships `a11y::WebA11y`, a plugin that mirrors the tree into hidden DOM
elements over the canvas:

```rust
setup: Some(Box::new(move |cc| {
    cc.egui_ctx.add_plugin(egui_react_app::a11y::WebA11y::new("egui_react_canvas"));
})),
```

On any target but wasm it is empty, so register it unconditionally. It turns
AccessKit on, which costs one node per widget per frame.

A screen reader can read the mirror, and it reads `label` on `<Button>` and
`alt` on `<Image>`, so fill them in. But the app is still a canvas. Find in
page, search engines and browser translation see nothing. That is why this
site is HTML and only the examples are wasm.

## See it running

Every example page is the same wasm build, told which example to draw by the
URL. [shader](/examples/shader) is the wgpu path. [font](/examples/font)
fetches a font over HTTP. [shell](/examples/shell) runs natively only, since
docked panels carve up the window. The runner and platform notes are in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#7-crate-layout)
sections 7 and 8.
