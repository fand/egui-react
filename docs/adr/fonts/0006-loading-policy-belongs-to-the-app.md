# 0006: Loading policy belongs to the app; the library exposes `pending()`

Date: 2026-09-08 · Status: accepted

## Context

A `Url` source arrives after the first frame, so text drawn before it lands is drawn in whatever font stands behind it in the chain. CSS calls that choice `font-display`, and the browser implements `swap`, `block` and the rest for you. egui has no such setting, and no notion of text that is laid out but invisible.

## Decision

`Fonts::pending()` returns whether any `Url` is still in flight. The policy is the app's: `examples/font` shows both, `swap` (draw with what stands behind the pending URL, today's default) and `block` (draw a placeholder while `pending()`).

## Rejected

- **A library-level `font-display` setting.** epaint has no "laid out but invisible" state to implement `block` with, so the library would have to invent a placeholder — and a placeholder is a design decision, which belongs to the app.

## Consequences

An app that cares writes the branch itself, and does not have to walk the per-entry report to find out whether to.

## Links

- [ARCHITECTURE section 8, fonts](../../ARCHITECTURE.md#8-platforms)
- `docs/tasks/font/progress.md`
- Commits `8dedaba`, `98eec89`
