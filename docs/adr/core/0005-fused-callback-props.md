# 0005: `on_*` props are fused into one event enum closure

Date: 2026-08 or earlier · Status: accepted

## Context

Two handlers on the same element that both capture the same state with `&mut` do not compile: two mutable borrows are alive at once. Sibling elements are fine, because direct expansion generates and consumes them in order. Only "several callback props on one element" clashes.

## Decision

`rsx!` fuses every `on_*` attribute on an element into a single `&mut dyn FnMut(E)` closure over a generated event enum, and `match`es inside it. Only that one outer closure captures the state.

## Rejected

- **Return events as the child's return value and `match` after the call.** Simple, but several events in one frame (a `TextEdit` change and a submit) need a `Vec`, and a borrowed payload cannot be returned at all.
- **A hand-written single `callback` prop.** That is what the fused closure is; making users write it every time is verbosity for nothing.

## Consequences

The enum variant is built textually from the element name and the attribute name (`Dialog` + `on_ok` → `DialogEvent::Ok`), so no type information is needed and a misspelt event name is a compile error. In exchange, `CounterEvent` has to be imported wherever `<Counter on_ok=../>` is written. Payloads may borrow. This is the Elm / Yew `Msg` enum, generated and hidden.

## Links

- [ARCHITECTURE section 3.6](../../ARCHITECTURE.md#36-events-callback-props)
