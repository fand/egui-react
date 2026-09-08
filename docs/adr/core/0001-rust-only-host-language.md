# 0001: Rust only as the host language

Date: 2026-08 or earlier · Status: accepted

## Context

The goal is React's way of writing UI — JSX, function components, hooks — for egui apps. One way to get it is to run real React: embed a JS engine, render to a canvas, and bridge to egui. The other is to build the same shapes in Rust, as macros and traits.

## Decision

User code is Rust only. `rsx!`, `#[component]` and the hooks are Rust; no JS runtime is bundled and no JS/TS React runs.

## Rejected

- **JS React plus a JS runtime.** Several times the binary size. iOS forbids JIT, so the engine would run interpreted there. And every call across the line needs a wasm-bindgen layer, which is the ceremony the project exists to remove.

## Consequences

Faithful React semantics are out of scope with it: no VDOM, no reconciler, no `memo()`, no `useCallback`. What is kept is the way code reads, not the way React works.

## Links

- [ARCHITECTURE section 1](../../ARCHITECTURE.md#1-goals-and-non-goals)
