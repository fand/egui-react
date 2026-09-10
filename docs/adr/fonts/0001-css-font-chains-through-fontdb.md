# 0001: CSS-style font chains resolved through fontdb

Date: 2026-09-07 · Status: accepted

## Context

epaint rasterizes text itself from bytes handed to `Context::set_fonts`; the browser's fonts and the OS's font matching are never involved, and egui's four bundled fonts have no CJK glyphs. So an app that shows Japanese has to find font bytes and register them, on native and on wasm both. Two crates do CSS font matching in Rust: `fontdb` and `font-kit`.

## Decision

`egui_react_app::fonts`: `Fonts` holds named `FontStack`s, a stack is a list of `FontSource::{Bundled, System, Url, Generic}`, and every source only puts bytes into one `fontdb::Database`. A chain is a fontdb query; the resolver turns the result into egui's `FontDefinitions`, whose per-family lists are already egui's per-glyph fallback. Built as one PR, not three.

## Rejected

- **font-kit.** `SystemSource` is `cfg`'d out on wasm32 and `freetype-sys` is a hard dependency there, so the crate does not build for the web target and the wasm path would have needed a second implementation. On Linux it links `libfreetype` and `libfontconfig`, so CI and every Linux user building an egui-react app need the `-dev` packages. Last release 2025-05. Nothing was lost by dropping it: fontdb's `find_best_match` is a port of font-kit's own CSS Fonts Level 3 algorithm, and its source cites it.

## Consequences

No C libraries on any target, so CI needs no new packages. Family names are matched byte for byte against the face's name-table families, so `"Hiragino Sans"` and `"ヒラギノ角ゴシック"` both work and `"hiragino sans"` does not; the report shows the family a face declared, which is how a user finds the spelling. Generic names keep fontdb's defaults on macOS and Windows ("Arial" and friends), which are not CJK-aware, so apps name the CJK fonts before the generic, as CSS is written in practice. `set_fonts` runs on every arrival, with no batching of a chain's URLs until someone needs it.

## Links

- [ARCHITECTURE section 8, fonts](../../ARCHITECTURE.md#8-platforms)
- `docs/tasks/font/plan.md` (1.2, 2.1, 2.2)
- Commit `844cfe6`
