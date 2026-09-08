# 0005: A `<Text>` galley lives for one pass

Date: 2026-09-08 · Status: accepted, supersedes [0004](0004-fingerprint-fonts-to-detect-atlas.md)

## Context

egui rebuilds its `Fonts`, and with it the atlas every galley points into, in three known cases: `Context::set_fonts`, a `TextOptions` change, and an atlas over 80% full. It exposes no atlas identity, so any detection of a rebuild is a guess, and each case egui adds garbles text again. [0004](0004-fingerprint-fonts-to-detect-atlas.md) guessed three times and text garbled three times.

## Decision

The engine keeps a `<Text>`'s galley for the pass that laid it out and no longer: the cache is keyed on `Context::cumulative_pass_nr`, reused within a pass (measure, then paint) and never past it. Across passes the galley comes from epaint's own `GalleyCache`, which lives inside `Fonts` and dies with the atlas. What survives between frames is the `LayoutJob`, which does not depend on the atlas. `Store::note_fonts`, the fingerprint, the fill ratio, `Store::fonts_generation` and the public `fonts_generation(ctx)` are gone.

The gallery's code pane follows the same rule: it caches the highlighted `LayoutJob`s, lays out only the rows in view each frame, and keys its widest-line width on layout metrics (row height, pixels per point, the width of `M`), which are layout properties, not atlas properties.

## Rejected

- **Keep guessing.** The set of cases is egui's to grow, so the work is unbounded and each miss is a garbled screen.
- **Ask upstream for a generation counter on `Fonts`.** The right fix, and still worth a PR, but a release away.
- **A `cx.layout(job)` helper for code outside the engine.** It adds nothing over `ctx.fonts_mut(|f| f.layout_job(job))`, so the rule went into ARCHITECTURE instead of a wrapper.

## Consequences

Per `<Text>` per frame: a `LayoutJob` clone, a hash and a lookup — what `egui::Label` already pays. Gallery bench, accesskit, ms/frame, running column, before → after: list-10k 0.22 → 0.25, board 0.18 → 0.17, patch 0.22 → 0.23. The code pane on the 1661-line `patch` goes 0.08 → 0.11; laying out every line per frame was 0.62 and was rejected. Worst case is +0.1 ms on a 16.7 ms frame.

The rule for escape hatches: never keep an `Arc<Galley>` across frames, keep the `LayoutJob` and lay it out each frame.

Known gap: taffy does not re-measure a clean node, so a font arriving with different metrics changes the galley but not the box it sits in.

## Links

- [ARCHITECTURE section 6, the `<Text>` bullet](../../ARCHITECTURE.md#6-layout)
- `docs/tasks/font/progress.md`
- `docs/tasks/font/plan.md` section 5
- Commits `27854ab`, `f4cb530`
