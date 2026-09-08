# 0007: Hook deps are compared by `Hash`

Date: 2026-08 or earlier · Status: accepted

## Context

`use_effect`, `use_memo` and `use_future` re-run when their deps change, so the deps have to be compared against the last pass's.

## Decision

Deps are compared by their `Hash`. The hash is stored on the slot; a different hash means a change.

## Rejected

- **`PartialEq + Clone`.** Keeping the old value to compare against requires owning it, which forbids borrowed deps such as `(&str, &[T])`.

## Consequences

Deps can borrow. Hash collisions are possible and are accepted as being as negligible as egui's own Ids, which are hashes too.

## Links

- [ARCHITECTURE section 4, `use_effect` details](../../ARCHITECTURE.md#use_effect-details)
- [ARCHITECTURE section 4, `use_memo` details](../../ARCHITECTURE.md#use_memo-details)
