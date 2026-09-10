# 0004: The web AccessKit adapter is a module of `egui-reactor-app`

Date: 2026-09-09 · Status: accepted

## Context

The DOM mirror of the AccessKit tree was its own crate, `crates/accesskit-web`, marked `publish = false`: it was a port of an unreleased AccessKit prototype, meant to go upstream. The upstream plan was withdrawn on 2026-09-05 (`docs/tasks/a11y/task.md`). `egui-reactor-app` depends on it on wasm32, and Cargo refuses to publish a crate whose dependency is not on crates.io. So the runner could not be published as it was.

## Decision

The adapter moves into the runner as `egui_reactor_app::accesskit_web`, a `#[cfg(target_arch = "wasm32")]` module. The code, its attribution header and its browser tests (`tests/a11y_mirror.rs`) move unchanged. The `accesskit` and `accesskit_consumer` dependencies become wasm32-only dependencies of `egui-reactor-app`.

## Rejected

- **Publish it as a crate of its own** (`egui-reactor-accesskit-web` or similar). Nothing else uses it, and a crate named after AccessKit that AccessKit does not maintain would sit on crates.io next to a future real `accesskit_web`. One more crate to version and release for no reader.
- **Keep the runner unpublished.** The runner is the crate every app starts from.

## Consequences

- Four crates are published, not five. `cargo publish --workspace` orders them.
- The module is public, so a hand-written runner can still use the adapter without `WebA11y`.
- If AccessKit ships a web adapter, the module is replaced by that dependency and `WebA11y` keeps its shape.

## Links

- `crates/egui-reactor-app/src/accesskit_web/`
- `docs/tasks/a11y/plan.md`, the port and what changed against upstream
