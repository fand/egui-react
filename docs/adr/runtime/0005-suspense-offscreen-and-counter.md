# 0005: `<Suspense>` draws children offscreen and counts pending futures

Date: 2026-08 or earlier · Status: accepted

## Context

React's Suspense works by throwing: a child that is not ready unwinds, the boundary catches it, and nothing the child did is committed. Rust has no cheap rollback, and egui has no commit to discard.

## Decision

Children are always drawn, into an invisible offscreen `Ui` while suspended. `use_future` calls `Store::note_pending()` just before returning `Pending`, which adds one to the nearest boundary's counter; a non-zero count draws `fallback` instead. The switch flips state and calls `request_discard`, so it happens within the same frame.

## Rejected

- **Rollback via panic and `catch_unwind`.** Unwinding is not free, it is off in some builds, and there is nothing to roll back to — the store writes have already happened. The child exits instead with a one-line let-else.

## Consequences

Only three methods are added to core (`begin_suspense`, `end_suspense`, `note_pending`); the boundary itself is an element. The offscreen rect is fixed, so the layout engine sees the same size every pass and asks for no useless discards. `invisible()` stops drawing and interaction, but egui still builds accessibility nodes for invisible widgets, so screen readers and `egui_kittest` see suspended children at offscreen coordinates. `use_effect` in a suspended child runs, unlike React. There is no `SuspenseList` and no `useTransition`.

## Links

- [ARCHITECTURE section 5.8](../../ARCHITECTURE.md#58-suspense)
