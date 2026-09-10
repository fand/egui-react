# 0008: One `Handler` trait for both arities, told apart by a marker

Date: 2026-08 or earlier · Status: accepted

## Context

The fused event closure calls a user's handler from inside a `match` arm. That handler may take the payload (`|v| ..`) or ignore it (`|| ..`), and may return a value. `rsx!` has no type information at expansion time, so it cannot pick a call form by looking at the closure.

## Decision

`Handler<A, Marker>` has two blanket impls, one for `FnOnce() -> R` and one for `FnOnce(A) -> R`. They conflict under coherence, so they are separated by the marker type argument `(Arity0, R)` / `(Arity1, R)`. The macro always emits the one form `::egui_reactor::Handler::call(closure, a)`.

## Rejected

- **Pick `call0` or `call1` by counting the closure's arguments in the macro.** The macro would have to know the arity from syntax alone, and it would emit two different forms for the same attribute shape.

## Consequences

The marker is always inferred, so it never appears at a call site. `on_ok={|| ..}`, `on_change={|v| ..}` without an argument annotation, and a body returning a non-`()` value all go through the same emitted path. The same trick is used again for `IntoCleanup` in `use_effect`.

## Links

- [ARCHITECTURE section 3.6](../../ARCHITECTURE.md#36-events-callback-props)
