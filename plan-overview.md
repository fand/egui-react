# Development plan

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for design decisions. This document defines how to divide and carry out the work.

## Workflow

- Group multiple phases into each PR, as shown in the table below.
- Follow these steps for each PR:
  1. Write the task definition (objective, scope, completion criteria) in `docs/tasks/<name>/task.md`.
  2. Write the detailed plan (work items, verification, tests) in `docs/tasks/<name>/plan.md`.
  3. Commit the docs.
  4. Delegate implementation to an Opus5 subagent (`/sub` command). The subagent implements according to task.md and plan.md.
- If a design assumption breaks during implementation, update `docs/ARCHITECTURE.md` before changing code. Update plan.md when the plan changes.
- Run CI (fmt / clippy / test / `wasm32-unknown-unknown` check) starting with PR1.
- Write egui_kittest tests starting with PR1 and ensure those tests continue to pass in subsequent phases.

## PR-to-phase mapping

| PR | Phases | Task definition |
|---|---|---|
| PR1 | 0, 1 | `docs/tasks/spike/` |
| PR2 | 2, 3, 4, 5 | `docs/tasks/core/` |
| PR3 | 6 | `docs/tasks/async/` |
| PR4 | 7 | `docs/tasks/mobile/` |
| PR5, PR6 | 6.5 | `docs/tasks/examples/` |
| PR7 | 6.5(canvas) | `docs/tasks/canvas/` |
| PR8 | 6.6 | `docs/tasks/board/`, `docs/tasks/patch/` |

Plan phase 8 (release preparation) separately after PR4. Web accessibility (`docs/tasks/a11y/`) is a candidate for later work; timing is undecided. Layout frame cost (`docs/tasks/perf/`) was done in 2026-09: egui_taffy is gone, replaced by our own layout engine over taffy (ARCHITECTURE section 6), so the phase 0 and phase 4 lines below name a dependency the library no longer has. Only the web / 120-Hz half of that task is left.

## Phases

### Phase 0: Workspace setup

- Create `egui-react` / `egui-react-macros` / `egui-react-elements` / `egui-react-app` and `examples/` in a Cargo workspace.
- Pin egui 0.36, rstml 0.13, and egui_taffy 0.14.
- Set up `rust-toolchain.toml`, CI, LICENSE, and a README skeleton.

### Phase 1: Spike

- Implement minimal `Store` / `Cx` / `State` / `use_state` / `use_effect` / `hook_scope` / sweep functionality in the `egui-react` core.
- Add handwritten expansions of Counter and Dialog (with two callback props) to examples, without macros.
- Capture each validation item from ARCHITECTURE.md section 10 as an egui_kittest test.
- Completion criteria: all validation tests pass. If any item fails, revise the design and update ARCHITECTURE.md.

### Phase 2: Core hooks

- `use_memo` / `use_reducer` + `Dispatch` / `provide_context` + `use_context` / `defer` + `update_later`.
- Implement the repaint policy. Display warnings for detected Id collisions.

### Phase 3: Macros

- `#[component]`: Props structs, children, event enums, `Emitter`.
- `#[hook]`: attach `#[track_caller]` and `hook_scope`.
- `rsx!`: parse with rstml. Elements, embedded expressions, `if` / `for` / `match`, `key`, `on_*` fusion, the `events=` escape hatch, and extraction of common layout attributes.
- Lock down compiler error messages with trybuild. Replace the spike's handwritten expansions with macro versions and verify that the same tests pass.

### Phase 4: Elements and layout

- Implement `<View>` / `<Text>` on egui_taffy, converting layout attributes into taffy styles.
- Button / Label / TextEdit (`bind`) / Checkbox / Slider / ComboBox / Image / Separator.
- ScrollArea / Collapsing / Frame / Window / Panel variants. egui-native Vertical / Horizontal / Grid.
- Capture appearance with kittest snapshots.

### Phase 5: Runner and examples

- `egui-react-app::run`. Set `Options::max_passes = 2`. `use_persisted`.
- Run native builds and wasm builds via trunk in CI.
- Examples: counter, todo (`use_reducer`), layout demo.

### Phase 6: Async

- `use_future` (thread / tokio on native, wasm-bindgen-futures on wasm). `request_repaint` on completion.
- Fetch example.

### Phase 6.5: Examples and gallery

- Split existing examples into lib + bin and publish a gallery (one wasm binary) on GitHub Pages where all examples can be tried in the browser.
- Show each example alongside its implementation in the gallery, with a toggle to compare it with the plain egui version.
- Add examples: form, theme, clock, custom-hook, escape-hatch, list-10k, shell, showcase.
- Add the `wgpu` feature and `Options.setup` to `egui-react-app`, the `<Canvas>` element, and a shader example.

### Phase 6.6: Complex UI examples

Use two examples to demonstrate React benefits that existing examples do not cover: dynamically added, removed, and reordered elements each retain local state; that state follows their keys; and custom components and hooks compose into larger UIs.

Combine them into one PR. Finish board first, then start patch once its custom hooks are settled.

- board: a Trello-style kanban board. Implement DnD, undo, and debounce as custom hooks and compare it side by side with a plain egui version.
- patch: a TouchDesigner-style node editor. Connecting shader nodes generates a single WGSL shader and updates the preview. Reuse board's custom hooks. Do not write a plain egui version.

### Phase 7: Mobile

- Android: build examples with eframe.
- iOS: implement an `egui-winit` + `egui-wgpu` runner in `egui-react-app` and build with cargo-mobile2.
- Adjust touch / IME / safe area handling.

### Phase 8: Release preparation

- Documentation (including English translation), API review, and publication to crates.io.
