# Task: font (font-family style fallback chains, `<Text font>`, three font sources on wasm)

## Goal

Let an egui-react app say which fonts it wants the way CSS does (`"Noto Sans JP", "Hiragino Sans", sans-serif`), have that resolved into egui's per-glyph fallback list, and pick a chain per `<Text>` with a `font` prop. On native the chain may name fonts installed on the device. On wasm, where nothing can read the OS font directory, give the app three ways to get font bytes: bundled into the wasm, fetched over HTTP, or read from the user's machine through the Local Font Access API.

The immediate motivation is that egui's bundled fonts have no CJK glyphs, so Japanese draws as boxes in every example (`examples/theme` had to drop its Japanese locale for this reason).

## Background (as of 2026-09, egui / eframe 0.36.1)

| Layer | State |
|---|---|
| Text rasterization | epaint: skrifa (parsing) + harfrust (shaping) + vello_cpu (rasterizing) into a texture atlas. The browser's text engine and CSS fonts are never involved, so `font-family` has no meaning by itself |
| Bundled fonts | `epaint_default_fonts`: Ubuntu-Light, Hack-Regular, NotoEmoji-Regular, emoji-icon-font. No CJK |
| Registering fonts | `FontDefinitions { font_data: name → FontData(bytes, index, tweak), families: FontFamily → [name, ..] }` + `Context::set_fonts`. `Context::add_font(FontInsert)` appends one font at highest or lowest priority without a rebuild |
| Fallback | Already per glyph: epaint walks the family's list and takes the first font that has the glyph. A CSS-style chain is therefore just the order of that list |
| Naming a chain | `FontFamily::Name(Arc<str>)` + `RichText::family(..)`. A name that is not in `families` **panics** at layout (`"FontFamily::{family:?} is not bound to any fonts"`), and bytes skrifa cannot parse panic in `Fonts::new` |
| System fonts | epaint has no system font enumeration on any target |
| egui-react today | No font code at all. `<Text>` has `size` / `color` / `strong` / `wrap` / `selectable`. `Options::setup(&CreationContext)` is the runner's hook that runs before the first frame, on native and wasm |
| wasm and the OS | wasm32 cannot read files. The only route to installed fonts in a browser is the Local Font Access API (Chromium only, permission prompt, user gesture) |

## Scope

### In scope

- **A font model in the runner crate** (`egui_react_app::fonts`): named chains (`FontStack`) made of sources (`Bundled`, `Url`, `System`, generic names), resolved into a `FontDefinitions` and applied with `set_fonts`. Sources that arrive later (HTTP, Local Font Access) re-apply. A chain can replace the default `Proportional` / `Monospace` lists so widgets pick it up too.
- **One face database on every target, `fontdb`**: on native it loads the installed fonts; on wasm it holds whatever the bundled, fetched and locally granted bytes provided. CSS-style matching (`Query { families, weight, style }`) is fontdb's, pure Rust, no C libraries.
- **`<Text font="...">`** in `egui-react-elements`: a chain name, `"proportional"` or `"monospace"`. An unknown name falls back to the text style's family and logs once; it never panics.
- **wasm, three sources**: `include_bytes!` bundling; HTTP fetch with `ehttp` (TTF / OTF, WOFF2 behind a feature); Local Font Access via `window.queryLocalFonts()` from a click handler, matched by family name, `blob()` → bytes.
- **An example** `examples/font` in the gallery showing all of the above with Japanese sample text, plus one paragraph each in ARCHITECTURE.md and the README.

### Out of scope

- Font weight / italic props on `<Text>`. `<Text>` keeps `strong`; a chain entry may carry a weight for matching, but no `weight` / `italic` props in this task.
- Subsetting fonts at build time (recommended in the docs, not built).
- Color / bitmap-only emoji fonts (Apple Color Emoji, Noto Color Emoji). Whether epaint 0.36 draws them is looked at during implementation and decided there; the text renderer may be swapped later, so nothing in the design depends on the answer. The bundled emoji fonts stay at the tail of every chain either way.
- `@font-face` / `document.fonts`: the browser never hands those bytes back, so it cannot feed epaint.
- Changing egui / epaint upstream.

## Deliverables

- `docs/tasks/font/plan.md` (research and steps; this pair of documents).
- `egui_react_app::fonts` module and the `woff2` cargo feature.
- `font` prop on `<Text>`, with kittest coverage.
- `examples/font`, registered in the gallery.
- ARCHITECTURE.md section 8 bullet and README row.

## Done criteria

- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo check --workspace --target wasm32-unknown-unknown` pass in CI with no new system packages.
- Native: `examples/font` on macOS shows Japanese in Hiragino Sans (or whatever the chain resolves to) with the resolved chain listed on screen; on Ubuntu CI a test resolves `sans-serif` through fontdb or skips cleanly when the runner has no fonts.
- wasm (`trunk serve --config examples/font/Trunk.toml`): the bundled font shows Japanese on first frame; the URL source swaps the font in after the fetch without a reload; in Chromium, the "use my fonts" button prompts for permission and, when granted, draws with the local font. Firefox / Safari show the button disabled with the reason.
- `<Text font="nope">` draws with the default font and logs one warning; the kittest for it passes.

## Decisions

- **One PR.** Decided 2026-09-07; the earlier three-PR split is withdrawn.
- **fontdb, not font-kit.** Decided 2026-09-07. font-kit does not build on wasm32 and links FreeType and fontconfig on Linux; fontdb is pure Rust, builds on every target, and its matching is a port of font-kit's CSS algorithm. The comparison is kept in plan.md section 1.2.
- The resolver lives in `egui-react-app`, not in core or elements: core only touches `&mut egui::Ui` (ARCHITECTURE section 8) and elements cannot depend on the runner. Elements only look at `Context::fonts(|f| f.definitions())` to decide whether a name is safe to use.
- The full `FontDefinitions` is recomputed and `set_fonts` is called on every change, instead of `add_font`. `add_font` can only put a font at the head or the tail, which cannot express a chain.
- Generic family names on wasm resolve to egui's bundled fonts unless a Local Font Access grant or a loaded source supplied something better.
- Emoji / color fonts: decided during implementation, recorded in plan.md section 5 when known.

## Estimate

- Model + resolver + fontdb + bundled source + `<Text font>` + tests: 2 days.
- URL source, WOFF2, Local Font Access, example, docs: 2 to 3 days. Local Font Access is only checkable by hand in Chromium.
