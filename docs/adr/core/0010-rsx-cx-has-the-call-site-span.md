# 0010: `rsx!` spells `cx` with the span of the call, not of the element

Date: 2026-09-08 · Status: accepted

## Context

`rsx!` expands to `view(|cx| { .. })` and draws every element with `enter_scope(cx, ..)`. The element's expansion was `quote_spanned!` with the element's own span, `cx` included, so the identifier carried the hygiene of wherever the element was *written*. That is the same place as the `rsx!` call in ordinary code, and nobody noticed. It is not the same place when a `macro_rules!` whose body is an `rsx!` takes elements as arguments (`item!(<Leaf/>)`, which is how `examples/styles` writes each row once for both its code column and its picture): the closure parameter has the macro's hygiene, the element's `cx` has the caller's, and the caller's `cx` is the enclosing closure's, or the component's own argument. Two closures then need the same `&mut Cx` and the borrow checker refuses.

## Decision

The `cx` an element is drawn with is a fresh identifier with `Span::call_site()`, the span of the `rsx!` invocation, which is what the closure parameter is already spelled with. Everything else keeps the element's span, so errors still point at the element.

## Rejected

- **`Span::mixed_site()` for the whole expansion.** It would also hide `cx` from user code that legitimately names it, and the rest of the expansion (`props_builder(&Path)`, setter names) wants the element's span for its error messages.
- **Telling users not to wrap `rsx!` in `macro_rules!`.** The pattern is the natural way to write a thing once and show it twice, and the fix is one identifier.

## Consequences

- An element handed through a `macro_rules!` resolves `cx` to the closure it sits in. `crates/egui-reactor/tests/rsx_children.rs` keeps it that way.
- `cx` inside an `rsx!` still names the closure's parameter; a user's own `cx` is shadowed as before.

## Links

- [ARCHITECTURE section 3.2](../../ARCHITECTURE.md#32-view-and-rsx)
- `examples/styles/src/lib.rs`, the `row!` macro
