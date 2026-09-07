# Plan: font

The task definition is in [task.md](task.md). This document records what was checked in the sources and the steps decided from that. If implementation departs from it, update this document, and ARCHITECTURE.md where the change has design meaning.

Research done 2026-09. Targets: egui / eframe / epaint 0.36.1 (skrifa 0.44.0, read-fonts 0.41.0, as in `Cargo.lock`), fontdb 0.24.0 (2026-07-29), font-kit 0.14.3 (evaluated and not used), web-sys 0.3.83 (locked; 0.3.105 upstream), wuff 0.2.9 (WOFF2, MIT, 2026-09-01). All crate sources were read from the `.crate` files, not from memory.

## 0. Overview

One PR, on the branch this document lands on. Steps in section 3 are ordered so that every intermediate commit builds and tests green on native and `cargo check`s on wasm, but nothing is merged before the example runs.

The shape in one sentence: **a `fontdb::Database` is the single place font bytes live on every target; a chain is a list of fontdb queries; the resolver turns the answers into egui's `FontDefinitions` and calls `set_fonts`; sources (bundled, system directory, HTTP, Local Font Access) only ever *feed bytes into the database* and ask for a re-resolve.**

## 1. Research results

### 1.1 What epaint 0.36 gives us, and the two panics to design around

- **The fallback chain already exists.** `FontDefinitions::families: BTreeMap<FontFamily, Vec<String>>` is documented as "the first font is the primary, then a list of fallbacks in order of priority", and glyph lookup walks that list per character. A CSS `font-family` list maps one to one onto the order of that `Vec`. Nothing needs to be built on the epaint side; the work is producing the right lists.
- **Named chains** are `FontFamily::Name(Arc<str>)`. `RichText::family(FontFamily)` selects one for a run of text; its doc says "Only the families available in `FontDefinitions::families` may be used". `WidgetText` has no `family` method, only `RichText` does; `WidgetText::RichText(Arc<RichText>)` and `WidgetText::Text(String)` are the variants `<Text>` has to unwrap.
- **Panic 1, unknown family.** `FontsImpl` does `self.definitions.families.get(family).unwrap_or_else(|| panic!("FontFamily::{family:?} is not bound to any fonts"))` (`epaint/src/text/fonts.rs`, around line 1031). So `<Text font="typo">` must never reach egui with an unregistered name. `Context::fonts(|f| f.definitions().families.contains_key(..))` is available for the check (`FontsView::definitions()` and `FontsView::families()` both exist).
- **Panic 2, bad bytes.** `Fonts::new` parses each `FontData` with skrifa and panics on error (`"Error parsing {name:?} TTF/OTF font file"`), which on wasm means a dead canvas. fontdb rejects bytes whose table directory does not parse (`LoadError::MalformedFont`) when they are loaded, which is the same depth of check `skrifa::FontRef::from_index` does; the resolver additionally runs `skrifa::FontRef::from_index(bytes, index)` on every face it is about to hand over, so the check is literally epaint's own. skrifa is already in the lock at 0.44.0. epaint does not re-export it.
- **`set_fonts` vs `add_font`.** `Context::add_font(FontInsert { name, data, families: [InsertFontFamily { family, priority: Highest | Lowest }] })` appends without rebuilding the definitions, but it can only put a font at the head or the tail of a family. A chain of three fonts cannot be expressed by appends, so the resolver keeps its own model as the source of truth, recomputes the whole `FontDefinitions`, and calls `set_fonts`. `set_fonts` is a no-op when the definitions are equal, otherwise it takes effect at the start of the next pass with a fresh atlas. The equality check compares the font bytes themselves (the source has a note saying so), so `apply` is something to call on change, never per frame; the design only calls it from `setup` and from a source's completion.
- **Default families.** `FontDefinitions::default()` is `Proportional = [Ubuntu-Light, NotoEmoji-Regular, emoji-icon-font]`, `Monospace = [Hack, Ubuntu-Light, NotoEmoji-Regular, emoji-icon-font]`. The two emoji fonts are what draw the gallery's icons, so every chain we produce keeps them at the tail.
- **`FontData`** carries `font: Cow<'static, [u8]>`, `index: u32` (face index in a TTC) and `tweak: FontTweak`. `from_static` / `from_owned` build it. TTC indexes are needed for the macOS and Windows system fonts (Hiragino and Yu Gothic ship as `.ttc`).
- **Where the engine lays out text.** `engine/mod.rs` `text_job` builds the `LayoutJob` from the `WidgetText` with `FontSelection::Default`, and `RichText::family` is honoured inside `into_layout_job`, so setting the family on the `RichText` is enough for all three surfaces (`Ui`, `Tree`, `Lite`). No engine change.
- **Coverage test.** `FontsView::has_glyphs(&FontId, &str)` exists, so a kittest can assert "this chain has glyphs for `日本語`" without pixels. `Glyph` carries no `FontId`; to see which family a galley used, read `galley.job.sections[i].format.font_id`.

### 1.2 fontdb 0.24 (chosen) and font-kit 0.14 (evaluated)

**fontdb.** `Database::new()`; `load_system_fonts()` (feature `fs`: Windows `%SYSTEMROOT%\Fonts` and the per-user dirs, macOS `/Library/Fonts`, `/System/Library/Fonts`, the `AssetsV2` font assets and `~/Library/Fonts`, Linux via a pure Rust fontconfig config parser (`fontconfig-parser`, feature `fontconfig`) with a fallback to the known directories); `load_font_data(Vec<u8>)` / `load_font_source(Source::Binary(Arc<dyn AsRef<[u8]>>)) -> TinyVec<[ID; 8]>` for bytes (one `ID` per face in a collection); `query(&Query { families: &[Family], weight, stretch, style }) -> Option<ID>`; `face(id) -> &FaceInfo { families: Vec<(String, Language)>, post_script_name, index, style, weight, stretch, monospaced, source }`; `with_face_data(id, |bytes, index| ..)`. Facts that matter:

- `query` walks `families` in order and, for the first family that has faces, runs `find_best_match`, a port of font-kit's CSS Fonts Level 3 matching (the source cites both). That is the `font-family` algorithm the task asked for.
- Family matching is an exact string compare against the face's name-table families (all languages present in the table, so `"ヒラギノ角ゴシック"` and `"Hiragino Sans"` both work). Not case-insensitive; the resolver lowercases nothing and documents that names are as the font declares them.
- Generic families are five settable names. Defaults are "Times New Roman" / "Arial" / "Courier New" everywhere; on Linux `load_system_fonts` reads fontconfig's `<alias>` entries and sets them properly. On macOS / Windows the resolver keeps the defaults and puts egui's bundled font behind, so a generic name never resolves to nothing; apps name the CJK fonts they want before the generic, as CSS is written in practice.
- Builds everywhere. `#![cfg_attr(not(feature = "std"), no_std)]`; dependencies are `log`, `slotmap`, `tinyvec`, optional `memmap2`, and `fontconfig-parser` only on Linux. Name tables are parsed by a vendored minimal ttf-parser. For wasm: `default-features = false, features = ["std"]` (no `fs`, no `memmap`). No C libraries on any target, so CI needs no new packages.
- `Source::File(path)` is read on each `with_face_data`; `SharedFile` (memmap) and `Binary` give a slice. epaint needs owned bytes, so the resolver copies once per face it registers, never for faces it only looked at.

**font-kit, why not.** Same API shape (`SystemSource::select_best_match(&[FamilyName], &Properties)`, `Handle::{Path, Memory}`), but: `SystemSource` is `cfg`'d out on wasm32 and `freetype-sys` is a hard dependency there, so the crate does not build for the web target and the wasm path would have needed a second implementation; on Linux it links `libfreetype` and `libfontconfig` (CI apt line, and every Linux user building an egui-react app needs the `-dev` packages); last release 2025-05. Decided against on 2026-09-07.

### 1.3 The three wasm sources

**(a) Bundled bytes.** `include_bytes!`, loaded into the database in `Options::setup` before the first frame. Works identically on native. The only cost is size: NotoSansJP-Regular is about 5.7 MB as a static OTF and about 9 MB as the variable TTF, on top of a gallery wasm that is a few MB. That is acceptable for an app that needs it, and wrong as the default for every example, so the example bundles a small font and documents subsetting (`fontcull` 2.0.1 is a pure Rust subsetter on crates.io; `pyftsubset` is the usual tool) rather than doing it in the build.

**(b) Web font over HTTP.** `ehttp::fetch` (already a workspace dependency; `examples/fetch` uses it, native via ureq on a thread, wasm via `fetch`) returns the bytes; the resolver loads them into the database and re-applies. Until they arrive the chain draws with whatever is behind them, which is why the bundled fallbacks stay at the tail. Details:

- **Formats.** skrifa reads TTF / OTF / TTC. WOFF and WOFF2 are wrappers with compression, and skrifa does not read them. Google Fonts' CSS API serves WOFF2 only, but the raw files in the google/fonts GitHub repository and most self-hosted fonts are TTF. For WOFF2, `wuff` 0.2.9 (pure Rust, MIT, depends on `brotli-decompressor` and `flate2` only, so it builds for wasm) decodes to OTF bytes; it goes behind a `woff2` feature because Brotli tables are not small. Detection is by the first four bytes (`wOF2` / `wOFF` / `OTTO` / `\0\1\0\0` / `true` / `ttcf`). If `wuff` chokes on a real Google Fonts file, `woff2-patched` 0.4.0 is the fallback.
- **CORS.** A cross-origin fetch from wasm needs `Access-Control-Allow-Origin`. `raw.githubusercontent.com` and `fonts.gstatic.com` send `*`; an app's own origin is the normal case. Same-origin relative URLs (`fonts/NotoSansJP-Regular.otf` next to `index.html`, copied by trunk with `data-trunk rel="copy-file"`) are the recommended default and what the example uses, so the gallery does not depend on a third party being up.
- **`@font-face` cannot help.** The CSS Font Loading API (`document.fonts`, `FontFace`) loads fonts into the browser's own engine and never exposes the bytes, so it cannot feed epaint. Ruled out.

**(c) Local Font Access API.** `navigator.fonts.query()` resolves to an array of `FontData { postscriptName, fullName, family, style, blob() }`. Facts checked:

- Chromium only (Chrome / Edge 103 and later). Firefox and Safari have not shipped it and have no positive signal. The example must feature-detect (`"fonts" in navigator`) and say so.
- Secure context, and the first `query()` shows a permission prompt (`local-fonts` permission). Chromium requires **transient user activation**, so the call has to be made from a click handler, not from `setup`. `navigator.permissions.query({ name: "local-fonts" })` can be used to show state without prompting.
- web-sys **has** `FontData` (feature `"FontData"`; `postscript_name()`, `full_name()`, `family()`, `style()`, `blob() -> Promise<Blob>`) but the `Navigator` binding has **no `fonts` getter** and there is no `FontManager` type (checked in upstream `gen_Navigator.rs` and the feature list). Found in step 5: in the locked web-sys 0.3.104 the whole `FontData` type is behind `--cfg=web_sys_unstable_apis`, a rustc flag every user of egui-react-app would then have to set, so the binding is not used either. `navigator.fonts`, `query()`, each face's `family` / `postscriptName` and `blob()` are all reached with `js_sys::Reflect::get` + `js_sys::Function::call0`; only `Blob::array_buffer` and the `Permissions` API come from web-sys.
- `blob()` returns the **whole font file**, which for a `.ttc` is the whole collection (Hiragino Sans is about 20 faces in one file). Loading the blob into fontdb gives one `ID` per face with its `post_script_name`, so the face the browser named is found by comparing `postscriptName`; nothing needs to be parsed by hand. Sizes are large (Hiragino Sans `.ttc` is several tens of MB); the design reads a blob only for a family the app asked for, never for everything the query returned.
- Matching: `query()` returns every face on the machine; we filter by `family()` against the `System(name)` entries the app listed, take one face per family for the blob (the file contains the rest), and let fontdb's `query` pick the face afterwards, exactly as on native.

### 1.4 Where it plugs into egui-react

- `Options::setup: Option<Box<dyn FnOnce(&eframe::CreationContext)>>` runs first in `ReactApp::new`, before the store is created, on native and wasm. That is the place the app builds its `Fonts` and applies it (`cc.egui_ctx`). The a11y module set the precedent for "runner-level feature with a wasm-only inside": `WebA11y` is one type on every target, with the DOM part under `#[cfg(target_arch = "wasm32")]`. `fonts` follows the same shape.
- `egui-react-elements` depends only on `egui` and `egui-react`; `egui-react-app` depends on elements only as a dev-dependency. So the `<Text font>` prop cannot ask the runner anything. It asks the `Context` instead (1.1).
- `Cx::text(style, WidgetText, wrap, selectable)` takes a `WidgetText`; the prop is applied in `<Text>` before that call, no core change.
- Async: sources that arrive later run on the browser's event loop or a thread (native, as `ehttp` already does). They need a `Context` clone to call `set_fonts` and `request_repaint`, which `CreationContext::egui_ctx` gives. The resolver's shared state is `Arc<Mutex<Inner>>` (`fontdb::Database` is `Send + Sync`), since the native fetch completes on another thread.

## 2. Design

### 2.1 Model

```rust
// egui_react_app::fonts
pub enum FontSource {
    /// Bytes compiled into the binary. Loaded into the database on `apply`;
    /// the chain entry is "the best face among the ones this blob had".
    Bundled(&'static [u8]),
    /// A font installed on the device, by family name as the font declares it.
    /// Native: whatever `load_system_fonts` found. wasm: what a `Local` grant
    /// or another source already put in the database. Otherwise skipped.
    System(String),
    /// Fetched over HTTP (TTF / OTF / TTC; WOFF2 with feature `woff2`).
    /// Skipped until the bytes arrive, then the chain is re-applied.
    Url(String),
    /// CSS generic: fontdb's answer (fontconfig aliases on Linux, "Arial" and
    /// friends elsewhere), then egui's bundled font for that generic behind it.
    Generic(Generic), // SansSerif | Serif | Monospace | Cursive | Fantasy
}

pub struct FontStack {
    pub name: Arc<str>,
    pub chain: Vec<FontSource>,
    /// Matching properties for every entry (regular / normal by default).
    pub weight: fontdb::Weight,
    pub style: fontdb::Style,
}

pub struct Fonts(Arc<Mutex<Inner>>);
struct Inner {
    db: fontdb::Database,
    stacks: Vec<FontStack>,
    default_proportional: Option<Arc<str>>,
    default_monospace: Option<Arc<str>>,
    /// Which faces each `Bundled` / `Url` produced, so a chain entry can name them.
    loaded: HashMap<SourceKey, Vec<fontdb::ID>>,
    pending: HashSet<String>,          // URLs in flight
    report: Vec<StackReport>,
    ctx: Option<egui::Context>,        // set by `apply`, used by completions
}
```

A builder on top, so the common case is short and reads like CSS:

```rust
Fonts::new()
    .stack("jp", [
        FontSource::System("Hiragino Sans".into()),
        FontSource::System("Yu Gothic UI".into()),
        FontSource::System("Noto Sans CJK JP".into()),
        FontSource::Url("fonts/NotoSansJP-Regular.otf".into()),
        FontSource::Generic(Generic::SansSerif),
    ])
    .default_proportional("jp")   // widgets and plain <Text> use it too
    .apply(&cc.egui_ctx);
```

A string form for the literal case, parsed once: `Fonts::css("jp", r#""Hiragino Sans", "Yu Gothic UI", url(fonts/NotoSansJP-Regular.otf), sans-serif"#)`. Quoted or bare names become `System`, `url(..)` becomes `Url`, the five CSS generics become `Generic`. Bundled bytes have no string form.

`Fonts` is `Clone` (it is the `Arc`), so the app can keep one in a `use_context` or a static and call `request_local_fonts` / `report()` from components.

### 2.2 Resolution

`apply(ctx)` does, synchronously, under the mutex:

1. First call only: `db.load_system_fonts()` on native (behind `cfg(not(wasm32))`; tens to a few hundred ms on a machine with many fonts, once). Load every `Bundled` blob not yet loaded and remember its `ID`s. Start a fetch for every `Url` not loaded and not in flight.
2. Start from `FontDefinitions::default()` so the four bundled fonts and their tweaks are always present.
3. For each stack, for each source in order, produce zero or one egui font key:
   - `Bundled` / `Url` (loaded) → `find_best_match` among that blob's `ID`s with the stack's weight / style (fontdb's `query` cannot be told "only these IDs", so this is a `Query` on a family name taken from the first face plus a filter on the IDs; twenty lines).
   - `System(name)` → `db.query(&Query { families: &[Family::Name(name)], weight, style, .. })`.
   - `Generic` → `db.query` with the generic `Family`, then the bundled egui font for that generic (`Ubuntu-Light` for sans / serif / cursive / fantasy, `Hack` for monospace) regardless of the answer.
   - `Url` (pending / failed) → nothing, recorded as `Pending` / `Failed`.
   For a hit: key = `"{post_script_name}#{index}"`, and if not already in `font_data`, copy the bytes with `with_face_data`, run `skrifa::FontRef::from_index`, and insert `Arc<FontData>` (with `index`). A face that skrifa rejects is `Invalid`, skipped, logged.
4. The stack's list is the keys in order, deduplicated, followed by egui's own list for the matching built-in family (so emoji and icons work in every chain). Insert under `FontFamily::Name(stack.name)`.
5. If the stack is the default for `Proportional` or `Monospace`, its list also replaces that family's list.
6. `ctx.set_fonts(defs)`. Store the report.

A completion (fetch done, Local Font Access granted) loads the bytes into `db`, then calls `apply` again with the stored `ctx` and `ctx.request_repaint()`. Faces are shared: two stacks that resolve to the same file share one `Arc<FontData>`.

`report()` returns `Vec<StackReport { name, entries: Vec<(FontSource, Outcome)> }>` with `Outcome::{Loaded { key, family: String }, Pending, Missing, Invalid(String), Failed(String)}`. The example prints it; tests assert on it instead of on pixels.

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
            warn_once(ctx, name); // log::warn!, once per name, via a HashSet in ctx.data
            None
        }
    }
};
```

Applied as `RichText::family` on the text: `WidgetText::Text(s)` becomes `RichText::new(s).family(..)`; `WidgetText::RichText(rt)` becomes `Arc::unwrap_or_clone(rt).family(..)`; `LayoutJob` and `Galley` are left alone (their fonts are already fixed). The check reads the definitions under the fonts lock once per `<Text>` per pass, a `BTreeMap` lookup; the 10k-row list benchmark is re-run in step 3 to confirm it is invisible, and if not the known-names set is cached per pass in `ctx.data`.

`Button` / `Checkbox` and the other widgets take `impl Into<WidgetText>` children; for them the route is `default_proportional`, or the caller passes `RichText::new(..).family(..)` as the child. Documented on the prop; no other element gets a `font` prop in this task.

### 2.4 Crate and feature layout

```
crates/egui-react-app/
  Cargo.toml        fontdb (native: default features; wasm: default-features = false, features = ["std"]),
                    skrifa = "0.44", ehttp; feature woff2 = ["dep:wuff"]; wasm: js-sys + web-sys features
  src/fonts/mod.rs      model, builder, `css` parser, `apply`, report
  src/fonts/resolve.rs  the six steps of 2.2 over a `&fontdb::Database` (pure, unit-testable)
  src/fonts/url.rs      ehttp fetch, format sniffing, optional WOFF2 decode
  src/fonts/local.rs    cfg(wasm32): Local Font Access
crates/egui-react-elements/src/view.rs   `font` prop on Text (+ `log` dependency)
examples/font/        lib.rs (App + META), main.rs, Trunk.toml, index.html, fonts/ (one small OFL font + license), tests/
```

## 3. Steps

Each step ends with the listed checks green: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo check --workspace --target wasm32-unknown-unknown`.

### Step 1: model, resolver, `Bundled`, `System`, `Generic`

- `fonts/mod.rs` and `fonts/resolve.rs` with the types in 2.1 and the algorithm in 2.2. `Url` sources resolve to `Pending` and start nothing yet.
- Unit tests on `resolve` with a `Database` built in the test from `epaint_default_fonts` bytes (dev-dependency; `HACK_REGULAR` and `UBUNTU_LIGHT` stand in for "a font the app bundled" and "a font installed on the device"): chain order is preserved; a missing `System` name is `Missing` and skipped; the tail is egui's list; `default_proportional` rewrites `Proportional`; duplicates collapse to one key and one `Arc<FontData>`; a re-`apply` with nothing changed produces equal definitions; invalid bytes are `Invalid`, never a panic; `Generic(Monospace)` puts `Hack` in even when the database has no monospace face.
- Native test: `load_system_fonts` then `[System("This Font Does Not Exist"), Generic(SansSerif)]`; the first is `Missing`; the second is `Loaded` **or** the database is empty, in which case the test prints that and passes (a runner with no fonts is not a failure of this code). `fonts-dejavu-core` is on `ubuntu-latest` today.
- kittest in `egui-react-app/tests/fonts.rs`: apply a `Fonts` with `Bundled(HACK_REGULAR)` as stack `"code"`, run a frame, assert `ctx.fonts(|f| f.definitions().families[&Name("code")])` starts with Hack's key and that the family draws with Hack. Found while writing it: `FontsView::has_glyphs` cannot be the check, because epaint answers "no" for any character owned by the face that also supplies the replacement glyph, and Hack has U+FFFD, so `has_glyphs("hello")` is false for every Hack-first family, egui's own `Monospace` included. The test compares `glyph_width('W')` between the stack, `Monospace` and `Proportional` instead.
- `cargo check --target wasm32-unknown-unknown` with fontdb's wasm feature set.

### Step 2: `<Text font>`

- The prop and resolution in 2.3. `log` added to elements.
- kittests in `egui-react-elements/tests`: `font="monospace"` changes the galley's font (compare `galley.job.sections[0].format.font_id.family` against a plain `<Text>`); `font="nope"` draws, does not panic, and the family on the galley is the default; the warning fires once for two `<Text font="nope">` (assert on the `ctx.data` set).

### Step 3: measurements and the emoji decision

- Re-run the list-10k measurement from `docs/tasks/list-perf/measurements.md` once with every row carrying `font="mono"` to confirm the lookup is invisible (target: within noise of the current number). If not, cache the known-names set per pass.
- Check whether epaint 0.36 draws COLR / sbix / CBDT glyphs (`epaint::text::font` and what it asks skrifa for). Decide there: either filter faces with no outline glyphs in `resolve` so a chain never leads with a font that renders nothing, or leave them through. Record the answer in section 5. Either way the bundled emoji fonts stay at the tail.

### Step 4: `Url` source

- `fonts/url.rs`: on first sight of a `Url`, `ehttp::fetch` (callback form, works on both targets without an executor). On completion: sniff the magic, decode WOFF2 with `wuff` if the feature is on (else `Failed("woff2 feature is off")`), `db.load_font_source(Source::Binary(..))`, remember the `ID`s, re-`apply`, `ctx.request_repaint()`. Errors go to the report and `log::warn!`. One in-flight fetch per URL. As built, the same feature also decodes WOFF (version 1), since `wuff` ships both decoders and the zlib side costs little.
- Native test with a local `std::net::TcpListener` serving a font file (ehttp uses ureq natively, so this is a real round trip): the chain reports `Pending` after the first `apply`, `Loaded` after the callback, and the definitions changed exactly once.
- wasm: `cargo check`; the example is the test.

### Step 5: Local Font Access

- `fonts/local.rs`: `Fonts::request_local_fonts(&self) -> impl Future<Output = Result<usize, LocalFontsError>>`: `Reflect::get(navigator, "fonts")` → `query()` → for each face whose `family` equals a `System(name)` in any stack, one `blob()` per family → `array_buffer()` → `Vec<u8>` → `db.load_font_source` → re-`apply`. Returns how many families were filled. It is `async` and meant to be called from an `on_click` inside `spawn_local`, because of the user activation rule; the doc says so and the example shows it. As built, the error is `LocalFontsError(String)` rather than `JsValue`, and the permission state is a `LocalFontsPermission { Granted, Denied, Prompt }` rather than `web_sys::PermissionState`: neither web type exists on native, and the point of the native stubs is that the example is one source file with one set of types.
- `Fonts::local_fonts_available() -> bool` (`"fonts" in navigator`) and `Fonts::local_fonts_permission() -> impl Future<Output = Option<LocalFontsPermission>>` for the UI.
- After a grant the same `System("Hiragino Sans")` entry that native resolves through `load_system_fonts` resolves here through the browser. Same model, same report.
- On native these three are present and return `false` / `None` / `Ok(0)` so the example is one source file. `Fonts::load_font_data(Vec<u8>)` is the primitive underneath, public so an app with its own way of obtaining bytes (a file picker) can feed the same database.
- The browser round trip is a manual check in Chromium, written up in the example's doc comment.

### Step 6: `examples/font`

- Layout: a title, three buttons for the sample stack (`bundled` / `web` / `system`), a sample paragraph in Japanese and English drawn with `<Text font=..>`, and a table of the current `report()` (source → outcome), which is the part that teaches what a chain did.
- `bundled`: one small OFL font shipped in `examples/font/fonts/` (under 500 KB; a Latin + kana subset of Noto Sans JP made with `pyftsubset` and committed, with `OFL.txt` next to it). It is also the `default_proportional`, so the whole example UI is in it.
- `web`: `Url("fonts/NotoSansJP-Regular.otf")` served same-origin by trunk (`<link data-trunk rel="copy-file" ..>`; the file is not committed, `Trunk.toml` has a pre-build hook that downloads it, and the native binary reads the same relative path from the example directory). Shows the Pending → Loaded transition live.
- `system`: `["Hiragino Sans", "Yu Gothic UI", "Noto Sans CJK JP", sans-serif]`. Native: through `load_system_fonts`. wasm: a "use my fonts" button, disabled with a one-line reason on browsers without the API, which calls `request_local_fonts` and then shows the same chain resolved from the grant.
- `META` with `hooks: ["use_state"]`, `elements: ["View", "Text", "Button"]`; added to `EXAMPLES` in the gallery and to the README table. `theme` may get its Japanese locale back in a follow-up now that the gallery has a CJK font; not in this task.
- kittest: the example renders and the report table lists the three stacks.

As built (2026-09-07), where it departs from the above:

- The `Fonts` is a `LazyLock` static (`font::fonts()`), applied from `setup` in `main.rs` and once more from a `use_effect` on `App`'s first frame, because the gallery runs `App` in its own runner with no per-example `setup`. The second `apply` is a no-op when the first happened (the definitions are equal). On the frame that applies, the names are not registered yet, so the samples use `"proportional"` for that one frame rather than trigger `<Text font>`'s warning.
- Natively the `Url` entry is `Failed` (a relative URL has no base for ureq), not read from the example directory; the report shows it, which is the honest answer for a source that is about HTTP. The web font is the static `NotoSansJP-Regular.otf` from notofonts/noto-cjk (`Sans/SubsetOTF/JP`, 4.5 MB, downloaded by `fonts/fetch-web-font.sh` from the trunk `pre_build` hooks of the example and of the gallery, gitignored). The variable `NotoSansJP[wght].ttf` from google/fonts was tried first and rejected: its default instance is Thin (`fvar` default `wght` 100) and epaint draws a variable font's default instance, so the `web` stack came out visibly lighter than the other two.
- `system` also names `"Noto Sans JP"`, which matches the bundled subset already in the database, by design of `System` (any face the database has).
- The Local Font Access click uses `egui_react::spawn` (`spawn_local` on wasm, a thread on native) and reports back through a `Dispatch`, so the example has no `cfg` block, only a `cfg!` for the button's note.
- One kittest, not several: the static is per process and `apply` calls `set_fonts` only when the definitions changed, so a second `Context` in the same process would not receive them. It also asserts `has_glyphs("日本語")` on the `bundled` family, which works here because the subset has neither U+FFFD nor `◻` and the replacement glyph therefore comes from a later face (see the step 1 note on `has_glyphs`).
- The Chromium round trip (permission prompt, `system` resolved from the grant) was not run in this environment; it is described in the module doc and remains a manual check.

### Step 7: docs

- ARCHITECTURE.md section 8: one bullet "Fonts", stating that text is rasterized by epaint from bytes, that the runner's `fonts` module resolves CSS-like chains into `FontDefinitions` through one `fontdb::Database`, which sources exist per target, and the two panics the design guards against. Section 7's crate list gets `fonts` after `run(..)`.
- README: the row for `examples/font`, and a sentence under the wasm build notes that bundled fonts add to the wasm size.
- This document: section 5 updated with the emoji answer and anything the steps changed.

## 4. API in one screen (what the app writes)

```rust
use egui_react_app::fonts::{Fonts, FontSource, Generic};

fn main() -> eframe::Result {
    let fonts = Fonts::new()
        .stack("jp", [
            FontSource::System("Hiragino Sans".into()),
            FontSource::System("Yu Gothic UI".into()),
            FontSource::Url("fonts/NotoSansJP-Regular.otf".into()),
            FontSource::Bundled(include_bytes!("../fonts/kana.ttf")),
            FontSource::Generic(Generic::SansSerif),
        ])
        .stack("code", [FontSource::System("JetBrains Mono".into()), FontSource::Generic(Generic::Monospace)])
        .default_proportional("jp")
        .default_monospace("code");

    let setup_fonts = fonts.clone();
    egui_react_app::run(
        Options {
            setup: Some(Box::new(move |cc| setup_fonts.apply(&cc.egui_ctx))),
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

## 5. Risks and open points

- **Emoji / color fonts.** Decided in step 3: epaint 0.36 draws outlines only. `FontFace::new` (`epaint/src/text/font.rs`) keeps skrifa's `charmap()` and `outline_glyphs()` and nothing else, `allocate_glyph_uncached` returns `None` when `outline_glyphs.get(id)` has nothing, and `has_glyph` asks the charmap alone, so a bitmap-only emoji font (Apple Color Emoji is `sbix`, Noto Color Emoji is `CBDT`, both with an empty or absent `glyf`) claims every emoji and draws nothing for it, and the chain never falls through to NotoEmoji. `resolve` therefore rejects a face with no outline table or with a `CBDT` / `sbix` table as `Invalid` (COLR fonts pass: their base glyphs are outlines and draw in one colour). It is one filter in `resolve::check`, to be removed when the renderer changes.
- **Generic names on macOS / Windows.** fontdb's defaults ("Arial", "Times New Roman", "Courier New") are not CJK-aware. Named fonts before the generic is the documented answer; a per-platform table of better generics is a later task.
- **Font size on wasm.** A CJK font is 5 to 10 MB. The `Url` source with same-origin hosting keeps it out of the wasm; the docs recommend subsetting for `Bundled`. Nothing in the code prevents an app from bundling 10 MB; that is the app's call.
- **`set_fonts` cost.** Each arriving font rebuilds the atlas and every galley once. Two or three arrivals at startup are fine; a chain of ten URLs would flicker ten times. The `Url` source could batch: wait for all URLs in a chain before the first re-apply. Not built until someone needs it; noted in the module docs.
- **`load_system_fonts` time.** fontdb reads name tables of every installed font on the first `apply`; on a machine with thousands of fonts this is a few hundred milliseconds inside `setup`, before the first frame. Acceptable; if it shows, the load moves to a thread and the first `apply` runs without system fonts, re-applying when the scan lands (the same path a URL completion takes).
- **`<Text font>` lookup per pass.** Not measured; the list-10k re-run was skipped in step 3. The check is one `BTreeMap::contains_key` under the fonts lock per `<Text font>` per pass, a handful of string compares against the few registered families and far below the galley it precedes, and a per-pass cache would need invalidating whenever a source arrives and `set_fonts` swaps the definitions. The reasoning is in the comment on `font_family` in `egui-react-elements/src/view.rs`; if a profile ever shows it, the per-pass cache is the fallback.
- **Local Font Access blobs are whole files.** A user with Hiragino installed grants access and the app pulls a 40 MB `.ttc` into wasm memory. Acceptable once; the database keeps it for the session and never re-reads. The docs say to list the specific families wanted, never all.
- **Found on the first web run (2026-09-07), both fixed.** (1) Every label that did not change its text came out as fragments of other glyphs once the web font arrived. The engine caches a `<Text>`'s galley across frames keyed by wrap width and pixels per point; a galley carries texture coordinates into the atlas of the `Fonts` that laid it out, and `set_fonts` builds a new `Fonts` with a new atlas, so the cached galley painted the wrong pixels (Japanese looked right only because those texts had changed family and been laid out again). `Store::note_fonts` now fingerprints the definitions once per pass from the root `Cx`, publishes a generation in egui's data, and the galley cache key carries it (`crates/egui-react/tests/fonts_change.rs`). This affected any app calling `set_fonts` after the first frame, not only this module. (2) The compiled-in subset and the full font fetched over HTTP both declare `NotoSansJP-Regular`, and the resolver keyed faces by PostScript name and index, so the web font was never registered and the `web` stack drew the subset. Faces that share a name but not their bytes now get `~2`, `~3`, .. keys; the same bytes reached twice still share one copy.
- **Family names are exact.** fontdb compares the name-table string byte for byte. `"Hiragino Sans"` and `"ヒラギノ角ゴシック"` both match because both are in the table; `"hiragino sans"` does not. The report shows the family a face declared, which is how a user finds the right spelling.

## 6. Out of scope, confirmed while researching

- Changing epaint's fallback semantics (they are already what CSS does).
- Reading fonts from the browser's own font engine (`document.fonts`): the bytes are not exposed.
- Native OS "generic" mapping better than fontdb's constants: needs per-platform tables (fontconfig does it on Linux; macOS would need `CTFontCreateUIFontForLanguage`, Windows `IDWriteFontFallback`); a later task.
