# Plan: examples

> `egui_taffy` below is historical. It was replaced in 2026-09 by egui-reactor's
> own layout engine over taffy (`crates/egui-reactor/src/engine.rs`, ARCHITECTURE
> section 6), which ports its measure function and node rules, so the layout
> behaviour described here still holds unless ARCHITECTURE says otherwise.

The plan for expanding the examples and for a gallery page where you can try every example in the browser. The design rationale is in [docs/ARCHITECTURE.md](../../ARCHITECTURE.md). This document sets the implementation steps and how to verify them. If a decision during implementation departs from this document, update it, and update ARCHITECTURE.md too if the change matters for the design.

## 0. Overview

Two aims.

1. Like egui.rs, have a summary page (gallery) where you can see live examples in the browser.
2. In the gallery, show each example next to its source code, and show the difference from plain egui. Put "the same UI written in plain egui" next to it, so the difference in state management and layout shows in code and line counts.

Split into 3 PRs. Stack them in order.

| PR | Content | Where it touches |
|---|---|---|
| A: gallery | Split examples into lib / bin, gallery, plain egui versions (counter / todo / layout), snapshot match tests, GitHub Pages | `examples/*`, `examples/gallery`, CI, README |
| B: examples | form / theme / clock / custom-hook / escape-hatch / list-10k / shell / showcase | `examples/*`, registration in the gallery |
| C: canvas | The `wgpu` feature and `Options.setup` on `egui-reactor-app`, the `<Canvas>` element, the shader example | `egui-reactor-app`, `egui-reactor-elements`, `examples/shader`, gallery |

Do not touch core (`egui-reactor`, macros). If it turns out to be needed, write it in section 7.

Dependencies to add (pin them in `[workspace.dependencies]`).

| crate | Purpose | Where |
|---|---|---|
| egui_extras (no features) | Code view in the gallery (`syntax_highlighting::code_view_ui`). Do not use `syntect`; it makes the wasm large. The built-in simple highlighter is enough | examples/gallery |
| eframe `wgpu` feature | Paint callback | egui-reactor-app (behind feature `wgpu`) |
| wgpu (through eframe, plus the `webgl` feature on web) | The shader example's pipeline | examples/shader |
| bytemuck | `Pod` for the uniform | examples/shader |

## 1. PR A: gallery

### 1.1 Splitting examples into lib / bin

Make each example `lib.rs` + a thin `main.rs`. The gallery takes the lib as a dependency, and `cargo run -p counter` keeps working as before.

```
examples/counter/
  Cargo.toml        [lib] + [[bin]] counter (+ [[bin]] counter-plain, 1.3)
  src/lib.rs        pub fn App(cx) and pub const META
  src/main.rs       run(Options{..}, |_cx| rsx!{ <App/> })
  src/plain.rs      plain egui version (1.3)
  index.html / Trunk.toml
```

```rust
pub struct Meta {
    pub name: &'static str,     // "counter"
    pub summary: &'static str,  // one line
    pub hooks: &'static [&'static str],
    pub elements: &'static [&'static str],
    pub source: &'static str,   // include_str!("lib.rs")
    pub plain: Option<&'static str>, // include_str!("plain.rs")
}
```

Rules for examples embedded in the gallery.

- `App` is a component that fills the area it is given. The root is `<View grow={1.0}>`.
- Do not use `Panel` / `CentralPanel` (they cut space out of the parent. ARCHITECTURE section 6).
- `use_persisted` keys are `"<example>/<key>"`. In the gallery, all examples share the same `STORAGE_KEY`, so avoid collisions. Change todo's `"todos"` to `"todo/todos"`.
- Do not use `std::time::Instant` (panics on wasm). Get time from `ui.input(|i| i.time)`.

fetch is already on main (PR3). Split it the same way and put it in the gallery.

### 1.2 gallery (`examples/gallery`)

One wasm. Putting list / run / code in one binary switches faster than a separate page per example, and deploying to Pages needs only one build.

The screen is three flex columns. No `Panel` (the gallery follows the same rule as the examples it embeds).

```
+----------+----------------------+-------------------------+
| list     | running example      | code                    |
| (w=200)  | (grow)               | (w=480, ScrollArea)     |
| filter   | <View key={name}     | [egui-reactor | plain]    |
| by tag   |   grow={1.0}>        | N lines / M lines       |
|          |   {App}              | link to GitHub          |
+----------+----------------------+-------------------------+
```

- Build the list from `Meta`. Clicking a hooks / elements tag filters the list.
- Switching the example through `key={name}` lets the sweep drop the previous example's hooks. That is enough to reset state.
- Code uses `egui_extras::syntax_highlighting::code_view_ui`. Draw it with `cx.leaf_fill`.
- Examples that have a plain egui version show a toggle, and the code switches too. Show both line counts side by side (`source.lines().count()`).
- On wasm, direct links via `location.hash` (`#todo`). On native, the first argument. Add the `Location` feature of `web_sys` to the gallery's wasm dependencies.
- Running the plain egui version keeps state in `use_state(cx, PlainState::default)` and draws it with `cx.leaf_fill(.., |ui| plain::ui(ui, &mut *state))`.

The gallery's `Options.setup` is used in PR C to register the shader pipeline (3.2).

### 1.3 Plain egui versions (`plain.rs`)

Write the same look as the egui-reactor version in egui alone. Leave the places where the difference shows.

| example | Difference visible in the plain egui version |
|---|---|
| counter | Almost the same. Centering with `ui.horizontal` + `ui.centered_and_justified` takes a bit of work, and that is all. Show honestly that "the difference is small in a small example" |
| todo | How state is held (pass every field of a `struct` around as `&mut self`), deletion cannot happen inside the loop so the index is carried over, persistence is written by hand in `eframe::App::save` |
| layout | Write `justify="space-between"` / `grow` / `wrap` / grid with `ui.horizontal` + `allocate_space` + manual math. This one becomes the longest |

The shape is `pub struct PlainState` + `pub fn ui(ui: &mut egui::Ui, state: &mut PlainState)`. So it also runs standalone, add `[[bin]] counter-plain` and call it from a thin `main` that implements `eframe::App`. trunk builds only the egui-reactor version.

### 1.4 Tests

Since examples become libs, kittest can live there. They run under `cargo test --workspace`.

| # | Where | Content |
|---|---|---|
| A-1 | `examples/counter/tests/` | Pressing `+` increases the display by 1. The plain egui version gives the same result for the same action |
| A-2 | `examples/todo/tests/` | Add / toggle / delete / clear done. Same for the plain egui version |
| A-3 | `examples/gallery/tests/snapshots.rs` (feature `snapshot`) | Draw the egui-reactor version and the plain egui version of each example at the same size, and compare them under the **same snapshot name**. If both match one image, that backs the claim "same look, only the code differs" |
| A-4 | `examples/gallery/tests/` | Choosing an example from the list shows its `App`. Tag filtering |

Snapshots do not run in CI, same as the existing ones (add the gallery to the Testing section of the README).

### 1.5 CI and Pages

- `ci.yml`: change the trunk build from the 2 steps for counter / fetch into a loop over `examples/*/Trunk.toml`. Includes the gallery.
- `pages.yml` (new): on push to `main`, run `trunk build --release --public-url /egui-reactor/ --config examples/gallery/Trunk.toml`, then `actions/upload-pages-artifact` + `actions/deploy-pages`. The URL is `https://fand.github.io/egui-react/`.
- In the repository settings, set the Pages source to GitHub Actions (by hand, once).

### 1.6 README

Turn the examples section into a table.

| Column | Content |
|---|---|
| name | `counter` |
| what | The same one line as `Meta.summary` |
| live | Direct link into the gallery (`#counter`) |
| source | `examples/counter/src/lib.rs` |

At the top, a link to the gallery and one screenshot (the gallery with todo open. The snapshot images are too small, so take it by hand).

## 2. PR B: More examples

Things that can be written with the features we have now. In priority order.

| example | What it shows | hooks / elements used | Plain egui version | gallery |
|---|---|---|---|---|
| `form` | A settings form. `bind` on every widget. `on_change` pushes to a change log (`Vec<String>`). Saved with `use_persisted("form/settings")`, reset button | `use_state` `use_persisted`, `TextEdit` `Checkbox` `Slider` `ComboBox` `Collapsing` | Yes | Yes |
| `theme` | `provide_context` hands out dark / light and a language (ja / en). Children nested 3 levels down read it with `use_context`. Switching uses `ctx.set_visuals` | `use_handle` `provide_context` `use_context` | No | Yes |
| `clock` | Clock + stopwatch. `ctx.request_repaint_after(1s)` updates every second, every frame while running. Lap list with `for` + `key`. A child is toggled in and out, and the `use_effect` cleanup shows in the log | `use_state` `use_effect` (cleanup) `use_memo` | No | Yes |
| `custom-hook` | Extract `use_debounce(cx, value, ms)`, `use_previous(cx, value)`, `use_window_size(cx)` with `#[hook]`, and use them from 2 components | `#[hook]` `use_state` `use_effect` | No | Yes |
| `escape-hatch` | Three ways to use plain egui inside rsx. A `view(\|cx\| ..)` closure, unwrapped widgets via `cx.leaf` (`ProgressBar` / `Hyperlink` / `color_edit_button`), drawing lines with `cx.ui().painter()`. Put hooks in a nested `Ui` with `Cx::new` (the shape of the `nested_ui` test) | `view` `Cx::leaf` `Cx::ui` | No | Yes |
| `list-10k` | Show 10k rows in a `ScrollArea` with `for` + `key`. Count Slider, filter TextEdit, FPS (`stable_dt`) display. Show the cost of immediate mode + taffy honestly. The plain egui version is virtualized with `ScrollArea::show_rows`, so the difference shows in numbers | `use_state` `use_memo`, `ScrollArea` | Yes | Yes |
| `shell` | An IDE-like frame. A tree (`Collapsing`) in the left `Panel`, an editor (`TextEdit multiline`) in the center, a log in the bottom `Panel`, an inspector in a floating `Window` | `Panel` `Window` `Collapsing` `Frame` | No | No (standalone bin, because it uses `Panel`. Section 5) |
| `showcase` | A small real app: notes. A list + search on the left, `TextEdit multiline` on the right. Add / delete / update with `use_reducer`, save with `use_persisted`, search results with `use_memo`, settings in a `Window`, theme via `use_context`. A "usable" example that combines everything | Almost all | No | Yes |

Put one kittest file in each example (1 to 3 main actions). Add form / list-10k, which have plain egui versions, to the A-3 snapshot match (take list-10k with the count fixed at 100).

`examples/template` (a starter for a new app) goes to Phase 8 (release prep).

## 3. PR C: canvas (wgpu)

**PR C has been split out to [docs/tasks/canvas/](../canvas/task.md).** What follows stays as the record from the time of the split.

Draw a wgpu shader animation inside a component. Core stays unchanged. Three layers: runner, elements, example.

### 3.1 The `wgpu` feature of `egui-reactor-app`

- eframe stays at default (glow). Add feature `wgpu = ["eframe/wgpu"]` to `egui-reactor-app`.
- When the `wgpu` feature is on, `Options::default()` sets `native.renderer` to `eframe::Renderer::Wgpu`. glow stays (both get compiled. Features unify across the workspace, so with `--workspace` every example runs on wgpu. No harm).
- wasm: turn on the `webgl` feature of `wgpu` for browsers without WebGPU. Check with `trunk build`.
- The note "`wgpu` is left out on purpose" in `Cargo.toml` is about kittest and does not affect headless tests.

### 3.2 `Options.setup`

```rust
pub struct Options {
    ..
    /// Called once right after eframe starts. The place to build the wgpu
    /// pipeline and put it in `render_state.renderer.write().callback_resources`.
    pub setup: Option<Box<dyn FnOnce(&eframe::CreationContext<'_>)>>,
}
```

Call it at the top of `ReactApp::new`. Same shape as the official egui demo (`custom3d_wgpu`). The idea of handing out `RenderState` via `use_context` is not taken: it gains little for the work of creating a slot. No wgpu types appear in hooks or context.

### 3.3 The `<Canvas>` element (`egui-reactor-elements`)

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
- elements does not depend on egui-wgpu. Adding the callback with `ui.painter().add(..)` is the example's job. It also works for drawing lines with `egui::Painter` alone (plots, etc.).
- Tests (kittest, headless): the `rect` passed to `on_paint` has the size given by `w h`. With `grow={1.0}` it takes all remaining space. Dragging fires `on_drag`.

### 3.4 `examples/shader`

```
examples/shader/src/
  lib.rs      App: <Canvas grow> + Slider(speed) + Checkbox(pause) + ComboBox(shader choice)
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

- `ShaderCallback::prepare` writes the uniform with `queue.write_buffer`, and `paint` draws 3 vertices. egui-wgpu sets the viewport / scissor to `rect`.
- The point to show is that `state` flows straight into the uniform. `Slider`'s `bind` -> `speed` -> uniform.
- `ShaderResources` is stored by type in `callback_resources` (`TypeMap`). Even when it lives next to other examples in the gallery, different types do not collide.
- Multiple passes: egui discards the shapes of a dropped pass. The callback never runs twice.
- The gallery turns on `egui-reactor-app/wgpu` and calls `shader::gpu::setup(cc)` in `setup`.
- No plain egui version (the wgpu part would be the same code, so there is no difference).

### 3.5 Tests

| # | Content |
|---|---|
| C-1 | `Canvas`'s `rect` and `on_drag` (3.3, headless) |
| C-2 | shader: moving the Slider changes `speed`, and pause stops `request_repaint` (look at the `harness`'s repaint requests) |
| C-3 | Snapshot (feature `snapshot`, local only): one image with time fixed at 0. Whether the equivalent of `setup` can be fed into the render state of kittest's `WgpuTestRenderer` is in section 5 |

## 4. Steps

1. PR A-1: split the 4 examples (counter / todo / layout / fetch) into lib / bin, add `Meta`. Prefix the `use_persisted` keys. Check `cargo run` and `trunk serve` by eye. Commit.
2. PR A-2: gallery (list / run / code, hash direct links). Must work with 3 examples. Commit.
3. PR A-3: 3 plain egui versions + toggle + line counts. Tests A-1 to A-4. Generate snapshots locally and commit them. Commit.
4. PR A-4: the CI trunk loop, `pages.yml`, README. Set the Pages source (by hand). Check the URL by eye after deploy. PR.
5. PR B: form -> theme -> clock -> custom-hook -> escape-hatch -> list-10k -> shell -> showcase. One commit per example. Register each in the gallery and add tests. Add to the README table. PR.
6. PR C-1: `wgpu` feature + `Options.setup`. Run counter with `--features wgpu` and confirm it draws with wgpu (`RUST_LOG=eframe=info`). Commit.
7. PR C-2: `<Canvas>` + the C-1 test. Commit.
8. PR C-3: the shader example (native -> trunk). Register in the gallery. C-2 / C-3. Update ARCHITECTURE.md. PR.
9. In each PR body, write the changes, the ARCHITECTURE.md changes, and what was dropped. Add A / B / C to the PR table in `plan-overview.md` (before or after PR4 mobile, either is fine).

## 5. Points that may need a decision

- **Snapshot match between the egui-reactor version and the plain egui version (A-3)**. Rounding in taffy and egui may shift by 1px. If it shifts, loosen the threshold (`SnapshotOptions::threshold`), or give up and use a different snapshot name and only "show them side by side". Try layout first.
- **`impl FnOnce` props (3.3)**. If the typed-builder of `#[component]` fails inference on a closure prop, fall back to `&mut dyn FnMut(&mut egui::Ui, egui::Rect)`.
- **Can `Panel` be embedded in the gallery (shell)?** `SidePanel::show_inside` cuts space out of the child `Ui`, so inside the gallery's center column the look might hold. Try it, and if it works put shell in the gallery too.
- **Shader snapshot with kittest (C-3)**. If `WgpuTestRenderer`'s `RenderState` has no way to inject `callback_resources`, drop C-3 and check by eye only.
- **Should wgpu be the only backend?** If the gallery requires wgpu, there is little reason to keep glow. But for now stay on the eframe default and decide in Phase 8.
- **`ScrollArea` in list-10k**. The element has no `show_rows` virtualization. If the gap to the plain egui version is too large, add a `rows={(count, row_h)}` prop to `ScrollArea` (a change in elements. A separate PR is fine).
- **The gallery's wasm size**. All examples + wgpu comes to several MB. `wasm-opt = "z"` is already in. If it is too heavy, split only shader into a separate wasm.
- **fetch running in the gallery**. Pages is https, so `http://` URLs fail as mixed content. The default URL is already https (currently `https://httpbin.org/get`). Note URLs that fail on CORS.

## 6. Changes to reflect in ARCHITECTURE.md

- Section 6, element list: add `Canvas` to containers (`sense` / `on_paint`, `on_drag` / `on_hover`). "Does not depend on egui-wgpu. The caller adds the callback with `painter().add`".
- Section 7, crate layout: the `wgpu` feature of `egui-reactor-app`, `Options.setup`. Examples are lib + bin and the gallery takes the lib as a dependency.
- Section 8, platforms: wgpu backend, WebGL fallback on web. The Pages URL.
- Section 9, tests: kittest in examples. Snapshot match between the egui-reactor version and the plain egui version (the gallery's `snapshot` feature).
- Section 11, decision log: why the gallery is one wasm, why the plain egui version sits next to it, why `Options.setup` was chosen over `use_context`, why `Canvas` does not have egui-wgpu.

## 7. Differences found during implementation

### Step 1 (A-1)

- `Meta` lives in `examples/meta` (package `example-meta`), shared by every example and the gallery. Registered in `[workspace.dependencies]`. Derives `Copy` (every field is `&'static`).
- layout's root was `<ScrollArea grow>`, so it was wrapped in `<View direction="column" grow={1.0}>` (to follow the rule in 1.1).
- `META` was placed right after the `use` lines in each `lib.rs`. `source` is the whole file, so it shows at the top of the gallery's code column. Move it to the end if it gets in the way.
- todo's persistence key is `"todo/todos"`. No migration from the old key `"todos"`.
- The sentence "`examples/counter` verbatim" in the README went stale with the lib / bin split. Fix it in step 4 (README).
- Checking by eye (`cargo run` / `trunk serve`) was not done because the subagent is headless. Do it together with the deploy check in step 4.

### Step 2.5 (bug fixes)

Two bugs outside core, found by looking at the gallery. They were not in the plan, so one commit is added. `crates/egui-reactor` (core) and the macros are unchanged.

**Bug 1: text inside a taffy leaf stacks vertically one character at a time.** The `reset` button of `cargo run -p counter` came out 15x77 (one character wide). The cause is how egui_taffy measures: a leaf remembers only "the `ui.min_size()` from the last draw" (`ui_finite` puts the same value into `min_size` and `max_size`) and reports it to taffy as both min-content and max-content. The first draw happens in a `Ui` of width 0, so a wrapping widget reports one character's width there, and the node gets pinned at that width. Leaves with `grow` or `w` are fine because taffy decides their width, which is why only the gallery's list buttons (`grow={1.0}`) looked right. The fix is in `egui-reactor-elements`: set every leaf that holds text to `TextWrapMode::Extend` (which `Text` already did). `Button` / `Label` use the widget's `wrap_mode` builder; `Checkbox` / `Slider` / `ComboBox` / the `Collapsing` header use `style.wrap_mode` on the leaf's `Ui`. `Label` got a `wrap` attribute so you can go back to egui's default wrapping (same as `Text`). The test is `a_label_in_a_taffy_leaf_stays_on_one_line` in `crates/egui-reactor-elements/tests/widgets.rs`.

**Bug 2: the root container does not fill the window.** counter's `0` and buttons showed at the top left instead of the center (the same `App` embedded in the gallery shows in the center). `reserve_available_space()` only tells egui_taffy the available space and calls `ui.set_min_size`; the root node's own `size` stays `auto`, so taffy sizes that node to its content. `<View grow={1.0} justify="center">` then has no room to grow into and nothing to center against. The fix is in `egui-reactor-app`: put `w("100%")` / `min_h("100%")` into the root `ItemStyle` (only a minimum on the vertical axis, so content taller than the window can extend as is). While there, the root id and style were exposed as `root_id()` / `root_style()` so tests can rebuild the same frame (`crates/egui-reactor-app/tests/root_fill.rs`).

- The gallery's `Chip` stays. `Button` is fixed, but tags want to show a pressed state, elements has no toggle element, and `egui::SelectableLabel` has no `wrap_mode` builder. It serves as an example that hand-written leaves also need to set `style.wrap_mode`.
- A toggle element in elements (`SelectableLabel` / `RadioButton`) is a future candidate.

### Step 2.6 (the gallery's 3 columns overflow)

A continuation of the same cause as bug 2. Choosing layout in the gallery pushed the code column off screen, and layout's `w="100%"` row was 1600px wide.

The cause is the other half of "the root node takes the size of its content". `min_w("100%")` only gives the root a lower bound; `size` stays `auto`, so if the content is wider than the window the root grows to match. No overflow means `flex-shrink` never gets a turn, so the middle column does not shrink even though it has `grow={1.0} min_w={0.0}`, and the row runs straight out of the window. It is not that `min_w` fails to reach taffy (`Length::Px(0.0)` -> `Dimension::length(0.0)`).

The fix is to set the root's `w` to `100%` so the width is fixed (`egui-reactor-app`). The window width is fixed, so a fixed value is right; anything wider belongs in a horizontal `ScrollArea`. Vertical stays `min_h("100%")`. There was no need to add `overflow` to core (`layout.rs`).

- On the gallery side, the code column got `shrink={0.0}`. With only the width fixed, an example with large content like layout squeezes the code column down to its `min_w` of 360. With `shrink={0}` all overflow goes to the middle column (`min_w={0}`), and the column width does not move between examples.
- The gallery tests were switched to `egui_reactor_app::root_id()` / `root_style()` so they use the same frame as the runner. With a hand-built frame the tests would miss exactly this bug.

### Step 3 (A-3)

3 plain egui versions, the gallery toggle, tests A-1 to A-3.

Line counts (`source.lines().count()`, the whole file including META and doc comments).

| example | egui-reactor | plain egui |
|---|---|---|
| counter | 32 | 84 |
| todo | 143 | 143 |
| layout | 144 | 278 |

todo comes out equal because `lib.rs` holds `META` (12 lines) and the reducer definition. The difference in substance (`Msg` + `use_reducer` versus "carry the index over and apply later", `use_persisted` versus `save`/`load`) shows as in 1.3. counter and layout are a plain 2.6x and 1.9x.

**Snapshot results.** All three matched under the same name.

| example | Measured difference | Allowed |
|---|---|---|
| counter | 124 px | 200 |
| todo | 26 px | 100 |
| layout | 750 px | 1000 |

`threshold` stays at egui_kittest's default (0.6). The allowance uses `max_failed_pixels` (a pixel count). Only glyph edges differ: taffy places things in floats and egui rounds to points, so when a shared edge shifts by a fraction of a point the rasterization moves 1px. A real layout break is orders of magnitude larger (the background bug below was 110k px, the wrong `nested` construction was 5001 px), so with this allowance a broken test still fails.

Things fixed on the way here.

1. **The painted background area** (40k to 110k px). In the plain egui harness, call `ui.set_min_size(ui.available_size())` so it takes as much room as the egui-reactor root.
2. **`<TextEdit grow={1.0}>` does not fill its node** (todo, 773 px). taffy widens the node to 321pt, but `egui::TextEdit` draws at its own default `desired_width` (280pt), leaving a 40pt gap inside. Fixed `TextEdit` in `egui-reactor-elements`: when `desired_width` is not set and `cx.in_taffy()`, pass `ui.available_width()`. In Ui mode there is nothing to fill, so egui's default stays. The test is `a_growing_text_edit_fills_its_node` (a `w={400}` node gives 400pt). todo's difference went from 773 to 26 px.
   - `Slider` and `ComboBox` have the same problem (both stay at 100pt in a 400pt node. `spacing.slider_width` / `spacing.combo_width` are the defaults). `Button` does not stretch either (28pt). Not fixed this time, only recorded. `Slider` has no width builder, so it would mean touching `ui.spacing_mut().slider_width`; `ComboBox` has `.width()`.
3. **The plain egui `nested` came out as one row**. `ui.allocate_ui` inherits the parent's horizontal layout, so use `allocate_ui_with_layout(.., Layout::top_down(..))` per column and claim the width with `ui.set_min_width` (otherwise the allocation shrinks to the content width).
4. **Made the `grow` math match flexbox**. At first the whole width was split 1:2, but `grow` distributes the *remainder*. Measure each column's content width and add the rest at 1:2. That brought the x of `right top` to 218.0 versus 217.9.
5. **Vertical spacing in the justify section**. `gap={4}` + `mb={4}` on each row means "8 between rows, 4 after the last row". egui also adds item_spacing around `add_space`, so set `item_spacing.y = 0` and write 8 and 4 explicitly. Then the y of every section matched exactly.

The 2 images in `crates/egui-reactor-elements/tests/snapshots/` were retaken. `row.png` had not been retaken after the wrap fix in step 2.5, and the **still-buggy picture** with `right` stacked one character per line was committed (snapshots sit behind a feature, so they did not run in that step). `widgets.png` changed by the amount the field widened from the `TextEdit` fix above. **Tests behind a feature must be run by hand on every change that touches what that feature covers.**

Generate snapshots by taking the egui-reactor side first (`UPDATE_SNAPSHOTS=1 cargo test -p gallery --features snapshot egui_reactor`). Updating both same-named tests at once makes them fight over the same file.

An empty list says nothing in the todo picture, so before taking it, drive both through the same steps (add `milk` / `eggs` and mark one done). `done (1)` is in the default collapsed state on both.

**Other decisions.**

- The gallery's plain egui version uses `use_state(cx, PlainState::default)` + `cx.leaf_fill(.., |ui| plain::ui(ui, state.bind()))`. With `&mut *state` it goes dirty every frame and keeps requesting repaints, and kittest's `run()` fails on `max_steps`. `bind()` is right here for the same reason it is right for bind widgets.
- The toggle sits in the code column, with both line counts under it (`"32 lines"` and `"84 lines plain"`). Choosing an example again goes back to the egui-reactor version.
- `PlainState` is a different type per example, so branch with `match` like `Running` (one macro generates all 3).
- todo's persistence became `save()` / `load()` that build JSON with `serde_json`, and `plain_main.rs` touches eframe's `Storage`. This keeps `plain.rs` to egui + serde only.
- The last section of the plain egui layout (`Grid` / `Vertical`) comes out about the same length on both sides. That is expected because both sides use egui's own containers, and it is shown honestly too.

### Step 4 (A-4)

CI, Pages, README.

- `ci.yml`: the 2 steps for counter / fetch became one loop, `for config in examples/*/Trunk.toml`. The glob matches 5 (counter / fetch / gallery / layout / todo). `examples/meta` has no Trunk.toml, so it is not included. Actions' `run:` uses `bash -e` by default, so a failure inside the loop stops right there (checked locally). The comment on why `--config` is passed and the fetch comment were gathered above the loop. The gallery was added to the snapshot comment at the end.
- `pages.yml` (new): on push to `main` and `workflow_dispatch`. The build job runs `trunk build --release --public-url /egui-reactor/ --config examples/gallery/Trunk.toml`, hands `examples/gallery/dist` to `upload-pages-artifact@v3`, and the deploy job runs `deploy-pages@v4`. `pages: write` / `id-token: write` are set only on the deploy job; the top level is `contents: read`. `concurrency: pages` uses `cancel-in-progress: false` (what gets published should be a commit whose build ran to the end).
  - `dist = "dist"` is relative to Trunk.toml, so the output at `examples/gallery/dist` is correct. Checked locally.
  - `--public-url` only rewrites the `<link href>` in the generated `index.html` to `/egui-reactor/gallery-….js`. The `#todo` direct link reads `location.hash` inside the wasm, so it has nothing to do with `--public-url`. Both checked locally.
  - The apt install of dependencies is the same as in ci.yml. A wasm-only build should not need it, but the deploy is not the place to find out.
  - **One manual step remains**: in the repository's Settings -> Pages -> Source, choose "GitHub Actions". This is also written in the comment at the top of pages.yml.
- `README.md`: fixed "`examples/counter` verbatim" (the homework from step 1). Stated clearly that the snippet combines the component from lib.rs and the `run(..)` from main.rs, and copied the content from the current files. Turned the Examples section into a table (name / what / live / source / plain egui) linking to the gallery, and listed how to run: `cargo run -p <name>`, `--bin <name>-plain`, `trunk serve`, `cargo run -p gallery <name>`. Added `cargo test -p gallery --features snapshot` to the Testing section, with the step of taking the egui-reactor side first for same-name comparisons. Line counts are not in the README (as in step 3, todo is equal, which misleads without explanation).
- The screenshot is not inserted yet. A `<!-- TODO: gallery screenshot -->` marker is in place.

## 8. PR B record

### Step 5-1: form

A settings form. `Settings` (name / notify / autosave / volume / theme) lives in `use_persisted(cx, "form/settings", ..)`, and `TextEdit`, `Checkbox` x2, `Slider`, `ComboBox` are all wired with `bind`. `on_change` pushes a log into a `use_state` `Vec<String>`, shown in a `Collapsing` (last 8 lines, newest first). The reset button restores the defaults. A summary line (`"anon, dark, volume 50"`) is shown so tests can read it.

- **`on_change` cannot read the new value**. A `bind` element's widget holds the `&mut` of the state, so a handler on the same element cannot touch the same state (the rule in section 6). So the log can only hold what the widget passes as payload: `Checkbox` gives the new `bool`, `ComboBox` the new index, `TextEdit` and `Slider` give `()`, so they can only say "name edited" and "volume changed". This is a constraint and also an inconvenience, but it is where the meaning of `bind` shows plainly, so it is shown as is and noted in a comment.
- **The width of `Slider` / `ComboBox` was not fixed**. The "stays at 100pt even in a grow node" from step 3 remains. But in a settings form it is normal for a widget to sit next to its label at a natural width, and a ComboBox stretched across the row would look odd. So form does not use `grow`; it gives the label column a width (90pt) to line things up. When an example that wants stretching comes along (list-10k or so), fix it then the same way as `TextEdit`.
- **The snapshot matches exactly** (diff 0 px, allowance also 0). Unlike counter / todo / layout, not a single pixel differs. On the egui-reactor side it is the line where `<Field>` gives the label `w={90}`; on the plain egui side it is `egui::Grid::new(..).min_col_width(90)`. Both say "make a label column" in one line.
  - At first it was off by 503 px. The plain egui side fixed the height with `ui.add_sized([200, interact_size.y], TextEdit)`; switching to `TextEdit::singleline(..).desired_width(200.0)` and letting egui choose the height brought it to 0.
- Line counts are **egui-reactor 158 / plain egui 123, so egui-reactor is longer**. Two reasons, both worth showing honestly. (a) `Settings`, `THEMES`, and `META` live in `lib.rs`, and `plain.rs` gets them with `use crate::Settings`. `lib.rs` carries the weight of the shared types. (b) egui's `Grid` aligns the label column for you, so the egui-reactor side, which writes a `<Field>` component, has more to do. **Forms are an area where egui is already strong, and egui-reactor does not win here.** The differences are in how state is held (one struct passed around as `&mut`), the need to "collect the log before drawing the rows", and writing persistence by hand. Those are the same kind of difference as todo in 1.3.
- In the gallery list it sits after counter / todo (before form / layout / fetch).

### Step 5-2: theme

`provide_context` / `use_context`. `Themed` holds `Theme { dark }` and `Locale` in `use_handle` and hands them to children; `use_context` reads them 3 levels down in `Page` -> `Card` -> `Greeting` / `ThemedButton` / `Swatch`. `Page` and `Card` in between take no props at all. `Toggles`, right under the provider, calls `set` on the `Handle` it read with `use_context` to write back (the same shape as React's `useTheme()` returning a value and a setter). `Orphan`, placed outside `Themed`, gets `None` from `use_context` and shows "outside: no theme provided".

- **`<Provide value={handle}>` cannot be written**. This is the main finding this time. The `Handle<'s, T>` that `provide_context` takes borrows the store, but `props_builder` requires the component to be `for<'a, 's, 'u> Fn(&'a mut Cx<'s, 'u>, P)`, so the props type cannot name `'s`. Writing it fails with `implementation of Fn is not general enough` (checked by putting it in elements, then removed). For the same reason `view(|cx| provide_context(cx, handle, ..))` does not compile either (`View::show` is also higher-ranked over `'s` and cannot connect to the outer `Handle`).
  - The form that works is "a provider component that creates the value itself and hands it out itself". If the handle is created from the inner `cx`, `'s` matches, so `#[component(shares_ui)] fn Themed(cx, children: impl View)` compiles as is. In React too, the provider usually owns the state, so in practice there is no loss. Written in ARCHITECTURE section 6. It could be fixed by touching core, but not this time.
- **The languages are en / fr, not ja / en**. egui's bundled fonts are Hack / Ubuntu-Light / NotoEmoji / emoji-icon-font, with no CJK glyphs (checked the contents of `epaint_default_fonts`). Japanese would render as tofu. Loading fonts is a job for a different example, so this uses two Latin-script languages. A deliberate departure from plan section 2.
- **`ctx.set_visuals` repaints the whole gallery**. `set_visuals` is per `egui::Context`, and there is only one Context. It does not break the embedding rules in plan 1.1 (it is neither `Panel` nor `Instant`), but opening theme in the gallery and switching to light makes the gallery light too. That is just how the egui API works, so it is accepted and noted in a comment rather than hidden.
- Only one snapshot, since there is no plain egui version. A `single!` macro was added next to `same!` (taken in the default dark state).
- In the gallery list it sits after form, before layout.

### Step 5-3: clock

Clock + stopwatch + `use_effect` cleanup.

- All time comes from `ui.input(|i| i.time)` (seconds since app start, f64). No `std::time::Instant`.
- **The wall clock is `web_time::SystemTime`**. `std::time::SystemTime::now()` panics on wasm32-unknown-unknown. Added `web-time = "1.1.0"` to `[workspace.dependencies]`. No time zone is possible (it needs a time zone database), so it is shown clearly as UTC. The `HH:MM:SS` formatting is hand-written; a date library is too big for one line.
- **Repaint is explicit**. `ctx.request_repaint()` while running, `ctx.request_repaint_after(1s)` while stopped. kittest's `run()` loops until no "repaint request without delay" remains, so `request_repaint_after` lets `run()` stop (its delay is not 0) but `request_repaint()` does not. Tests of the running state use `step()`.
- **Cleanup and `Dispatch`**. The `show ticker` checkbox mounts and unmounts `Ticker`, and the closure returned by `Ticker`'s `use_effect(cx, (), || { .. ; move || .. })` is the cleanup. The cleanup is stored, so it is `'static` and cannot borrow the log state. So the log lives in `use_reducer`, and `Dispatch<String>` is passed as a prop (`Dispatch` is `Clone + Send + 'static`, so it can be a prop. `Handle` cannot, as in 5-2). The unmount message is sent inside the sweep and applied the next time the reducer is visited, so "ticker unmounted" shows one frame late.
- `use_memo` is used for the formatted lap strings (deps is `laps.len()`). Formatting every frame at 60fps is waste; rebuilding only when the count changes is enough, so it does not feel forced.
- **How the snapshot was made stable**. kittest has `harness.input_mut()`, and setting `RawInput::time = Some(x)` fixes egui's `i.time` (`let time = new.time.unwrap_or(self.time + predicted_dt)`). But that only fixes egui's clock; **it has no effect on the `SystemTime` wall clock**. So `App` takes a `now: Option<u64>` prop (seconds since UTC midnight, `None` means the real clock), and the snapshot and tests pass a fixed value. The stopwatch is stopped at `00:00.00`, so it is stable with no work, and fixing `i.time` turned out to be unnecessary.
  - `single!` cannot pass props, so only the clock snapshot is written by hand without the macro.
- Tests: note that in kittest, the label of a `Role::Label` goes into `Node::value()`, not `Node::label()` (accesskit's spec). Hit this in the helper that reads the stopwatch display.
- In the gallery list it sits after theme, before layout.

### Step 5-4: custom-hook

Three hooks written with `#[hook]`, each called from 2 components. Package name `custom-hook` / lib name `custom_hook`.

| hook | Content | Callers |
|---|---|---|
| `use_debounce(cx, &str, f64) -> String` | 3 `use_state` (latest value / time of change / settled value). Time is `i.time`. While waiting, nobody asks for the next frame, so the hook itself calls `request_repaint_after(remaining)` | `SearchBox` / `Mirror` |
| `use_previous<T>(cx, T) -> Option<T>` | 3 lines of `use_state((current, previous))` | `SearchBox` (the settled query one step back) / `Counter` |
| `use_window_size(cx) -> Vec2` | `cx.ctx().viewport_rect().size()`. A hook with no state | `Responsive` (switches row / column by width) / `SizeReadout` |

- **The effect of `#[hook]` is the example itself**. `SearchBox` and `Mirror` call the same `use_debounce`, but typing into one does not move the other's display. `#[hook]` uses `Location::caller()` to open a scope per call site, and a test pins that down.
- `egui::Context` has no `screen_rect()`. Use `viewport_rect()`.
- **Embedded in the gallery, `use_window_size` returns the size of the gallery window** (not the center column). That is correct for what the hook means (it asks for the window size), but when embedded it falls to the `layout: row` side. Run standalone, you can see it switch when you narrow the window.
- Whether to put `#[hook]` on the stateless `use_window_size` was a question, but it got one. A hook is "a reusable function that reads from `Cx`", and having state is not the point. If state is added later, callers do not change.
- The snapshot name follows the lib name, `custom_hook.png` (because `single!` uses `stringify!`). The example name is `custom-hook`.
- **Tests can stop time for `use_debounce`**. `harness.input_mut().time = Some(t)` survives into the next frame because `RawInput::take()` keeps `time`. Fixed at 0.1 seconds `settled` does not move; at 5.0 seconds it catches up.
- In the gallery list it sits after clock, before layout.

### Step 5-5: escape-hatch

Four exits to plain egui, one per section. Package `escape-hatch` / lib `escape_hatch`.

1. `{view(|cx| ..)}`: ordinary code placed in the middle of rsx. Hooks work too (slots are keyed by line).
2. `cx.leaf(&style, |ui| ..)`: widgets that have no element (`egui::ProgressBar`, `ui.color_edit_button_srgba`) placed as a taffy item. `ItemStyle::default().w(..)` applies as is.
3. painter: `allocate_exact_size` + `ui.painter()` draws a sparkline. The values are `use_state(Vec<f32>)`, deterministic via `sin(i * 0.7)`.
4. Nested `Cx`: inside `ui.group(..)`, build `Cx::new(store, ui, scope)` and use hooks inside `cx.scope("inner", ..)`. `Cx::new` / `cx.store` / `cx.scope_id()` / `cx.scope` are all public API and reachable from the prelude. **Section 4 can be written**.

Hit 3 things during implementation. Each is worth more than the example itself.

- **`cx.ui()` inside a `<View>` is not "where you are now"**. In taffy mode, `cx.ui()` is the `Ui` that started the taffy tree, so drawing there lands outside the layout at the top left of the tree (the first draft of section 1 did exactly that and overlapped the heading). The doc of `Cx::ui` already says so. Reading (`visuals()`, `input()`) is safe anywhere; for drawing, use `cx.leaf`. **This is the very reason `leaf` exists**, so section 1 was rewritten in that form, and the module doc names it as a trap.
- **`leaf_fill` takes the whole window on any axis you did not size**. egui_taffy reports the max-content of an `infinite` leaf as the size of the root rect. A ProgressBar leaf given only `w` grew to the full window height, the sections below were pushed down by a window's height, and the test's click landed outside the viewport (egui ignores pointers out of range). Fixed by giving both `w` and `h`. The same point as "give a `ScrollArea` inside a `<View>` either `grow` or `h`" in ARCHITECTURE section 6 applies to `leaf_fill` in general.
- **`ui.spinner()` does not get along with tests**. It animates, so it requests a repaint every frame, and `Harness::run()` fails on `max_steps`. Removed from section 1 with a comment giving the reason (using it as the `Suspense` fallback is different; there it only runs while waiting).
- `egui::ProgressBar` exposes nothing to accesskit (both label and value of `ProgressIndicator` are `None`). To make it readable, `<Text>{format!("progress {:.2}", ..)}</Text>` sits next to it, and the test looks at that.
- The snapshot uses a form of `single!` that takes a drive function (the 2-argument form expands to the 3-argument form), taken after pressing "add sample" twice. With a single point the sparkline is not a line.
- In the gallery list it sits after custom-hook, before layout.

### Step 5-6: list-10k

Show the price of a long list honestly. Package `list-10k` / lib `list_10k`, with a plain egui version.

**Measured numbers** (`cargo test --release -p list-10k --test bench -- --ignored --nocapture`. `Harness::step` for 20 frames, 600x800, no GPU, so "CPU side of one frame". M4 Max).

| Rows | egui-reactor | plain egui (`show_rows`) |
|---|---|---|
| 100 | 0.84 ms | 0.18 ms |
| 1,000 | 5.03 ms | 0.14 ms |
| 10,000 | 86.82 ms | 0.17 ms |

egui-reactor draws every row. The `for` in `rsx!` is a real loop; one row is a `<View>` + 3 children, so 10k rows means 40k taffy nodes. The plain egui version uses `ScrollArea::show_rows` to draw only the roughly 15 visible rows and reserves height for the rest, so the frame time does not move when the row count grows 100x. **Plain egui wins this example.** The numbers are not in the README (they are here and in the example's module doc).

- **No virtualization prop was added to `<ScrollArea>`**. It was a candidate in plan section 5, but `<ScrollArea>` takes its children as an opaque `impl View` closure, so the body of the `for` loop cannot be pulled out. To make `rows={(count, row_height)}` mean anything, a separate element is needed that takes "a closure that receives an index and returns a View" as a prop. -> **Step 5-8 added that element (`<VirtualList>`).** The "plain egui wins" conclusion of this section is updated there.
- **The default row count**. `DEFAULT_COUNT = 10_000` (as the name says). But the gallery passes `initial_count={1_000}`. At 10k one frame is 85ms and the whole gallery drops to 12fps, which reads as "egui-reactor is slow". The slider reaches 10k, so anyone who wants to can push it. The plain egui version is also set to 1,000 in the gallery (it is virtualized so 10k is fine, but if the row count changed with the toggle there would be no comparison).
- **Snapshots use different names** (`list_10k_react.png` / `list_10k_plain.png`). Under the same name they differ by 9,373 px. The content is the same list, but one draws every row and the other draws the visible 12 to 14 rows and reserves the rest, so a shift of about 3px inside a row repeats for every row. Closing the gap would mean writing the plain egui version to match taffy's math, which crosses the line in section 5. Unlike form / counter / todo / layout, the structure differs here.
- **kittest: a button inside a `ScrollArea` cannot be pressed with `click()`**. The simulated pointer press is absorbed by the scroll area and does not reach the widget. `click_accesskit()` works. Hit this in the row deletion test.
- The bench is an `#[ignore]` test (`tests/bench.rs`). It only means anything in release, and it is not an assertion.
- In the gallery list it sits after escape-hatch, before layout.

### Step 5-7: shell (and a fix to the list-10k index column)

**The list-10k fix.** The index column of the plain egui version took the width of its content, so the start of the name did not line up with the egui-reactor version. `INDEX_W` (64pt) is exported from the lib and both use it. The plain egui side uses `allocate_ui_with_layout` + `set_min_width` (`add_sized` centers, and without a minimum width it shrinks to the content). Snapshots retaken. Still under different names.

**shell.** An IDE-like frame. Top / left / bottom `<Panel>`, the editor in `<CentralPanel>`, the inspector in a floating `<Window>`, the tree on the left is `<Collapsing>` + `selectable_label`.

**Bug: `<Panel>` did not dock under the runner. Fixed in elements.**

- Cause. `Panel` is `shares_ui`, but its body is `cx.leaf(&style, ..)`, so in taffy mode one node is created and the panel cuts space out of that. The runner always opens `root_container`, so 4 panels cut out 4 small nodes and all of them drew stacked at the same top left (measured: `save` / `files` / `log` all near (16,10)). The "panels are meant for the app root" of ARCHITECTURE section 6 did not hold under the runner.
- Fix (`egui-reactor-elements`). **A panel cuts space out of "the nearest egui `Ui`", which is the `Ui` that started the current taffy tree.** In taffy mode it calls `show_inside` on `cx.ui()` instead of `cx.leaf`. Outside a tree (Ui mode) it works as before. `CentralPanel` is the same. Under the runner this `Ui` is the window, so "use at the root" holds automatically.
- As a result, a `<Panel>` written deep inside a `<View>` is not part of that row; it jumps to the window edge. That is what docking means, so the doc comment states "this is not a bug". The test is `a_panel_inside_a_view_docks_in_the_window` (a left panel inside `<View grow>` reaches the window's left edge and does not overlap the rest). The existing `panels_written_as_siblings_dock` still passes as is.
- **It cannot go in the gallery** (this rule does not change that). Even placed in the gallery's center column, what the panel cuts from is the `Ui` that started the gallery's tree, which is the whole window. Tried at 1280x800: the gallery's own labels were painted over by `CentralPanel` and vanished. The "try it, and if it works put it in the gallery" of plan section 5 **did not work out**. It stays standalone.

**Two holes in elements plugged on the way** (shell needs both).

- `default_pos` and `default_size` on `<Window>` (the same-named methods of `egui::Window`, first frame only). Without them egui's default position (top left) covers the toolbar and the tree. The inspector starts open and sits toward the bottom right.
- `rows` on `<TextEdit>` (-> `desired_rows`). Also, `multiline` inside taffy fills its node on both axes with `ui.add_sized(ui.available_size(), ..)`. At first it was `desired_rows = available_height / row_height`, but that only matches in whole rows and the remainder spilled out of the node, so it became `add_sized`. Tests are `a_growing_multiline_text_edit_fills_its_node` and `an_explicit_row_count_wins`.

**Hit "an auto node is measured by its content" once more**. The `<View grow={1.0}>` holding the editor takes the size of its content with `grow` alone, and the `TextEdit` inside is measured by its own content, so the two agreed on "two characters wide" and got stuck there. Fixed by giving fixed values, `<View w="100%" h="100%">`. Same point as the runner root (step 2.6), for the third time. **`grow` distributes the remainder; it is not a substitute for a fixed size.**

- Note that `h="100%"` is 100% of the tree's root rect, slightly larger than the content height of `CentralPanel`. The editor overflows by a few pt but egui clips it, so it looks fine. The test allows for that and checks "log is below the top edge of the editor".
- The snapshot lives in `examples/shell/tests/snapshots.rs` under shell's own `snapshot` feature, not in the gallery. Making the gallery depend on shell for the picture of an example it does not show is poor form, and it bloats the wasm. `cargo test -p shell --features snapshot`.
- `<Window open={..}>` gets `inspector.bind()`. With `&mut *inspector` it goes dirty every frame and repaints never stop (same reason as `Checkbox`'s `bind`).
- **More on kittest**. Not only inside a `ScrollArea` (5-6): **widgets inside a nested taffy tree in general** do not receive simulated pointer clicks. Here it was the toolbar buttons inside a `<View>` inside a panel. In the same panel, tree items drawn directly on a plain `Ui` via `cx.leaf` can be pressed with `click()`. **Rule: inside a `<View>`, use `click_accesskit()`**.

### Step 5-8: `<VirtualList>` and the third list-10k

It is true that `ScrollArea` + `for` draws every row, but ending with "so plain egui wins" is not right. **egui-reactor can virtualize too. It just cannot do it through `<ScrollArea>`.** An element was added, and list-10k was rebuilt around that comparison.

**`crates/egui-reactor-elements/src/virtual_list.rs`**

```rust
#[component]
pub fn VirtualList(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    rows: usize,
    row_h: f32,
    render: impl for<'a, 's, 'u> FnMut(&'a mut Cx<'s, 'u>, usize),
)
```

The body is exactly section 4 of escape-hatch: inside `cx.leaf_fill`, call `egui::ScrollArea::show_rows`, rebuild `Cx::new(store, ui, scope)` on the returned `Ui`, and run `cx.scope(i, |cx| render(cx, i))` only for the visible rows. Each row enters its own scope, so rows can hold hooks, same as `for` + `key={i}`.

- **Closure props can be written. But state the bound explicitly** (the answer to the worry in 3.3). `render: impl FnMut(&mut Cx, usize)` does not compile. `#[component]`'s `ElideToPropLifetime` rewrites the prop's elided lifetimes to the props struct's, so an undeclared lifetime appears inside `impl Trait` and you get `use of undeclared lifetime name`. Write `impl for<'a, 's, 'u> FnMut(&'a mut Cx<'s, 'u>, usize)` yourself, and there is nothing to rewrite, so it compiles as is. No need to fall back to `&mut dyn FnMut`. This is a different problem from the `Handle` prop in 5-2; there the cause was the props type naming `'s`.
- The constraint is "all rows have the same height". That is the condition under which `show_rows` can produce a range without measuring, and the element cannot check it, so the doc states it.
- It is `leaf_fill`, so give it `grow` or `h` (the lesson from step 5-5).
- Tests (`crates/egui-reactor-elements/tests/virtual_list.rs`): 10,000 rows in a 300pt harness put only about 15 rows in the tree, and `row 9999` does not exist. Scrolling removes the first row and brings in later rows.

**The third list-10k.** A `Checkbox "virtualise"` switches between `<ScrollArea>` + `for` and `<VirtualList>`. A row is one `<Row>` component, and both paths draw the same thing. The default is off, so what the gallery shows first is still "the price of drawing everything".

| Rows | `<ScrollArea>` + `for` | `<VirtualList>` | plain egui `show_rows` |
|---|---|---|---|
| 100 | 0.88 ms | 0.36 ms | 0.16 ms |
| 1,000 | 5.06 ms | 0.28 ms | 0.13 ms |
| 10,000 | 78.04 ms | 0.27 ms | 0.17 ms |

`<VirtualList>` is flat in the row count. The gap to plain egui (0.27 versus 0.17) is the price of the taffy nodes for the roughly 15 rows on screen, which is the price of using egui-reactor itself, so it is shown as is. **The conclusion changed from "plain egui wins" to "writing 10k rows with `for` is expensive. There is a dedicated element for long lists".**

- Tests: switching does not change filter and delete behavior, and both paths show the same thing in the first 10 rows.
- Added `VirtualList` to the element table in ARCHITECTURE section 6, noting that `ScrollArea` draws everything and when to use which. The list-10k line in the README was replaced too.

### Step 5-9: showcase (last in PR B)

A notes app. It combines what the other examples showed one at a time, in the shape of an app. The model and reducer are in `src/notes.rs` (flat functions that know nothing about egui, so they can be read without `Ui`), the UI in `src/lib.rs`.

- Persistence is the same shape as todo: "the reducer owns it, `use_persisted` mirrors it". A reducer cannot reduce into someone else's slot, so the two-line mirror is the price. The reason is in a comment.
- The `use_memo` deps are `(search, (len, next_id), XOR of updated)`. `next_id` means "something was added", `len` means "something was deleted", the XOR of `updated` means "a body was edited (so the order moved)".
- `provide_context` is the same `Themed` wrapper as theme. The two columns in between carry nothing around.
- "clear all" in the settings window is a two-step confirm. One `if` in the middle of rsx is enough.
- After `Msg::Add`, `*selected = None` so the new note opens itself (the "open the first one when nothing is selected" rule picks it up).

**Things hit.**

- **An `Option<T>` prop is "an optional prop", not "a prop that takes an Option"**. `#[component]` adds `strip_option`, so the setter takes `T`, and `selected={current}` (`Option<u64>`) does not type check. With `&Option<u64>` it is a reference type, so it is not stripped and passes as is.
- **Anything placed after a filling widget goes off screen**. `<TextEdit multiline grow>` reports "as much height as there is" as its content, so the word-count row placed after it in the same column was pushed below the window. `min_h={0}` does not fix it (the column is what does the pushing). Fixed by moving the word-count row **before** the editor. "Put a filling leaf last in its column" is the practical rule.
- The disk filled up partway through (`ld: write() failed, errno=28`); deleted `target/debug/incremental` (6.9GB) and continued. target grew to 27GB in this session.

**Reordering the gallery.** `showcase` moved to the top (what a visitor should see first). Then counter / todo / form / theme / clock / custom-hook / escape-hatch / list-10k / layout / fetch. The default arm of the `match` in `Running` became `<ShowcaseApp/>`, and the `"counter"` arm is explicit. The gallery tests assumed "counter first", so they were fixed (the toggle test picks counter first, since showcase has no plain egui version). The README table uses the same order, and the intro sentence says "look at showcase first; the others are one topic each".

## 9. PR C record

### Step 6: `Options.setup` (and the decision not to add a `wgpu` feature)

**The assumption in 3.1 was wrong. "eframe stays at default (glow)" does not hold for eframe 0.36.** The `default` feature of eframe 0.36.1 is `["accesskit", "default_fonts", "links", "wayland", "web_screen_reader", "wgpu", "winit/default", "x11"]`, and **`glow` is not in it**. `Renderer::Glow` does not even exist without the `glow` feature, and `Renderer::default()` returns `Wgpu`. So **this repository has been drawing with wgpu from the start**. The reverse of 0.35 and earlier; now glow is the opt-in.

So **no `wgpu` feature is added**. `wgpu = ["eframe/wgpu"]` was added once, but with today's eframe it is a feature that changes nothing and only adds noise to the API. The "should wgpu be the only backend" question of section 5 was settled by eframe first. Anyone who wants glow can set `eframe/glow` explicitly, and that is not this crate's job. The reasoning is kept in a comment in `crates/egui-reactor-app/Cargo.toml` and in ARCHITECTURE section 8.

**The WebGL fallback is also in without doing anything.** `eframe/wgpu` -> `egui-wgpu/default` -> `wgpu/webgl`. The "turn on the `webgl` feature of `wgpu` on wasm" in 3.1 was unnecessary. `[workspace.dependencies]` only pins `wgpu = "30.0"` (the version eframe 0.36.1 uses), so the shader example links to the same wgpu when it builds its pipeline.

**`Options.setup`** was added as in 3.2. The type is `Option<Setup>`, `pub type Setup = Box<dyn FnOnce(&eframe::CreationContext<'_>)>` (clippy's `type_complexity` rejects the raw type, so it got an alias. It also reads better as an API). `ReactApp::new` calls `take()` on it at the top. A paint callback may be added in the first frame, so it runs before the store is created. The `options` argument of `ReactApp::new` changed from `&Options` to `&mut Options`, and both the native and wasm startup closures move `options` in and `take` it (each closure is called only once).

- One test in `#[cfg(test)] mod tests` in `crates/egui-reactor-app/src/lib.rs`: the default of `setup` is `None`. kittest cannot run eframe, so confirming that it really draws with wgpu is left to a check by eye (`RUST_LOG=eframe=info`).
- Updated ARCHITECTURE section 7 (the `Options` list and `setup`) and section 8 (backend and WebGL fallback).
