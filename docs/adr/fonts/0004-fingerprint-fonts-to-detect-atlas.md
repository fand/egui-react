# 0004: Detect a new atlas by fingerprinting the fonts

Date: 2026-09-07 · Status: superseded by [0005](0005-galley-lives-one-pass.md)

## Context

A galley holds texture coordinates into the atlas of the epaint `Fonts` that laid it out. egui throws that `Fonts` away and builds a new one, with a new atlas, after `Context::set_fonts`. The layout engine cached a `<Text>`'s galley across frames, keyed by wrap width and pixels per point, so on the first web run every label that had not changed its text came out as fragments of other glyphs the moment the web font arrived. This hit any app calling `set_fonts` after the first frame, not only the fonts module.

## Decision

`Store::note_fonts` fingerprints the definitions once per pass from the root `Cx` and publishes a generation; the engine's galley cache key carries it. Later extended to compare the whole `TextOptions` and to watch the atlas fill ratio, after the `theme` and `showcase` examples garbled on a dark / light switch — `Visuals::dark` and `Visuals::light` carry different `TextOptions`, epaint builds a new atlas for that, and the definitions were equal so the fingerprint saw nothing.

## Rejected

- **Do not cache galleys at all.** Rejected on a performance worry that was never measured, which is what [0005](0005-galley-lives-one-pass.md) went back and measured.

## Consequences

Each case egui rebuilds its `Fonts` for needs its own detection, and there were three by the end. The generation was also published as a public `egui_react::fonts_generation(ctx)`, because the gallery's code pane kept galleys across frames too and had the same bug.

## Links

- [ARCHITECTURE section 6, the `<Text>` bullet](../../ARCHITECTURE.md#6-layout)
- `docs/tasks/font/progress.md`
- `docs/tasks/font/plan.md` section 5
- Commits `7b24925`, `f8d078d`, `120b375`
