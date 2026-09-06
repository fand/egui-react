# Task: spike (PR1 = Phase 0 + 1)

## Goal

Confirm, with hand-written code and tests and no macros, that the design in [docs/ARCHITECTURE.md](../../ARCHITECTURE.md) holds up on Rust's borrow rules and egui's execution model. If any assumption breaks here, fix ARCHITECTURE.md before moving on with implementation. At the same time, set up the Cargo workspace and CI that later PRs build on.

## Scope

### In scope

- Cargo workspace (4 crates + examples), `rust-toolchain.toml`, CI, LICENSE, README skeleton.
- Minimal `egui-react` core: `Store`, `Cx`, `State`, `Handle`, `use_state`, `use_handle`, `use_effect`, `hook_scope`, Id collision detection, sweep, repaint policy.
- Minimal `provide_context` / `use_context` (only what the verification items need).
- Counter and Dialog, hand-written as the code the macros should generate (2 callback props, `Handler` trait, `Emitter`).
- Pin down all verification items in ARCHITECTURE.md section 10 as egui_kittest tests.
- One eframe example for visual checks.

### Out of scope

- `rsx!` / `#[component]` / `#[hook]` macros (Phase 3). `egui-react-macros` is only placed as an empty crate.
- `use_memo` / `use_reducer` / `Dispatch` / `defer` / `update_later` / `use_persisted` / `use_future` (Phase 2 and later).
- elements (`View` / `Text` / widget wrappers) and layout attributes (Phase 4). egui_taffy is used only to verify multi-pass.
- On-screen overlay for collision detection (Phase 2). In spike, we only record in the store and `log::warn!`.
- Running on wasm. CI goes as far as `cargo check --target wasm32-unknown-unknown`.

## Deliverables

- `Cargo.toml` (workspace), `crates/egui-react`, `crates/egui-react-macros`, `crates/egui-react-elements`, `crates/egui-react-app`, `examples/spike`.
- `.github/workflows/ci.yml`.
- kittest tests under `crates/egui-react/tests/` (one file per verification item).
- Update to ARCHITECTURE.md (only if an assumption broke).

## Done criteria

- All verification items in ARCHITECTURE.md section 10 are green as kittest tests. See the table in [plan.md](plan.md) for the mapping from item to test.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, `cargo check -p egui-react --target wasm32-unknown-unknown` pass in CI.
- `cargo run -p spike` runs Counter and Dialog.
- Design fixes found during verification are reflected in ARCHITECTURE.md. If there are none, write "no changes" in the PR body.

## Decisions (assumptions at the start)

- License is `MIT OR Apache-2.0` (same as egui). If it changes, say so before starting.
- Rust toolchain is stable. The channel in `rust-toolchain.toml` must be at or above the MSRV of egui_taffy 0.14 (README says 1.95).
- Use `elsa::FrozenMap` for stable slot addresses. You may pick another option if it is awkward, but it must allow inserting another slot while a `State` guard is alive.
