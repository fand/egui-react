# Task: canvas (PR7 = the rest of Phase 6.5, implementation paused)

## Goal

Make it possible to draw wgpu shader animations inside a component. Open the smallest hooks needed for that: a `<Canvas>` element (a leaf that gets a rect from taffy and calls `on_paint(ui, rect)`) and `examples/shader` (fullscreen triangle + fragment shader, with state flowing into uniforms). The runner-side hook `Options.setup` (called once at startup; the place to build the pipeline and put it in `callback_resources`) is done.

This was originally PR C in [docs/tasks/examples/](../examples/task.md). PR A (#4) / PR B (#6 -> #8) were merged first, and work on C stopped after only `Options.setup` was implemented, so the rest was split out as its own task. Core (`egui-react`, `egui-react-macros`) is not touched.

## Current state

- Done: `Options.setup: Option<Setup>` (`egui-react-app`). Called at the top of `ReactApp::new`. `wgpu = "30.0"` pinned in the workspace. ARCHITECTURE sections 7 / 8 updated.
- Done (decision): no `wgpu` feature. eframe 0.36 defaults to wgpu and glow is opt-in. The WebGL fallback also comes in via `egui-wgpu/default`.
- Not done: the `<Canvas>` element, `examples/shader`, gallery registration, tests, README.

## Scope

### In scope

- `<Canvas>` (`egui-react-elements`): `style` / `sense` / `on_paint` / `on_drag` / `on_hover`. taffy decides the size via `leaf_fill`. No dependency on egui-wgpu. With kittest.
- `examples/shader`: `lib.rs` (App) / `gpu.rs` (`setup(cc)`, `ShaderResources`, `ShaderCallback: CallbackTrait`) / `shader.wgsl` / `main.rs`. Slider (speed), Checkbox (pause), and drag drive the uniforms. Runs on native and trunk. Registered in the gallery, and the gallery's `setup` registers the pipeline.
- Replace the painter section of the escape-hatch example with `<Canvas>` (if it fits in one line).
- Add shader to the README table. `Canvas` in ARCHITECTURE section 6.

### Out of scope

- A raw egui version of shader (there would be no difference).
- Drawing APIs other than wgpu, egui_glow callbacks.
- Events beyond a variable `Sense` on `Canvas` (wheel, keys). When someone asks.

## Deliverables

- `crates/egui-react-elements/src/canvas.rs` + `tests/canvas.rs`.
- `examples/shader/`.
- Updates to gallery / README / ARCHITECTURE. Differences found during implementation go in plan.md section 7.

## Done criteria

- kittest: the `rect` of `Canvas` follows `w h` / `grow`. `on_drag` / `on_hover` fire. The shader Slider changes `speed`, and pause stops `request_repaint`.
- `cargo run -p shader` animates, and the Slider changes the speed (visual check). Same in the browser with `trunk serve` (visual check; browsers without WebGPU use the WebGL fallback).
- shader appears in the gallery, and pipeline registration does not clash with the other examples living next to it.
- CI (fmt / clippy / test / wasm check / trunk loop) is green.

## Decisions (assumptions at the start)

- Build the pipeline in `Options.setup`, and keep wgpu types out of hooks / context (the idea of handing out `RenderState` via `use_context` is not taken).
- `Canvas` knows nothing about egui-wgpu. The user side does `painter().add` with the callback.
- The closure prop spells out the higher-ranked bound: `impl for<'a> FnOnce(&'a mut egui::Ui, egui::Rect)`. `#[component]` rewrites elided lifetimes in props to the one of the props struct, so the elided form does not compile (confirmed with `VirtualList`).
- If `#[prop(default = ..)]` does not accept an expression (`egui::Sense::hover()`), use `Option<egui::Sense>` + `unwrap_or`.
- Snapshot (C-3) depends on whether `callback_resources` can be injected into kittest's `WgpuTestRenderer`. If not, drop it and rely on visual checks only.
