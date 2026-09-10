# 0007: egui's embedded fonts sit behind a `default_fonts` feature

Date: 2026-09-08 · Status: accepted

## Context

egui's `default_fonts` feature embeds 1.4 MB of font bytes. An app that ships its own subset does not want them, especially on wasm. Cargo unifies features across the whole graph, so as long as any workspace crate takes `egui` or `eframe` with default features, the app cannot drop them however it declares its own dependencies.

## Decision

`egui` and `eframe` are taken without default features workspace-wide; eframe's other defaults are re-added by name, and `winit` becomes a direct dependency so `winit/default` can be named (a dependency's feature list cannot name a transitive crate's feature). `egui-reactor-app` re-exposes the switch as its own `default_fonts` feature, on by default. The resolver takes its base `FontDefinitions` as input and pushes a built-in key only when the bytes exist.

## Rejected

- **Leave egui's defaults on.** Then the 1.4 MB is unremovable, whatever the app does.

## Consequences

A chain that resolved to nothing is an empty family, which epaint 0.36 lays out as zero glyphs rather than panicking — verified, and pinned by a unit test against an empty base. The two epaint panics it could have hit instead are an unbound family and a key with no data, both guarded. CI runs `cargo test -p egui-reactor-app --no-default-features`.

An app that turns the feature off has to give a `Bundled` or `System` face before the first frame, or draw a text-free loading screen while `pending()` ([0006](0006-loading-policy-belongs-to-the-app.md)).

A curated font set in a separate crate is a follow-up, not a decision: crates.io's 10 MB limit is per crate, so it would have to live outside this one.

## Links

- [ARCHITECTURE section 8, fonts](../../ARCHITECTURE.md#8-platforms)
- `README.md`
- `docs/tasks/font/progress.md`
- Commit `1ee2396`
