# 0003: A bitmap-only face is rejected as `Invalid`

Date: 2026-09-07 · Status: accepted

## Context

epaint 0.36 draws outlines only. `FontFace::new` keeps skrifa's `charmap()` and `outline_glyphs()` and nothing else, `allocate_glyph_uncached` returns `None` when the outline is missing, and `has_glyph` asks the charmap alone. So a bitmap-only emoji face — Apple Color Emoji is `sbix`, Noto Color Emoji is `CBDT`, both with an empty or absent `glyf` — claims every emoji in the chain and then draws nothing for it, and the chain never falls through to a font that could.

## Decision

The resolver rejects a face with no outline table, or with a `CBDT` or `sbix` table, as `Invalid`. COLR fonts pass: their base glyphs are outlines and draw in one colour.

## Rejected

- **Register them and let the chain sort it out.** It cannot. Claiming a glyph and drawing nothing is invisible to epaint's fallback, which asks the charmap.

## Consequences

Colour emoji cannot be added by registering a font, and the report says why rather than leaving blank glyphs on screen. It is one filter in `resolve::check`, to be removed when the text renderer changes.

## Links

- [ARCHITECTURE section 8, fonts](../../ARCHITECTURE.md#8-platforms)
- `docs/tasks/font/plan.md` section 5
- Commit `d759bbe`
