# 0009: A value prop and a handler over the same state stay the user's problem

Date: 2026-08 or earlier · Status: accepted

## Context

`<Dialog title={&*title} on_rename={|s| *title = s} />` is E0502: the props struct holds a shared borrow of the state and the fused closure holds a mutable one. The fused closure ([0005](0005-fused-callback-props.md)) removes the handler-versus-handler clash but not this one, because the two borrows are of different kinds.

## Decision

The user resolves it: clone the value (`title={title.clone()}`) or route the write through `update_later`. `rsx!` inserts nothing.

## Rejected

- **Have `rsx!` clone implicitly.** The macro has no type information, so it cannot tell what is cloneable or where a clone is needed. And zero-copy props should be the default; silently cloning makes cost invisible.

## Consequences

Props stay borrowed. The error is a compile error, not a runtime panic, and its fix is one of two known edits. Where it comes up often, the answer is a component that passes read and write through one `&mut`, like `bind` on `TextEdit`.

## Links

- [ARCHITECTURE section 3.7](../../ARCHITECTURE.md#37-remaining-borrow-constraints)
