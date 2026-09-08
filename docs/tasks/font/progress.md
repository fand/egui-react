# font: progress and handoff

PR: https://github.com/fand/egui-react/pull/14, branch `claude/text-rendering-fonts-umshi6`, base `main`.
Task in `task.md`, research and design in `plan.md` (section 5 carries what the web runs found), the example in `examples/font`.

## State (2026-09-08)

Everything in the task's scope is built, tested and pushed; the PR is open and up to date with `main`. Review round one (2026-09-08): `Fonts::pending()`, the `font-display` toggle, the typical-stack marker and matrix in the example, and an engine fix for galleys garbling on a dark / light switch (see the table). Review round two (2026-09-08): the engine no longer keeps a galley across passes at all, so the detections were removed; the rule for escape hatches (keep a `LayoutJob`, not an `Arc<Galley>`) is in ARCHITECTURE. Left as known: taffy does not re-measure a clean node, so a font arriving with other metrics changes the galley but not the box it sits in. CI and the Cloudflare Pages preview are green on the head. What is left is a look by a person on macOS (the `system` stack resolving to Hiragino Sans) and in a desktop Chrome with Japanese fonts installed ("Use my fonts" turning the `system` stack's `missing` entries into `loaded`); the sandbox could prove the mechanism, not those two machines.

## What was done, in order

| Step | Commit | Change | Notes |
|---|---|---|---|
| Plan | `fe80651`, `5887c2f` | `task.md` / `plan.md`. First written for font-kit in three PRs; rewritten the same day as one PR on fontdb | font-kit does not build for wasm32 and links FreeType and fontconfig on Linux; fontdb is pure Rust, builds everywhere, and its matching is a port of font-kit's CSS algorithm |
| Asset | `b2c3087` | `examples/font/fonts/NotoSansJP-Subset.ttf`, 433 KB, OFL: ASCII, Latin-1, kana, CJK punctuation, fullwidth forms, 505 kanji. Regeneration commands in `fonts/README.md` | Made with fonttools from google/fonts' variable Noto Sans JP, instanced at wght 400 |
| 1 | `844cfe6` | `egui_react_app::fonts`: `Fonts` / `FontStack` / `FontSource::{Bundled, System, Url, Generic}`, `parse_css`, resolver over one `fontdb::Database` into `FontDefinitions`, `report()` | Every source only puts bytes into the database; chains are fontdb queries; egui defaults stay at the tail of every chain |
| 2 | `3bbc057` | `<Text font="...">` in elements: a registered `FontFamily::Name`, `"proportional"` or `"monospace"`; unknown names fall back and warn once | One `BTreeMap` lookup under the fonts lock per `<Text>`; never reaches egui's panic |
| 3 | `d759bbe` | Faces with no outline glyphs, or a `CBDT` / `sbix` table, are `Invalid` | epaint 0.36 draws outlines only; a bitmap emoji font would claim the glyphs and draw nothing. One filter in `resolve::check`, to go when the renderer changes |
| 4 | `ca0a7ca` | `Url` source over `ehttp`; WOFF / WOFF2 via `wuff` behind the `woff2` feature | Pending until the bytes land, then re-apply; one fetch per URL |
| 5 | `ca2a07a` | Local Font Access for wasm: `request_local_fonts`, `local_fonts_available`, `local_fonts_permission`; native stubs | Reached through `js_sys::Reflect`, since web-sys 0.3.104 keeps `FontData` behind an unstable cfg |
| 6 | `ec53a2b`, `c0eb557` | `examples/font`: stacks `bundled` / `web` / `system`, report table, "Use my fonts"; registered in the gallery; trunk `pre_build` hook downloads the web font | The web font is the static Regular OTF from noto-cjk: the variable font from google/fonts draws as its Thin default instance |
| 7 | `9ac5df5`, `14a93ca` | ARCHITECTURE 7 / 8, README row | |
| Fix | `7b24925` | **Engine**: `<Text>` galleys re-laid out after `set_fonts`. `Store::note_fonts` fingerprints the definitions once per pass, the engine's galley cache key carries the generation. **Resolver**: same-named files (the subset and the full font both say `NotoSansJP-Regular`) get `~2`, `~3` keys instead of sharing one | Found on the first web run: every unchanged label drew glyph fragments once the web font arrived. The engine bug affected any app calling `set_fonts` after the first frame |
| | `ad4fcd5` | `code` stack (`monospace` generic, then the subset) as `default_monospace` | The gallery's source pane draws in `Monospace`; the sample strings were boxes |
| | `85b8ce5` | Merge of `main` (Cloudflare preview workflow, code pane rewrite, shader) | `pull_request` workflows come from the PR branch, so the branch had no preview until it carried the workflow file |
| | `a65c067` | Under the stack buttons: how the selected stack gets its bytes on this target | |
| Fix | `f8d078d` | **Local Font Access** is `window.queryLocalFonts()`, not `navigator.fonts.query()` (the 2020 draft the plan had carried). **Gallery** code pane galleys keyed by `egui_react::fonts_generation(ctx)`, new public fn | Found on the second web run: the button was disabled in Chrome; the source pane garbled after the web font arrived, the same stale-galley bug as the engine's, in the gallery's own per-line cache |
| | `df20c15` | Local Font Access note wraps, result on its own line | |
| | `8dedaba` | `Fonts::pending()`: any `Url` still in flight | So an app can draw a loading state without walking the report |
| Fix | `120b375` | **Engine**: galleys laid out again after a change of visuals or a full atlas, not only after `set_fonts`. `Store::note_fonts` compares the `TextOptions` whole and watches the atlas fill ratio (monotonic while one atlas lives); `end_pass` reads the ratio once more | Found in the `theme` and `showcase` examples: their dark / light switch goes through `set_visuals`, `Visuals::dark` and `Visuals::light` carry different `TextOptions`, and epaint's `Fonts::begin_pass` builds a new atlas for that. The definitions were equal, so the fingerprint saw nothing |
| | `98eec89` | `font-display` toggle in the example: `swap` (today's FOUT) or `block` (a placeholder while `pending()`) | egui has no "laid out but invisible" text, so `block` is a placeholder; the policy is the app's, the library only exposes `pending()` |
| | `45ab262` | The example starts on the stack an app uses on this target (`system` natively, `web` on wasm), marks it `★`, and draws a native / web matrix of where each stack gets its bytes | `how()` only spoke about one target; the choice is a pair |
| Fix | `27854ab` | **Engine**: a `<Text>` galley is kept for the pass that laid it out and no longer (keyed on `Context::cumulative_pass_nr`); across passes it comes from epaint's `GalleyCache`, which lives inside `Fonts` and is rebuilt with the atlas. `Store::note_fonts`, the fingerprint, the fill ratio, `Store::fonts_generation` and the public `fonts_generation(ctx)` are gone | The three detections above were guesses at something egui does not expose; every case epaint adds would garble text again. Holding only the `LayoutJob` across frames removes the class of bug. Cost per `<Text>` per frame: a job clone, a hash and a lookup, what `egui::Label` pays; the gallery bench's running column did not move (list-10k 0.20 → 0.23 ms, board 0.18 → 0.18, patch 0.21 → 0.21) |
| | `f4cb530` | **Gallery** code pane: caches the highlighted `LayoutJob`s, lays out only the rows in view each frame through epaint's cache, and keeps the widest-line width keyed on layout metrics (row height, pixels per point, width of `M`), not on the atlas | Laying every line out per frame cost 0.62 ms on the 1661-line `patch`; per visible row it is 0.10 ms against a 0.08 baseline. The width key is a metric key: advances move only with definitions, size or ppp, and a miss costs a few pixels of scroll range, never a garbled glyph |

## Verified

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test` per package (23 packages), `cargo check --workspace --target wasm32-unknown-unknown`: green locally and in CI.
- Tests added: resolver units (chain order, tail, defaults, dedup, same-name files, invalid bytes, bitmap-only faces, pending / failed URLs, generic fallback), `fonts.rs` kittest, `fonts_url.rs` (a local HTTP server), `text_font.rs` (elements), `fonts_change.rs` (engine: a galley is laid out again after `set_fonts`, kept otherwise), the example's kittest (Japanese in `bundled`, `code` and `Monospace`).
- Headless Chromium 141 in the sandbox, against a trunk release build of the gallery served on localhost, with the `local-fonts` permission granted: the web font arrives and no text garbles (main pane and source pane); the `web` stack shows the full font under its own key; "Use my fonts" is enabled and `queryLocalFonts()` succeeds (0 of the named families on that Linux machine, which is the right answer there). The trunk `pre_build` hook and `copy-file` with `data-target-path` work as written.

## Not verified here

- macOS: `System("Hiragino Sans")` resolving through `load_system_fonts` (the `.ttc` face index path).
- A desktop Chrome with Japanese fonts: the grant filling the `system` stack. The mechanism is proven; the fonts were not on the machine.
- Safari / Firefox: only that they take the "Chromium only" branch by construction.

## Decisions recorded on the way

- fontdb over font-kit; one PR; emoji decided in code, not up front (`plan.md` section 5).
- Generic names on macOS / Windows stay fontdb's defaults ("Arial" and friends); apps name the CJK fonts before the generic, as CSS is written.
- `set_fonts` on every arrival, no batching of a chain's URLs until someone needs it.
- Text renderer swap is a separate task; the seam is `TextNode::galley` plus the paint of `TextShape` in `engine/mod.rs`. Candidates and the Canvas2D web-only option are in the conversation record, not yet in a task file.
