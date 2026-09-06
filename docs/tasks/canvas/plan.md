# Plan: canvas

The task definition is in [task.md](task.md). The design rationale is in [docs/ARCHITECTURE.md](../../ARCHITECTURE.md). This document is split out of [docs/tasks/examples/plan.md](../examples/plan.md) section 3 (PR C); 3.1 (the `wgpu` feature) turned out to be unnecessary and was dropped, and 3.2 is already implemented. If we decide to deviate from this during implementation, update this document, and update ARCHITECTURE.md too if the change matters for the design.

## 0. Overview

One PR. We touch `egui-react-elements` (`Canvas`), `examples/shader`, `examples/gallery` (registration and `setup`), `examples/escape-hatch` (replacing the painter section, optional), README, and ARCHITECTURE. `egui-react-app` is done (`Options.setup`). Core is unchanged.

Correction to an assumption: eframe 0.36 defaults to wgpu (glow is opt-in), and the WebGL fallback comes in via `egui-wgpu/default`. We do not add a feature to pick the backend.

## 1. Design (from examples plan 3.2 to 3.5)

### 3.2 `Options.setup`

```rust
pub struct Options {
    ..
    /// Called once right after eframe starts. The place to build the wgpu
    /// pipeline and put it in `render_state.renderer.write().callback_resources`.
    pub setup: Option<Box<dyn FnOnce(&eframe::CreationContext<'_>)>>,
}
```

Called at the top of `ReactApp::new`. Same shape as the official egui demo (`custom3d_wgpu`). The idea of handing out `RenderState` via `use_context` is not taken; it gains little for the effort of making a slot. No wgpu types show up in hooks or context.

### 3.3 The `<Canvas>` element (`egui-react-elements`)

```rust
#[component]
pub fn Canvas(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    #[prop(default)] sense: egui::Sense,          // default hover
    on_paint: impl FnOnce(&mut egui::Ui, egui::Rect),
    #[event] on_drag: egui::Vec2,                  // drag_delta
    #[event] on_hover: egui::Pos2,                 // pointer position inside the rect
)
```

- `cx.leaf_fill(&style, |ui| { let (rect, resp) = ui.allocate_exact_size(ui.available_size(), sense); on_paint(ui, rect); resp })`. Because it is `leaf_fill`, taffy decides the size (`w h` / `grow`).
- elements does not depend on egui-wgpu. The example side does `ui.painter().add(..)` with the callback. It also works for uses that only draw lines with `egui::Painter` (plots and so on).
- Tests (kittest, headless): the `rect` passed to `on_paint` has the size given by `w h`. With `grow={1.0}` it takes all the remaining space. Dragging fires `on_drag`.

### 3.4 `examples/shader`

```
examples/shader/src/
  lib.rs      App: <Canvas grow> + Slider(speed) + Checkbox(pause) + ComboBox(shader selection)
  gpu.rs      setup(cc), ShaderResources, ShaderCallback: CallbackTrait
  shader.wgsl fullscreen triangle + fragment. uniform { time, resolution, mouse, speed }
  main.rs     run(Options { setup: Some(Box::new(gpu::setup)), .. }, ..)
```

```rust
#[component]
fn App(cx: &mut Cx) {
    let mut speed = use_state(cx, || 1.0f32);
    let mut paused = use_state(cx, || false);
    let mut mouse = use_state(cx, || egui::Vec2::ZERO);
    let time = cx.ui().input(|i| i.time) as f32;
    if !*paused {
        cx.ui().ctx().request_repaint();
    }
    rsx! {
        <View direction="column" gap={8} grow={1.0}>
            <Canvas
                grow={1.0}
                on_drag={|d: egui::Vec2| *mouse += d}
                on_paint={|ui: &mut egui::Ui, rect| {
                    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                        rect,
                        gpu::ShaderCallback { time: time * *speed, mouse: *mouse },
                    ));
                }}
            />
            <View direction="row" gap={8} align="center">
                <Slider bind={speed.bind()} range={0.0..=4.0} label="speed"/>
                <Checkbox bind={paused.bind()} label="pause"/>
            </View>
        </View>
    }
}
```

- `ShaderCallback::prepare` writes the uniforms with `queue.write_buffer`, and `paint` draws 3 vertices. egui-wgpu fits the viewport / scissor to `rect`.
- The point of the demo is that `state` flows straight into the uniforms. `Slider`'s `bind` -> `speed` -> uniform.
- `ShaderResources` is stored by type in `callback_resources` (a `TypeMap`). Living next to other examples in the gallery does not clash as long as the types differ.
- Multiple passes: egui throws away the shapes of a discarded pass. The callback never runs twice.
- The gallery turns on `egui-react-app/wgpu` and calls `shader::gpu::setup(cc)` in `setup`.
- No raw egui version (the wgpu part would be the same code, so there would be no difference).

### 3.5 Tests

| # | Content |
|---|---|
| C-1 | `Canvas`'s `rect` and `on_drag` (3.3, headless) |
| C-2 | shader: moving the Slider changes `speed`, and pause stops `request_repaint` (watch the `harness` repaint request) |
| C-3 | snapshot (feature `snapshot`, local only): one image with time fixed at 0. Whether a `setup` equivalent can be fed into the render state of kittest's `WgpuTestRenderer` is in section 5 |


## 2. Steps

1. `<Canvas>` + kittest (3.3 in section 1). ARCHITECTURE section 6. Commit.
2. `examples/shader` (native -> trunk). Register in the gallery and call `shader::gpu::setup(cc)` in the gallery's `Options.setup`. README table. Commit.
3. Replace the painter section of escape-hatch with `<Canvas>` (only if it fits in one line). Commit.
4. Try the snapshot (C-3). If it is not possible, write the reason in section 3. PR.

## 3. Differences found during implementation

### Step 1: `Canvas`

**The closure prop is named `paint`, not `on_paint`.** `rsx!` treats any attribute whose name starts with `on_` as an event handler and looks for the `<ElementName>Event` variant (`on_paint` -> `CanvasEvent::Paint`) (attribute dispatch in `crates/egui-react-macros/src/rsx/mod.rs`). So a plain prop starting with `on_` cannot be written from rsx!. The ways out are "change core" or "change the name"; since core stays unchanged, we took the latter. It matches the naming of `VirtualList`'s `render`, and keeps the reading "`on_*` = event, everything else = prop". The shader example code in 3.4 also becomes `paint={..}`.

`#[prop(default = egui::Sense::hover())]` worked (the "if it does not work, use `Option<egui::Sense>`" in task.md was not needed). Events fire from `Response`: `on_drag(drag_delta())` when `dragged()`, and `on_hover(pos)` when `hover_pos()` is present. kittest has 4 tests in `crates/egui-react-elements/tests/canvas.rs` (rect matching `w`/`h`, all remaining space with `grow`, drag delta, hover position). Event handlers cannot be written with `move` (the fused closure is `FnMut`, so the test's `Rc` is captured by borrow).

### Step 2: `examples/shader`

**`<Canvas grow={1.0}>` alone is not enough. Add `h={0.0}` next to it.** `leaf_fill` passes `infinite: Vec2b::TRUE` to egui_taffy, so the max-content measurement becomes the height of the root itself (the closure in `egui_taffy`'s `compute_layout_with_measure`). The runner's root has `min_h: 100%` and height `auto`, so the column height becomes "the whole canvas + the other children", and the Slider and Checkbox overflow below the window (y=588 in a 520px window). `basis={0.0}` / `min_h={0.0}` / the parent's `h="100%"` have no effect (a % against an auto parent falls back to auto). What worked is the usual flexbox trick, `h={0.0}` + `grow={1.0}`. We wrote a comment in the example and pinned it with a test (`the_controls_stay_below_the_canvas`). The kittest in 3.3 could not catch this because both cases set an explicit `h` on the parent.

**The wgpu 30 API differs a bit from the sketch in 3.4.** `PipelineLayoutDescriptor` has `immediate_size: u32` instead of `push_constant_ranges`, `bind_group_layouts` is `&[Option<&BindGroupLayout>]`, and `RenderPipelineDescriptor` has `multiview_mask` instead of `multiview`.

**`speed` was also put into the uniform** (3.4 only had `{ time, mouse }`). The color is driven by `speed` so you can see the Slider still works while paused. The layout is `{ time, speed, resolution, mouse }` + 8 bytes of tail padding = 32 bytes (`vec2<f32>` is on an 8-byte boundary in a uniform block). We use `bytemuck`'s `Pod`/`Zeroable` derives, so `bytemuck` was added to `[workspace.dependencies]`. Same for `egui-wgpu = "0.36.1"` (same version and same features as eframe).

**The fragment `@builtin(position)` is in whole-framebuffer coordinates; the origin is not the top-left of the rect.** Shifting the viewport does not move fragcoord, so we pass clip coordinates as a varying and build -1..1 from them. No need to add the rect position to the uniform.

**The test (C-2) watches `harness.ctx.has_requested_repaint()`.** While animating, every frame requests a repaint, so `run()` panics at max steps. As with clock, use `step()`, and only after pause let it settle with `run_ok()`. The paint callback is only pushed onto the painter, and kittest's default renderer does not run callbacks, so headless is safe (`gpu::setup` is not called either).

**No snapshot (C-3) for shader.** There is no hook to inject a `setup` equivalent into `WgpuTestRenderer`'s `RenderState`, so `ShaderResources` is not found and nothing is drawn. On top of that, it requests a repaint every frame, so the image never settles. The reason is also written in `examples/gallery/tests/snapshots.rs`. Visual check only.

**No ComboBox (shader selection).** It was in the file layout comment in 3.4, but keeping to a single shader makes the "state -> uniform" line easier to see.

### Step 6: `Options.setup` (and the decision not to add a `wgpu` feature)

**The assumption in 3.1 was wrong. "eframe stays on default (glow)" does not hold for eframe 0.36.** The `default` feature of eframe 0.36.1 is `["accesskit", "default_fonts", "links", "wayland", "web_screen_reader", "wgpu", "winit/default", "x11"]`, and **`glow` is not in it**. `Renderer::Glow` does not even exist without the `glow` feature, and `Renderer::default()` returns `Wgpu`. In other words, **this repo has been drawing with wgpu from the start**. The opposite of 0.35 and earlier; now glow is the opt-in one.

So **we do not add a `wgpu` feature**. We once added `wgpu = ["eframe/wgpu"]`, but with today's eframe it is a feature that changes nothing and is only API noise. The question in section 5, "make wgpu the only backend?", was in effect decided by eframe first. People who want to run on glow set `eframe/glow` explicitly, and that is not this crate's job. The rationale is kept in a comment in `crates/egui-react-app/Cargo.toml` and in ARCHITECTURE section 8.

**The WebGL fallback is also in without doing anything.** `eframe/wgpu` -> `egui-wgpu/default` -> `wgpu/webgl`. The "turn on the `webgl` feature of `wgpu` for wasm" in 3.1 was not needed. `[workspace.dependencies]` only pins `wgpu = "30.0"` (the version eframe 0.36.1 uses), so that the shader example links against the same wgpu when it builds its pipeline.

**`Options.setup`** was added as in 3.2. The type is `Option<Setup>`, with `pub type Setup = Box<dyn FnOnce(&eframe::CreationContext<'_>)>` (clippy's `type_complexity` rejects the raw type, so it got an alias. It also reads better as an API). `ReactApp::new` does `take()` and calls it at the top. A paint callback may be added on the first frame, so it runs before the store is created. The `options` argument of `ReactApp::new` changed from `&Options` to `&mut Options`, and both the native and wasm startup closures hold `options` by move and `take` it (each closure is only called once).

- One test in `#[cfg(test)] mod tests` in `crates/egui-react-app/src/lib.rs`: the default of `setup` is `None`. kittest cannot run eframe, so confirming that wgpu really draws is left to a visual check (`RUST_LOG=eframe=info`).
- Updated ARCHITECTURE section 7 (the `Options` list and `setup`) and section 8 (backend and WebGL fallback).
