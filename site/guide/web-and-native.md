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

Per frame the runner wraps everything in a `CentralPanel`, opens a pass on the
hook store, and draws your root view inside a root layout node that reserves
the whole window (`direction: column`, width `100%`, minimum height `100%`).
That is why a child of the root can use `grow` and `justify` straight away.

### Keep hooks out of the root closure

`root` is called every pass, and the view it returns may not borrow anything
created inside it — a hook called there hands out a guard, and the `rsx!` that
reads it cannot outlive the closure body. Put hooks in a component and keep the
root as `|_cx| rsx! { <App/> }`. The `Cx` argument is there for escape-hatch
roots and is normally unused.

## `Options`

| Field | Meaning |
|---|---|
| `title` | The native window title. Ignored on wasm |
| `max_passes` | How many passes egui may run for one frame; 3 by default. The layout engine asks for another pass when a node was created, removed or moved, and each level of nesting inside an egui container can need one more. Deeper nesting settles on the next frame instead |
| `persist` | Whether `use_persisted` is saved to and loaded from eframe's storage |
| `canvas_id` | The id of the `<canvas>` to attach to. wasm only; `"egui_react_canvas"` by default |
| `native` | Everything else eframe accepts (`eframe::NativeOptions`). Native only |
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

`setup` is a hole in the runner rather than a hook, on purpose: no wgpu type
appears anywhere in the hook API, and an app that does not draw with wgpu never
sees one. It runs on native and on the web alike, before anything is drawn,
which is what a paint callback added on frame one needs.

```rust
Options {
    setup: Some(Box::new(move |cc| {
        // Build the pipeline and put it in `cc.wgpu_render_state`'s
        // `renderer.write().callback_resources`, keyed by its own type.
        shader::gpu::setup(cc);
        cc.egui_ctx.add_plugin(WebA11y::new(canvas_id));
    })),
    ..Default::default()
}
```

Drawing then goes through `<Canvas>`: it asks the layout for a rect and hands
it to your `paint` closure, which pushes an
`egui_wgpu::Callback::new_paint_callback(rect, ..)` onto the painter. The
[shader](/examples/shader) example is the whole round trip — a slider in a hook,
a uniform buffer, a fragment shader — and [patch](/examples/patch) generates the
WGSL it runs.

## The web build

You need an `index.html` with a canvas whose id matches `canvas_id`:

```html
<link data-trunk rel="rust" href="Cargo.toml" data-wasm-opt="z" />
<canvas id="egui_react_canvas"></canvas>
```

and the usual CSS to make the canvas fill the page (`html, body { margin: 0;
height: 100% }`, `canvas { display: block; width: 100%; height: 100% }`). Then
`trunk serve`, or `trunk build --release` for a deployable `dist/`.

The renderer is wgpu, with a WebGL fallback for browsers without WebGPU; both
come from eframe's defaults, so there is nothing to configure.

### Size

An egui app is a few megabytes of wasm. `data-wasm-opt="z"` in the trunk link
is worth keeping — it is what the examples in this repository use. Two other
levers: build with `--release`, and turn off `default_fonts` if you are
shipping your own font anyway (about 1.4 MB), remembering what that means from
[Fonts](/guide/fonts).

### Accessibility on the web

egui builds an AccessKit tree every pass. Natively, eframe hands it to the OS
and a screen reader reads your app with no work from you. On the web, eframe
throws that tree away — so `egui-react-app` ships `a11y::WebA11y`, an egui
plugin that picks the tree up first and mirrors it into hidden DOM elements
over the canvas:

```rust
setup: Some(Box::new(move |cc| {
    cc.egui_ctx.add_plugin(egui_react_app::a11y::WebA11y::new("egui_react_canvas"));
})),
```

On any target but wasm it is an empty plugin, so registering it unconditionally
is fine. Registering it turns AccessKit on, which costs one node per widget per
pass — do not register it if you do not want to pay that.

Be honest with yourself about what this does and does not buy. A screen reader
can read the mirror, and element labels (`label` on `<Button>`, `alt` on
`<Image>`) are what it reads, so fill them in. But the app is still a canvas:
the browser's find-in-page has nothing to find, search engines see an empty
page, and the browser's translation cannot touch it. That is why this
documentation site is HTML and only the running examples are wasm.

## See it running

Every example page on this site is the same wasm build, told which example to
draw by the URL hash. [shader](/examples/shader) is the wgpu path,
[font](/examples/font) fetches a font over HTTP in the browser, and
[shell](/examples/shell) is the one that only runs natively, because docked
panels carve up the window itself.

The runner's frame, `Options` in full, and the platform notes are in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#7-crate-layout)
sections 7 and 8.
