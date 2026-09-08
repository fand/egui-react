# 0002: `rsx!` expands directly, with no retained tree

Date: 2026-08 or earlier · Status: accepted

## Context

React diffs a virtual tree because DOM operations are expensive. egui re-emits every widget every frame, so there is no retained side to diff against. The question was whether `rsx!` should still build a node tree and walk it, the way Yew does.

## Decision

`rsx!` expands in place into egui calls. Nothing is retained between frames, and nothing is traversed.

## Rejected

- **A retained VNode tree plus a traversal.** A handler stored in a node outlives the call site, so it has to be `'static` and wrapped in `Rc`, and every captured local has to be cloned. That is exactly the Yew pain the project set out to remove.

## Consequences

Handlers run where they are written and are thrown away. They need neither `'static` nor `Rc`, and can borrow locals with `&mut`. This is the property every later decision is measured against.

## Links

- [ARCHITECTURE section 2.1](../../ARCHITECTURE.md#21-no-reconciler)
