# Task: core (PR2 = Phase 2 + 3 + 4 + 5)

## Goal

Build the API that users actually write on top of the core that spike (PR1) verified. This means the rest of the hooks (`use_memo` / `use_reducer` / `defer` / `update_later` / `use_persisted`), the three macros (`rsx!` / `#[component]` / `#[hook]`), elements and layout, and the runner. When this is done, the examples run on native and wasm, and later PRs (async, mobile) only need to add a runner layer and one hook.

Replace all of spike's hand-written expansions with the macro versions. Spike's tests keep passing as-is, which pins the macro expansions.

## Scope

### In scope

- Phase 2 (core hooks)
  - The `View` trait and the closure View that `rsx!` returns.
  - `use_memo` / `use_reducer` + `Dispatch` / `cx.defer` + `update_later`. The order in which the deferred queue is applied.
  - Give `Cx` a layout context (egui `Ui` or egui_taffy `Tui`), and switch the draw target with `leaf` / `container`.
  - On-screen overlay for Id collision detection (debug builds).
- Phase 3 (macros)
  - `#[component]`: Props struct + builder, `children`, event enum and `Emitter` from `#[event]`.
  - `#[hook]`: `#[track_caller]` and `hook_scope`.
  - `rsx!`: parse with rstml. Elements, embedded expressions, string literals, `if` / `else` / `for` / `match`, `key`, fusing `on_*`, the `events=` escape hatch, extracting common layout attributes.
  - Pin compile error messages with trybuild.
- Phase 4 (elements and layout)
  - `egui-react-elements`: implement `<View>` / `<Text>` on egui_taffy and convert layout attributes to taffy style.
  - Widgets: `Button` / `Label` / `TextEdit` (`bind`) / `Checkbox` / `Slider` / `ComboBox` / `Image` / `Separator`.
  - Containers: `ScrollArea` / `Collapsing` / `Frame` / `Window` / `SidePanel` / `TopBottomPanel` / `CentralPanel`. egui-native `Vertical` / `Horizontal` / `Grid`.
  - kittest interaction tests and layout snapshot tests.
- Phase 5 (runner and examples)
  - `egui-react-app::run(Options, |cx| rsx!{..})`. One function covers both native and wasm. Set `Options::max_passes` explicitly (default 3; see plan.md section 8 for why).
  - `use_persisted` (saved to eframe's `Storage`).
  - examples: `counter`, `todo` (`use_reducer`), `layout`. Delete `examples/spike`.
  - Add wasm `cargo check --workspace` and a trunk build to CI.

### Out of scope

- `use_future` (PR3). This PR only guarantees that `Dispatch` is `Send + 'static`.
- Android / iOS (PR4). English docs, API review, crates.io release (Phase 8).
- Detailed taffy Grid track specs (`minmax`, `auto-fill`, named areas). Stop at `display="grid"` with equal-width columns and `col_span` / `row_span`.
- `Image` loader registration (`egui_extras`). `Image` only takes an `egui::ImageSource`; loaders are the app's job.
- Automated tests for the wasm side of `use_persisted` (localStorage). Test with a mock that stands in for native `Storage`; wasm is checked by eye only.
- Running snapshot tests in CI. The images depend on the renderer of the OS that made them, so put them behind a feature and run them locally only (see "Decisions" below).
- IDE completion or formatter support for `rsx!`.

## Deliverables

- `crates/egui-react`: `view.rs` (`View`), additions to `hooks.rs`, `dispatch.rs`, `layout.rs` (`ItemStyle` / `ContainerStyle` / `Length`), `Cx` extensions, the deferred queue and persistence in `Store`, the collision overlay.
- `crates/egui-react-macros`: `component.rs` / `hook.rs` / `rsx/` (parser, custom nodes, expansion).
- `crates/egui-react/tests/ui/` (trybuild) and new tests in `crates/egui-react/tests/`. Replace spike's tests and `tests/common` with the macro versions.
- `crates/egui-react-elements`: each element plus `tests/` and snapshot images.
- `crates/egui-react-app`: `run` / `Options`, the eframe `App` impl, the wasm runner.
- `examples/counter` / `examples/todo` / `examples/layout` (each with `index.html` and `Trunk.toml`).
- Update `.github/workflows/ci.yml`.
- Update `docs/ARCHITECTURE.md` (changes known at the start are in [plan.md](plan.md) section 7; add changes found during implementation as they come up).
- A usage section in the README (one counter example).

## Done criteria

- Every test table in each phase of [plan.md](plan.md) is green. Spike's tests (`sibling_handlers` / `custom_hook` / `fused_events` / `context_handle` / `multi_pass` / `collision` / `unmount` / `nested_ui` / `repaint` / `effect_deps`) pass with the macro-version components.
- trybuild tests are green and the `.stderr` files are committed.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo check --workspace --target wasm32-unknown-unknown`, and `trunk build` pass in CI. Snapshots are not part of CI (see above).
- `cargo run -p counter` / `cargo run -p todo` / `cargo run -p layout` run on native, and `trunk serve` runs counter in the browser.
- Quit `todo` and restart it; the `use_persisted` content is still there (native, checked by eye).
- ARCHITECTURE.md matches the implementation. List the changes in the PR body.

## Decisions (assumptions at the start)

- The 4 phases go in 1 PR, but split commits by phase (at least 4), and CI must be green at the end of each phase. Do not stack commits that are broken across phases.
- `egui-react` (core) depends on `egui_taffy`. Since `Cx` holds the layout context, core has to know about `Tui`. The wasm `cargo check` must keep passing.
- Use the `typed-builder` crate for the Props builder, re-exported from `egui-react` under `__private` (`#[builder(crate_module_path = ..)]`). Switch to our own generator only if it does not work through the re-export.
- The `#[component]` Props struct is named `<Name>Props`. `rsx!` gets the builder from the function type with `::egui_react::props_builder(&Name)` (the user only needs to `use` `Name`). If inference fails with props that have lifetimes, switch to the fallback in [plan.md](plan.md) section 6.
- Snapshot tests need egui_kittest's `snapshot` + `wgpu` features. Put them behind the `snapshot` cargo feature of `egui-react-elements`. The committed images were made with the macOS renderer and do not match Linux's software renderer, so do not run them in CI; run them locally only. Describe how in the README Testing section.
- egui 0.36's default for `Options::max_passes` is 2. The runner sets 3 explicitly (nested egui_taffy trees need one more pass; plan.md section 8).
- Closures passed to `update_later` / `defer` are `'static` (they need `move`). They go into a queue that lives until the end of the pass. To borrow, use `Dispatch` or clone the value.
- `use_reducer` messages are applied "the next time the hook is visited", not "at the end of the pass" (see plan.md 1.3 for why).
- The Id for `use_persisted` is derived from the string key only, not from the scope. Using the same key in 2 places shares the same state. The storage format is JSON, written to eframe's `Storage` under the single key `"egui_react"`.
- New dependencies: `syn` 2 / `quote` / `proc-macro2` / `typed-builder` / `trybuild` / `serde` / `serde_json` / `wasm-bindgen-futures` / `web-sys`. Pin the latest versions at start time in `[workspace.dependencies]`.
