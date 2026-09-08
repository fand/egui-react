# 0004: `use_future` returns `std::task::Poll<T>`

Date: 2026-08 or earlier · Status: accepted

## Context

An async hook has to say "not yet" or "here it is", and most frameworks invent a three-state enum (`Loading` / `Ready` / `Error`) for it.

## Decision

`use_future` returns `&Poll<T>`, the standard library's own type.

## Rejected

- **A custom `Loading` / `Ready` / `Error` enum.** A third state that only some futures use. Errors fit `T = Result<..>` without one, and a bespoke type adds a name every user has to learn.

## Consequences

`Poll` is in std, so it needs no import ceremony and no conversion at the edges. A child under `<Suspense>` is written `let Poll::Ready(x) = use_future(..) else { return };`, which is also how it exits ([0005](0005-suspense-offscreen-and-counter.md)).

## Links

- [ARCHITECTURE section 4](../../ARCHITECTURE.md#4-hooks-list)
