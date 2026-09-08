# 0002: The store lives in the runner's `App`, not in egui memory

Date: 2026-08 or earlier · Status: accepted

## Context

`Store` is a slab of hook slots plus an Id map. egui already has a place for per-context data, `Context::data()`, and putting the store there would make it reachable from anywhere with a `Context`.

## Decision

The runner's `App` struct owns the `Store` and hands `&mut Store` into the frame.

## Rejected

- **`Context::data()`.** Everything would go through a `TypeMap` behind egui's lock, and a test would have to build a whole eframe context to get at a store.

## Consequences

A test can build a `Store` directly and drive passes over it, which is how the spike and the engine tests work. It also gives the sweep its safety: `end_pass` takes `&mut Store`, so the type system forbids running it while a `State` guard or a `Handle` that borrows the store is alive.

## Links

- [ARCHITECTURE section 5.1](../../ARCHITECTURE.md#51-store)
- [ARCHITECTURE section 5.2](../../ARCHITECTURE.md#52-sweep)
