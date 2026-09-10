# Task: examples (PR5 + PR6 + PR7 = Phase 6.5)

## Goal

Turn the examples from "four working samples" into "a place that shows what the library is good at". Like egui.rs, have a gallery page where you can try every example in the browser, and show each example next to its source code. Put a version of the same UI written in plain egui next to it, so the difference in state management and layout shows in code and line counts. Add one example for each feature area that is missing (forms, context, effect cleanup, custom hooks, the exit to plain egui, many elements, panel layout, a real app). Finally, add an example that draws a wgpu shader inside a component, and open the minimum entry points needed for it (the runner's `wgpu` feature, `Options.setup`, the `<Canvas>` element).

Do not touch core (`egui-react`, `egui-react-macros`).

## Scope

Split into 3 PRs. Details in [plan.md](plan.md).

### In scope

- PR5 (gallery)
  - Split the 4 existing examples (counter / todo / layout / fetch) into lib + bin, and give each a `Meta` (name, summary, hooks / elements tags, source).
  - `examples/gallery`: one wasm. Three columns: list (filter by tag) / running example / code view. Toggle between the egui-react version and the plain egui version, with line counts. Direct links via `location.hash`.
  - Plain egui versions (`plain.rs`): counter / todo / layout.
  - Tests: kittest for each example, snapshot match between the egui-react version and the plain egui version (the gallery's `snapshot` feature).
  - Make the CI trunk build a loop over all examples. A workflow that deploys the gallery to GitHub Pages.
  - Turn the examples section of the README into a table, and link to the gallery.
- PR6 (examples)
  - form / theme / clock / custom-hook / escape-hatch / list-10k / shell / showcase. Register each in the gallery with a kittest (shell is a standalone bin because it uses `Panel`).
  - Also write plain egui versions of form / list-10k.
- PR7 (canvas)
  - Feature `wgpu` on `egui-react-app` (eframe's wgpu backend). When on, `Renderer::Wgpu` becomes the default.
  - `Options.setup: Option<Box<dyn FnOnce(&CreationContext)>>`. The place to put the pipeline into `callback_resources`.
  - `<Canvas>` element (`egui-react-elements`): a leaf that gets a rect from taffy and calls `on_paint(ui, rect)`. `on_drag` / `on_hover`. Does not depend on egui-wgpu.
  - `examples/shader`: fullscreen triangle + fragment shader. State (speed / pause / drag) flows into the uniform. Runs on native and with trunk. Registered in the gallery.

### Out of scope

- Changes to core. If one turns out to be needed, write it in plan.md section 8 and handle it in a separate PR.
- `examples/template` (a starter), `cargo generate`. Phase 8.
- Virtualization of `ScrollArea` (`show_rows`). Listed in plan.md section 5 as a candidate to add if the gap in list-10k is too large.
- A plain egui version of the shader example (there would be no difference).
- Design tuning of the gallery (fonts, colors). Only as far as it works and is readable.
- English docs, API review, crates.io release (Phase 8). Android / iOS (PR4).

## Deliverables

- `examples/*/src/lib.rs` + `main.rs` (+ `plain.rs`), `examples/gallery/`.
- 9 new examples (`form` `theme` `clock` `custom-hook` `escape-hatch` `list-10k` `shell` `showcase` `shader`).
- `crates/egui-react-app`: feature `wgpu`, `Options.setup`.
- `crates/egui-react-elements/src/canvas.rs` (`Canvas`).
- `.github/workflows/ci.yml` (trunk loop), `.github/workflows/pages.yml`.
- The examples table in the README. Updates to `docs/ARCHITECTURE.md` (plan.md section 6). The PR table in `plan-overview.md`.

## Done criteria

- The tests in plan.md (A-1 to A-4, each example, C-1 to C-3) are green. All existing tests still pass as they are.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo check --workspace --target wasm32-unknown-unknown`, and `trunk build` for every example and the gallery pass in CI.
- The gallery opens at `https://fand.github.io/egui-react/`, every example (except shell) runs, the code is readable, and you can switch to the plain egui version (checked by eye).
- `cargo run -p <example>` works for every example. `cargo run -p shader` animates the shader and the Slider changes the speed (checked by eye). `trunk serve` shows the same in the browser (checked by eye).
- ARCHITECTURE.md matches the implementation. List the changes in each PR body.

## Decisions (assumptions at the start)

- The gallery is one wasm. No separate page per example (faster switching, simpler deploy).
- An example shown in the gallery is "a component that fills the area it is given". It does not use `Panel` / `CentralPanel`. `use_persisted` keys are `"<example>/<key>"`. It does not use `std::time::Instant`.
- The plain egui version aims for the same look as the egui-react version, and is compared under the same snapshot name. If a 1px rounding difference appears, absorb it with a threshold; if that fails, use a different name (plan.md section 5).
- The code view uses `egui_extras::syntax_highlighting` (without `syntect`).
- wgpu comes through eframe's `wgpu` feature. glow stays. Build the pipeline in `Options.setup`; do not expose wgpu types in hooks / context. `Canvas` does not know about egui-wgpu.
- Enable the WebGL fallback for wgpu on web.
