# 0006: `use_effect` runs the body in place

Date: 2026-08 or earlier · Status: accepted

## Context

React runs effects after commit, so they can measure the committed DOM. egui has no commit, and the `Response` rect is in hand the moment the widget call returns.

## Decision

`use_effect` runs the body immediately at the call site when the deps change.

## Rejected

- **Run after commit, at the end of the pass.** There is no commit to run after, and a deferred body has to be `'static`. That brings back the `Rc` and `.clone()` ceremony this design exists to remove.

## Consequences

The body can borrow `State` guards and locals. Cleanups are stored, so they are `'static`, which is naturally satisfied by what a body creates (task handles, unsubscribers); to change other state on unmount, hold a `Dispatch`. On the second pass of a multi-pass frame the deps match and the body does not re-run.

## Links

- [ARCHITECTURE section 4, `use_effect` details](../../ARCHITECTURE.md#use_effect-details)
