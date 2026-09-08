# 0001: A multi-pass frame needs no state rollback

Date: 2026-08 or earlier · Status: accepted

## Context

The layout engine calls `request_discard` when the frame on screen is now wrong, and egui runs a second pass within the same frame. A handler that ran on the first pass has already written to the store. If the second pass re-delivered the same events, every click would count twice.

## Decision

Nothing is rolled back. The second pass simply runs again against the state the first pass left.

## Rejected

- **Snapshot the store before a pass and roll back before the second one.** It costs a clone of every slot per frame and needs `T: Clone`, and it would undo writes that are correct.

## Consequences

This rests on an egui property, confirmed in its source: `Context::run` passes `new_input.take()` per pass and `RawInput::take` moves the events out, so the second pass runs with empty events and handlers fire only once. An effect placed before a handler whose deps derive from what that handler wrote does run on the second pass; that is next frame's run pulled forward, one deps change to one run. `max_passes` is 3, and when the budget runs out the runner calls `request_repaint` to settle on the next frame.

## Links

- [ARCHITECTURE section 5.3](../../ARCHITECTURE.md#53-multi-pass)
