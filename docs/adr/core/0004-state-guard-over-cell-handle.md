# 0004: `use_state` returns a guard, with `Handle` as the helper

Date: 2026-08 or earlier · Status: accepted

## Context

State lives in a `RefCell` slot in the store. What the hook hands back decides how user code reads. A Cell-style handle (`get` / `set` / `update`) is safe and copyable but loses ordinary Rust syntax; a `RefMut`-like guard keeps the syntax but can double-borrow.

## Decision

`use_state` returns `State<'s, T>`, a guard implementing `Deref` and `DerefMut` onto the slot. `Handle<'s, T>` is the Cell-style helper, reached with `use_handle` or `into_handle`.

## Rejected

- **A Cell-style `Handle` only.** `*count += 1` and `&mut *name` stop working, and every read needs `T: Clone`. The borrow conflicts that motivate a handle-only API are already removed by the fused event closure ([0005](0005-fused-callback-props.md)), so the cost bought little.

## Consequences

The value lives in the store and is not cached in the handle, so there is no write-back on drop and no `T: Clone`. The guard lives only for the component body. `State::handle(&self)` is deliberately absent — using a `Handle` while the guard is alive is a double borrow of the same `RefCell` — so the only ways to get a `Handle` are `use_handle` (makes no guard) and `into_handle` (releases it), and `provide_context` takes a `Handle` only.

## Links

- [ARCHITECTURE section 3.5](../../ARCHITECTURE.md#35-states-t)
