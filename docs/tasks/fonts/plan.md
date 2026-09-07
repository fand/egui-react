# Plan: fonts

The task definition is in [task.md](task.md). This document records what was checked in the sources and the steps decided from that. If implementation departs from it, update this document, and ARCHITECTURE.md where the change has design meaning.

Research done 2026-09. Targets: egui / eframe / epaint 0.36.1 (skrifa 0.44.0, read-fonts 0.41.0, as in `Cargo.lock`), font-kit 0.14.3 (latest, 2025-05-26), fontdb 0.24.0 (alternative, 2026-07-29), web-sys 0.3.83 (locked; 0.3.105 upstream), wuff 0.2.9 (WOFF2, 2026-09-01). All crate sources were read from the `.crate` files, not from memory.

## 0. Overview

Three PRs, each leaving main green and usable on its own.

| | What | Where |
|---|---|---|
| PR A | `egui_react_app::fonts`: the model (`FontStack`, `FontSource`), the resolver to `FontDefinitions`, the `Bundled` source, `<Text font>` in elements, tests, this document | native + wasm, no new dependencies |
| PR B | `System` source on native through font-kit (`system-fonts` feature, default on for native), CI package list, resolver test on Ubuntu | native only |
| PR C | wasm sources: `Url` (ehttp, TTF/OTF; WOFF2 behind `woff2`), `Local` (Local Font Access API), `examples/fonts`, gallery / README / ARCHITECTURE | native + wasm (the `Url` source works natively too) |

The order is chosen so that the thing that unblocks Japanese in the examples (bundling a font and pointing `Proportional` at it) lands first and is small.

## 1. Research results

### 1.1 What epaint 0.36 gives us, and the two panics to design around

- **The fallback chain already exists.** `FontDefinitions::families: BTreeMap<FontFamily, Vec<String>>` is documented as "the first font is the primary, then a list of fallbacks in order of priority", and glyph lookup walks that list per character. A CSS `font-family` list maps one to one onto the order of that `Vec`. Nothing needs to be built on the epaint side; the work is producing the right lists.
- **Named chains** are `FontFamily::Name(Arc<str>)`. `RichText::family(FontFamily)` selects one for a run of text; its doc says "Only the families available in `FontDefinitions::families` may be used". `WidgetText` has no `family` method, only `RichText` does; `WidgetText::RichText(Arc<RichText>)` and `WidgetText::Text(String)` are the variants `<Text>` has to unwrap.
- **Panic 1, unknown family.** `FontsImpl` does `self.definitions.families.get(family).unwrap_or_else(|| panic!("FontFamily::{family:?} is not bound to any fonts"))` (`epaint/src/text/fonts.rs`, around line 1031). So `<Text font="typo">` must never reach egui with an unregistered name. `Context::fonts(|f| f.definitions().families.contains_key(..))` is available for the check (`FontsView::definitions()` and `FontsView::families()` both exist).
- **Panic 2, bad bytes.** `Fonts::new` parses each `FontData` with skrifa and panics on error (`"Error parsing {name:?} TTF/OTF font file"`), which on wasm means a dead canvas. Every byte source (HTTP, Local Font Access, a system file) is validated with `skrifa::FontRef::from_index(bytes, index)` before it is handed over. skrifa is already in the lock at 0.44.0; depending on it directly at the same version costs nothing. epaint does not re-export it.
- **`set_fonts` vs `add_font`.** `Context::add_font(FontInsert { name, data, families: [InsertFontFamily { family, priority: Highest | Lowest }] })` appends without rebuilding the definitions, but it can only put a font at the head or the tail of a family. A chain of three fonts cannot be expressed by appends, so the resolver keeps its own model as the source of truth, recomputes the whole `FontDefinitions`, and calls `set_fonts`. `set_fonts` is a no-op when the definitions are equal, and otherwise takes effect at the start of the next pass with a fresh atlas. The equality check compares the font bytes themselves (the source has a note saying so), so `apply` is something to call on change, never per frame; the design only calls it from `setup` and from a source's completion. The atlas rebuild is paid once per arriving font, which is rare.
- **Default families.** `FontDefinitions::default()` is `Proportional = [Ubuntu-Light, NotoEmoji-Regular, emoji-icon-font]`, `Monospace = [Hack, Ubuntu-Light, NotoEmoji-Regular, emoji-icon-font]`. The two emoji fonts are what draw the gallery's icons, so every chain we produce keeps them at the tail.
- **`FontData`** carries `font: Cow<'static, [u8]>`, `index: u32` (face index in a TTC) and `tweak: FontTweak`. `from_static` / `from_owned` build it. TTC indexes are needed for the macOS and Windows system fonts (Hiragino and Yu Gothic ship as `.ttc`).
- **Where the engine lays out text.** `engine/mod.rs` `text_job` builds the `LayoutJob` from the `WidgetText` with `FontSelection::Default`, and `RichText::family` is honoured inside `into_layout_job`, so setting the family on the `RichText` is enough for all three surfaces (`Ui`, `Tree`, `Lite`). No engine change.
- **Coverage test.** `FontsView::has_glyphs(&FontId, &str)` exists, so a kittest can assert "this chain has glyphs for `日本語`" without pixels.

### 1.2 font-kit 0.14.3

**API that matters.** `SystemSource::new()`, then `Source::select_best_match(&[FamilyName], &Properties) -> Result<Handle, SelectionError>`. It is documented as "font matching according to CSS Fonts Level 3": it walks the family names in order, resolves each to a family, and picks the face whose weight / stretch / style best matches `Properties`. `FamilyName` has `Title(String)`, `Serif`, `SansSerif`, `Monospace`, `Cursive`, `Fantasy`. This is exactly the CSS shape the task asks for. `Handle` is either `Path { path, font_index }` or `Memory { bytes: Arc<Vec<u8>>, font_index }`.

**We do not need font-kit to load or rasterize.** epaint does that. For `Handle::Path` we `std::fs::read` the file and keep `font_index`; for `Handle::Memory` we take the bytes. font-kit's loaders still run inside `select_best_match` (it opens each candidate face to read its properties), so the loader dependency is unavoidable, but no glyph ever goes through it.

**Per platform.**

| Target | `SystemSource` | Handle kind | Generic names |
|---|---|---|---|
| macOS / iOS | CoreText | `Memory` (bytes copied out of CoreText) | `sans-serif` → "Arial", `serif` → "Times New Roman", `monospace` → "Courier New" (fixed constants in `source.rs`) |
| Windows | DirectWrite | `Path` | same constants |
| Linux | fontconfig (`yeslogic-fontconfig-sys`) | `Path` | passes `"sans-serif"` etc. through to fontconfig, which aliases them properly |
| Android | `FsSource` over `/system/fonts` | `Path` | no generic mapping |
| wasm32 | **none**: `SystemSource` is `cfg`'d out, and `freetype-sys` is a hard dependency on every target that is not Windows / macOS / iOS, wasm included, so the crate does not build at all | | |

Two consequences. font-kit goes under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` **and** behind a `system-fonts` feature, so `cargo check --target wasm32-unknown-unknown` never sees it. And on Linux it links `libfreetype` and `libfontconfig` at build time: the CI runner (`ubuntu-latest`, `ci.yml` installs GTK / X11 / Wayland headers) needs `libfreetype6-dev libfontconfig1-dev` added to the same `apt-get install`. The `source-fontconfig-dlopen` feature would remove the fontconfig link dependency but not the FreeType one, so it does not remove the apt line; not worth the moving part.

**Generic names on macOS and Windows are weak.** "Arial" for `sans-serif` is not what a Japanese user wants; the resolver therefore treats a generic name as "the platform's default for that generic, then egui's bundled font", and the documentation tells apps to name the fonts they want before the generic (`"Hiragino Sans", "Yu Gothic UI", "Noto Sans CJK JP", sans-serif`), which is also how CSS is written in practice.

**Alternative recorded: fontdb 0.24.** Pure Rust (ttf-parser), `Database::load_system_fonts()` plus `Query { families: &[Family::Name(..), Family::SansSerif], weight, stretch, style }` → `FaceInfo { source: File(path) | Binary(bytes), index }`. Same query model, no C libraries, builds on wasm for the in-memory case (so the resolver's system source and the Local Font Access cache could share one code path), maintained in 2026 while font-kit's last release is 2025-05. The task asked for font-kit, so PR B uses font-kit; the system source is a two-method trait (`match_family(&[Family], &Properties) -> Option<FontFile>`, `list_families() -> Vec<String>`) so the swap, if wanted later, is confined to one file. Decision point recorded in section 5.

### 1.3 The three wasm sources

**(a) Bundled bytes.** `FontData::from_static(include_bytes!(..))`, registered in `Options::setup` before the first frame. Works identically on native. The only cost is size: NotoSansJP-Regular is about 5.7 MB as a static OTF and about 9 MB as the variable TTF, on top of a gallery wasm that is a few MB. That is acceptable for an app that needs it, and wrong as the default for every example, so the example bundles a small font and documents subsetting (`fontcull` 2.0.1 is a pure Rust subsetter on crates.io; `pyftsubset` is the usual tool) rather than doing it in the build.

**(b) Web font over HTTP.** `ehttp::fetch_async` (already a workspace dependency; `examples/fetch` uses it, native via ureq on a thread, wasm via `fetch`) returns the bytes; the resolver validates them and re-applies. Until they arrive the chain draws with whatever is behind them, which is why the bundled fallbacks stay at the tail. Details:

- **Formats.** skrifa reads TTF / OTF / TTC. WOFF and WOFF2 are wrappers with compression, and skrifa does not read them. Google Fonts' CSS API serves WOFF2 only, but the raw files in the google/fonts GitHub repository and many self-hosted fonts are TTF. For WOFF2, `wuff` 0.2.9 (pure Rust, depends on `brotli-decompressor` and `flate2` only, so it builds for wasm) decodes to OTF bytes; it goes behind a `woff2` feature because Brotli tables are not small. Detection is by the first four bytes (`wOF2` / `wOFF` / `OTTO` / `\0\1\0\0` / `ttcf`).
- **CORS.** A cross-origin fetch from wasm needs `Access-Control-Allow-Origin`. `raw.githubusercontent.com` and `fonts.gstatic.com` send `*`; an app's own origin is the normal case. Same-origin relative URLs (`fonts/NotoSansJP.ttf` next to `index.html`, copied by trunk with `data-trunk rel="copy-file"`) are the recommended default and what the example uses, so the gallery does not depend on a third party being up.
- **`@font-face` cannot help.** The CSS Font Loading API (`document.fonts`, `FontFace`) loads fonts into the browser's own engine and never exposes the bytes, so it cannot feed epaint. Ruled out.

**(c) Local Font Access API.** `navigator.fonts.query()` resolves to an array of `FontData { postscriptName, fullName, family, style, blob() }`. Facts checked:

- Chromium only (Chrome / Edge 103 and later). Firefox and Safari have not shipped it and have no positive signal. The example must feature-detect (`"fonts" in navigator`) and say so.
- Secure context, and the first `query()` shows a permission prompt (`local-fonts` permission). Chromium requires **transient user activation**, so the call has to be made from a click handler, not from `setup`. `navigator.permissions.query({ name: "local-fonts" })` can be used to show state without prompting.
- web-sys **has** `FontData` (feature `"FontData"`; `postscript_name()`, `full_name()`, `family()`, `style()`, `blob() -> Promise<Blob>`) but the `Navigator` binding has **no `fonts` getter** and there is no `FontManager` type (checked in upstream `gen_Navigator.rs` and the feature list). So `navigator.fonts` and `query()` are reached with `js_sys::Reflect::get` + `js_sys::Function::call0`, three lines, no `inline_js`. The `web_sys::FontData` name collides with `egui::FontData`; alias one.
- `blob()` returns the **whole font file**, which for a `.ttc` is the whole collection (Hiragino Sans is about 20 faces in one file). The returned `FontData` names one face, so the face index is recovered by parsing the collection with skrifa and matching `postscriptName` against each face's name table. Sizes are large (Hiragino Sans `.ttc` is several tens of MB); the design reads a blob only for a family the app asked for, never for everything the query returned.
- Matching: `query()` returns every face on the machine; we filter by `family()` (case-insensitive, also try `postscript_name()`) against the names the app listed, and take the face whose `style()` is "Regular" (or the first) when the chain asked for no properties.

### 1.4 Where it plugs into egui-react

- `Options::setup: Option<Box<dyn FnOnce(&eframe::CreationContext)>>` runs first in `ReactApp::new`, before the store is created, on native and wasm. That is the place the app builds its `Fonts` and applies it (`cc.egui_ctx.set_fonts`). The a11y module set the precedent for "runner-level feature with a wasm-only inside": `WebA11y` is one type on every target, with the DOM part under `#[cfg(target_arch = "wasm32")]` in a thread-local registry. `fonts` follows the same shape.
- `egui-react-elements` depends only on `egui` and `egui-react`; `egui-react-app` depends on elements only as a dev-dependency. So the `<Text font>` prop cannot ask the runner anything. It asks the `Context` instead (1.1).
- `Cx::text(style, WidgetText, wrap, selectable)` takes a `WidgetText`; the prop is applied in `<Text>` before that call, no core change.
- Async: sources that arrive later run on the browser's event loop (`wasm_bindgen_futures::spawn_local`) or a thread (native, as `ehttp` already does). They need a `Context` clone to call `set_fonts` and `request_repaint`, which `CreationContext::egui_ctx` gives. The resolver's shared state is an `Arc<Mutex<Model>>`, since `Context` is `Send + Sync` and the native path may complete on another thread.

## 2. Design

### 2.1 Model

```rust
// egui_react_app::fonts
pub enum FontSource {
    /// Bytes compiled into the binary. `index` is the face in a TTC.
    Bundled { name: &'static str, bytes: &'static [u8], index: u32 },
    /// A font installed on the device, matched by family name.
    /// Native: font-kit (feature `system-fonts`). wasm: only what a
    /// `Local` grant has already loaded. Otherwise skipped.
    System(String),
    /// Fetched over HTTP (TTF / OTF, WOFF2 with feature `woff2`).
    /// Skipped until the bytes arrive, then the chain is re-applied.
    Url(String),
    /// CSS generic: resolved to the platform default (native) or to
    /// egui's bundled font (wasm), then egui's bundled font behind it.
    Generic(Generic), // SansSerif | Serif | Monospace | SystemUi
}

pub struct FontStack { pub name: Arc<str>, pub chain: Vec<FontSource> }

pub struct Fonts {
    stacks: Vec<FontStack>,
    /// Which stack, if any, replaces `Proportional` / `Monospace`.
    default_proportional: Option<Arc<str>>,
    default_monospace: Option<Arc<str>>,
    tweak: BTreeMap<String, FontTweak>,
}
```

A builder on top, so the common case is short and reads like CSS:

```rust
Fonts::new()
    .stack("jp", [
        FontSource::System("Hiragino Sans".into()),
        FontSource::System("Yu Gothic UI".into()),
        FontSource::System("Noto Sans CJK JP".into()),
        FontSource::Url("fonts/NotoSansJP-Regular.ttf".into()),
        FontSource::Generic(Generic::SansSerif),
    ])
    .default_proportional("jp")   // widgets and plain <Text> use it too
    .apply(&cc.egui_ctx);
```

A string form is offered for the literal case, parsed once: `Fonts::css("jp", r#""Hiragino Sans", "Yu Gothic UI", url(fonts/NotoSansJP-Regular.ttf), sans-serif"#)`. Quoted or bare names become `System`, `url(..)` becomes `Url`, the five CSS generics become `Generic`. Bundled bytes have no string form.

### 2.2 Resolution

`apply(ctx)` does, synchronously:

1. Start from `FontDefinitions::default()` so the four bundled fonts and their tweaks are always present.
2. For each stack, for each source in order, ask for a `ResolvedFont { key, data: Arc<FontData> }` or nothing:
   - `Bundled` → validate with skrifa once, insert under `name`.
   - `System` → the `SystemFonts` trait object for this target (font-kit on native; on wasm the Local Font Access cache). A miss is skipped, not an error, exactly like a CSS name that is not installed.
   - `Url` → look in the byte cache; on a miss, skip and (once) start the fetch. When the fetch completes, the bytes are validated, cached, and `apply` runs again from the model.
   - `Generic` → native: `SystemFonts::match_generic`; wasm: nothing. Then the bundled font for that generic (`Ubuntu-Light` for sans / serif / system-ui, `Hack` for monospace).
3. The stack's list is the resolved keys in order, deduplicated, followed by egui's own list for the matching built-in family (so emoji and icons work in every chain). Insert under `FontFamily::Name(stack.name)`.
4. If the stack is the default for `Proportional` or `Monospace`, its list also replaces that family's list.
5. `ctx.set_fonts(defs)`; if anything is still pending, nothing else, the completion re-enters at 1.

A `Resolution` report (`Vec<(stack, Vec<(source, Outcome)>)>`, with `Outcome::{Loaded(key), Pending, Missing, Invalid(reason)}`) is kept in the model and readable through `Fonts::report()`; the example prints it, and tests assert on it instead of on pixels.

### 2.3 `<Text font>`

`font: Option<&str>` on `Text`. Resolution, in the component:

```rust
let family = match font {
    None => None,
    Some("proportional") => Some(FontFamily::Proportional),
    Some("monospace") => Some(FontFamily::Monospace),
    Some(name) => {
        let family = FontFamily::Name(name.into());
        if cx.ctx().fonts(|f| f.definitions().families.contains_key(&family)) {
            Some(family)
        } else {
            log::warn!(..); // once per name, via a small `HashSet` in ctx.data
            None
        }
    }
};
```

Applied as `RichText::family` on the text: `WidgetText::Text(s)` becomes `RichText::new(s).family(..)`; `WidgetText::RichText(rt)` becomes `Arc::unwrap_or_clone(rt).family(..)`; `LayoutJob` and `Galley` are left alone (their fonts are already fixed). The check reads the definitions under the fonts lock once per `<Text>` per pass, which is a `BTreeMap` lookup; the 10k-row list benchmark is re-run in step 3 to confirm it is invisible, and if not the known-names set is cached per pass in `ctx.data`.

`Button` / `Checkbox` and the other widgets take `impl Into<WidgetText>` children, so `<Button><Text font="jp">..</Text></Button>` is not how egui works; for them the route is `default_proportional`, or the caller passes `RichText::new(..).family(..)` as the child. Documented on the prop; no other element gets a `font` prop in this task.

### 2.4 Crate and feature layout

```
crates/egui-react-app/
  Cargo.toml        features: default = ["system-fonts"] (native only in effect), woff2 = ["dep:wuff"]
  src/fonts/mod.rs  model, builder, resolver, report, `apply`
  src/fonts/system.rs   trait SystemFonts + `NoSystemFonts`
  src/fonts/font_kit.rs cfg(all(not(wasm32), feature = "system-fonts")): font-kit implementation
  src/fonts/url.rs      ehttp fetch, format sniffing, optional WOFF2 decode
  src/fonts/local.rs    cfg(wasm32): Local Font Access, the grant cache, implements SystemFonts
  src/fonts/validate.rs skrifa parse + TTC face lookup by PostScript name
crates/egui-react-elements/src/view.rs   `font` prop on Text
examples/fonts/       lib.rs (App + META), main.rs, Trunk.toml, index.html, fonts/ (one small OFL font), tests/
```

Dependencies added: `skrifa = "0.44"` (app; already in the lock), `font-kit = "0.14"` (app, native, optional), `wuff = "0.2"` (app, optional), `ehttp` (app; already a workspace dep), `js-sys` and web-sys features `FontData`, `Blob`, `Navigator`, `Permissions`, `PermissionStatus` (app, wasm), `log` in elements (it is not there yet; only for the one warning).

## 3. Steps

Each step ends with the listed checks green: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo check --workspace --target wasm32-unknown-unknown`.

### Step 1 (PR A): model, resolver, `Bundled`, tests

- `fonts/mod.rs` with the types in 2.1, the builder, `apply`, `report`. The `SystemFonts` trait with only `NoSystemFonts` for now. `Url` sources resolve to `Pending` and do nothing yet.
- `fonts/validate.rs`: `fn face(bytes: &[u8], index: u32) -> Result<FaceInfo, Invalid>` over skrifa, returning family and PostScript names (needed for the TTC lookup later and for the report).
- Unit tests (no egui `Context` needed, `FontDefinitions` is plain data): chain order is preserved; a missing `System` name is skipped; the tail is egui's list; `default_proportional` rewrites `Proportional`; duplicates collapse; a re-`apply` with nothing changed produces equal definitions (so `set_fonts` is a no-op); invalid bytes give `Outcome::Invalid`, never a panic. The bundled bytes in tests come from `epaint_default_fonts` (add it as a dev-dependency; `HACK_REGULAR` is a fine stand-in for "a font the app bundled").
- kittest in `egui-react-app/tests/fonts.rs`: apply a `Fonts` with `Bundled(Hack)` as stack `"code"`, draw `<Text font="code">`, assert `ctx.fonts(|f| f.definitions().families[&Name("code")][0] == "code-hack")` and `has_glyphs`.

### Step 2 (PR A): `<Text font>`

- The prop and resolution in 2.3. `log` added to elements.
- kittests in `egui-react-elements/tests`: `font="monospace"` changes the galley's font (compare `galley.job.sections[0].format.font_id.family` against a plain `<Text>`; `Glyph` itself carries no `FontId`); `font="nope"` draws, does not panic, and the family on the galley is the default; the warning is emitted once for two `<Text font="nope">` (capture with a test logger or assert on the `ctx.data` set).
- Re-run the list-10k measurement from `docs/tasks/list-perf/measurements.md` once with every row carrying `font="mono"` to confirm the lookup is invisible (target: within noise of the current number).
- Check whether epaint 0.36 draws COLR / sbix / CBDT glyphs (`skrifa` exposes `color_glyphs()`; look at what `epaint::text::font` calls). Record the answer in section 5 and, if it does not, make `validate` reject faces with no outline glyphs so a chain never leads with a font that renders nothing.

### Step 3 (PR B): font-kit system source

- `fonts/font_kit.rs`: `FontKitSource(SystemSource)` implementing `SystemFonts`. `match_family(names, props)` maps `FontSource::System(name)` to `FamilyName::Title` and `Generic` to the font-kit generic, calls `select_best_match` with `Properties::new()` (regular weight), and turns the `Handle` into bytes (`Path` → `fs::read`, `Memory` → the `Arc<Vec<u8>>`) plus the face index. `SystemSource::new()` is built once per `Fonts` and kept; on Linux the first call loads the fontconfig cache (tens of ms), so it is created lazily on the first `System` source.
- Feature `system-fonts` on `egui-react-app`, default on; the module is `cfg(all(not(target_arch = "wasm32"), feature = "system-fonts"))`. Without it, `System` resolves to `Missing` on native too.
- `ci.yml` and `pages.yml`: add `libfreetype6-dev libfontconfig1-dev` to the apt list. Confirm `fonts-dejavu-core` is on `ubuntu-latest` (it is on the current image; if it is gone, install it, the test below depends on at least one sans-serif being present).
- Test (native, not `#[ignore]`): resolve `[System("This Font Does Not Exist"), Generic(SansSerif)]`; assert the first is `Missing`, and the second is `Loaded` **or** the whole report says no system font was found, in which case the test prints that and passes (a runner with no fonts is not a failure of this code). A second test asserts the loaded bytes pass `validate`.
- Try it by hand on macOS: `"Hiragino Sans"` resolves to a `Memory` handle out of a `.ttc`, the index is not 0, and Japanese draws. Record what CoreText hands back for `SansSerif` (expected "Arial") in section 5.

### Step 4 (PR C): `Url` source

- `fonts/url.rs`: on first sight of a `Url`, `ehttp::fetch` (callback form works on both targets without an executor). On completion: sniff the magic, decode WOFF2 with `wuff` if the feature is on (else `Invalid("woff2 feature is off")`), `validate`, store in the model's byte cache, re-`apply`, `ctx.request_repaint()`. Errors go to the report and `log::warn!`. One in-flight fetch per URL.
- Native test with a local `std::net::TcpListener` serving a font file (ehttp uses ureq natively, so this is a real round trip): the chain reports `Pending` after the first `apply`, `Loaded` after the callback, and the definitions changed exactly once.
- wasm: `cargo check` only; the example is the test (step 6).

### Step 5 (PR C): Local Font Access

- `fonts/local.rs` (wasm only): `pub async fn request_local_fonts(fonts: &Fonts, ctx: &Context) -> Result<usize, JsValue>`: `Reflect::get(navigator, "fonts")` → `query()` → for each face whose `family()` matches a `System(name)` in any stack (case-insensitive), pick the regular style, `blob()` → `array_buffer()` → `Vec<u8>` → `validate` (with the TTC lookup by `postscript_name()` to get the index) → into the grant cache; then re-`apply`. Returns how many families were filled. The function is `async` and meant to be called from an `on_click` inside `spawn_local`, because of the user activation rule; the doc says so and the example shows it.
- `pub fn local_fonts_available() -> bool` (`"fonts" in navigator`) and `pub async fn local_fonts_permission() -> Option<PermissionState>` for the UI.
- On wasm the `SystemFonts` implementation is the grant cache, so after a grant the same `System("Hiragino Sans")` entry that native resolves through font-kit resolves here through the browser. Same model, same report.
- Test: the parsing helpers (name matching, TTC face lookup) are plain Rust and get unit tests on native with a two-face collection built from `epaint_default_fonts` bytes (a TTC can be assembled in the test; if that is more than 40 lines, test the lookup on single-face fonts and leave the collection case to the manual check). The browser round trip is a manual check in Chromium, written up in the example's README section.

### Step 6 (PR C): `examples/fonts`

- Layout: a title, a row of three radio-style buttons for the sample stack (`bundled` / `web` / `system`), a sample paragraph in Japanese and English drawn with `<Text font=..>`, and a table of the current `report()` (source → outcome), which is the part that teaches what a chain did.
- `bundled`: one small OFL font shipped in `examples/fonts/fonts/` (under 500 KB; candidates: a Latin + kana subset of Noto Sans JP made with `pyftsubset` and committed, or M PLUS 1p subset; the license file goes next to it). It is also the `default_proportional`, so the whole example UI is in it.
- `web`: `Url("fonts/NotoSansJP-Regular.ttf")` served same-origin by trunk (`<link data-trunk rel="copy-file" href="fonts/NotoSansJP-Regular.ttf">`; the file is not committed, `Trunk.toml` has a pre-build hook that downloads it into `dist/`, and the native binary reads the same relative path from the example directory). Shows the Pending → Loaded transition live.
- `system`: native, `["Hiragino Sans", "Yu Gothic UI", "Noto Sans CJK JP", sans-serif]` through font-kit. wasm: a "use my fonts" button, disabled with a one-line reason on browsers without the API, which calls `request_local_fonts` and then shows the same chain resolved from the grant.
- `META` with `hooks: ["use_state", "use_effect"]`, `elements: ["View", "Text", "Button"]`; added to `EXAMPLES` in the gallery and to the README table. `theme` may get its Japanese locale back in a follow-up now that the gallery has a CJK font; not in this task.
- kittest: the example renders and the report table lists the three stacks.

### Step 7 (PR C): docs

- ARCHITECTURE.md section 8: one bullet "Fonts", stating that text is rasterized by epaint from bytes, that the runner's `fonts` module resolves CSS-like chains into `FontDefinitions`, which sources exist per target, and the two panics the design guards against. Section 7's crate list gets `fonts` after `run(..)`.
- README: the row for `examples/fonts`, and a sentence under the wasm build notes that bundled fonts add to the wasm size.

## 4. API in one screen (what the app writes)

```rust
use egui_react_app::fonts::{Fonts, FontSource, Generic};

fn main() -> eframe::Result {
    let fonts = Fonts::new()
        .stack("jp", [
            FontSource::System("Hiragino Sans".into()),
            FontSource::System("Yu Gothic UI".into()),
            FontSource::Url("fonts/NotoSansJP-Regular.ttf".into()),
            FontSource::Bundled { name: "fallback-kana", bytes: include_bytes!("../fonts/kana.ttf"), index: 0 },
            FontSource::Generic(Generic::SansSerif),
        ])
        .stack("code", [FontSource::System("JetBrains Mono".into()), FontSource::Generic(Generic::Monospace)])
        .default_proportional("jp")
        .default_monospace("code");

    egui_react_app::run(
        Options {
            setup: Some(Box::new(move |cc| fonts.apply(&cc.egui_ctx))),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}

// in a component
rsx! {
    <Text font="jp" size={18.0}>"日本語のテキスト"</Text>
    <Text font="code">"let x = 1;"</Text>
}
```

## 5. Risks, open questions, decision points

- **font-kit vs fontdb.** Recorded in 1.2. Cost of font-kit: two C libraries on Linux (CI apt line, and every Linux user building an egui-react app needs the `-dev` packages, which is a real onboarding cost the README must state), a crate last released in 2025-05, and generic-name constants that are wrong for CJK on macOS / Windows. Benefit: exactly the CSS matching algorithm the task named. **Recommendation: build PR B on font-kit as asked, keep the trait boundary, and revisit if the Linux packaging cost bites.** If the choice is reopened before PR B starts, fontdb is the same amount of work.
- **Color fonts.** Unknown until step 2 whether epaint 0.36 draws COLR / bitmap emoji. If it does not, a `System("Apple Color Emoji")` entry would be `Loaded` but paint nothing; the outline check in `validate` handles it.
- **macOS hidden system fonts.** CoreText may return `.SF NS` faces for some queries; skrifa parses them fine, but they are not meant to be enumerated. Not a problem for named chains; noted for the report output.
- **Font size on wasm.** A CJK font is 5 to 10 MB. The `Url` source with same-origin hosting keeps it out of the wasm; the docs recommend subsetting for `Bundled`. Nothing in the code prevents an app from bundling 10 MB; that is the app's call.
- **`set_fonts` cost.** Each arriving font rebuilds the atlas and every galley once. Two or three arrivals at startup are fine; a chain of ten URLs would flicker ten times. The `Url` source could batch: wait for all URLs in a chain before the first re-apply. Not built until someone needs it; noted in the module docs.
- **`<Text font>` lookup per pass.** A `BTreeMap<FontFamily, _>` lookup under the fonts `RwLock` per `<Text>`. Measured in step 2; the fallback is a per-pass cache.
- **Local Font Access blobs are whole files.** A user with Hiragino installed grants access and the app pulls a 40 MB `.ttc` into wasm memory. Acceptable once; the grant cache keeps it for the session and never re-reads. The docs say to list the specific families wanted, never all.
- **WOFF2 decoder.** `wuff` 0.2.9 is MIT and 27 KB of source; its weight is in `brotli-decompressor`. `woff2-patched` 0.4.0 is the fallback if it turns out not to handle a real Google Fonts file in step 4.
- **Weight and italics.** `Properties::new()` always asks for regular. `strong` on `<Text>` is a synthetic bold in egui (text is drawn slightly wider), so it keeps working. Real weight matching would be `FontSource::System { name, weight }` and a `weight` prop; left for a later task.

## 6. Out of scope, confirmed while researching

- Changing epaint's fallback semantics (they are already what CSS does).
- Reading fonts from the browser's own font engine (`document.fonts`): the bytes are not exposed.
- Native OS "generic" mapping better than font-kit's constants: needs per-platform tables (fontconfig does it on Linux; macOS would need `CTFontCreateUIFontForLanguage`, Windows `IDWriteFontFallback`); a later task, and the reason apps should name fonts before the generic.
