//! Fonts: CSS-style fallback chains, resolved into egui's font definitions.
//!
//! epaint rasterizes text itself, from font bytes, so the browser's fonts and
//! the OS's font matching are never involved: a font an app wants has to be
//! handed to `Context::set_fonts` as bytes. egui's own four fonts have no CJK
//! glyphs, which is why Japanese draws as boxes in a plain egui app.
//!
//! What epaint *does* have is the fallback chain itself: for each family it
//! walks a list of fonts and takes the first one that has the glyph, exactly
//! what CSS `font-family` does per character. So the work here is producing
//! the right lists, and that is what [`Fonts`] does:
//!
//! - A [`FontStack`] is a named chain of [`FontSource`]s, in priority order:
//!   bytes compiled into the binary, a font installed on the device by family
//!   name, a URL, or a CSS generic such as `sans-serif`.
//! - One `fontdb::Database` holds every face the sources produced. On native
//!   it also holds the installed fonts; on wasm, which cannot read files, it
//!   holds whatever the bundled, fetched and locally granted bytes provided.
//!   Matching is fontdb's, a port of font-kit's CSS Fonts Level 3 algorithm.
//! - [`Fonts::apply`] turns every stack into a `FontFamily::Name(stack)` list,
//!   optionally makes one the default `Proportional` / `Monospace` family so
//!   plain widgets pick it up too, and calls `set_fonts`. Sources that arrive
//!   later (a fetch, a Local Font Access grant) load their bytes into the same
//!   database and apply again.
//!
//! Two epaint panics are designed around. A `FontFamily::Name` that is not in
//! the definitions panics at layout, so `<Text font="..">` checks the name
//! against the definitions and falls back (see `egui_react_elements`). Bytes
//! skrifa cannot parse panic inside `Fonts::new`, which on wasm is a dead
//! canvas, so the resolver runs epaint's own `skrifa::FontRef::from_index`
//! check on every face and reports a failure instead of passing it on.
//!
//! ```ignore
//! let fonts = Fonts::new()
//!     .stack("jp", [
//!         FontSource::System("Hiragino Sans".into()),
//!         FontSource::System("Yu Gothic UI".into()),
//!         FontSource::Url("fonts/NotoSansJP-Regular.ttf".into()),
//!         FontSource::Generic(Generic::SansSerif),
//!     ])
//!     .default_proportional("jp");
//! let setup_fonts = fonts.clone();
//! egui_react_app::run(
//!     Options { setup: Some(Box::new(move |cc| setup_fonts.apply(&cc.egui_ctx))), ..Default::default() },
//!     |_cx| rsx! { <App/> },
//! )
//! ```
//!
//! The chain a URL or a system name resolves to is only known at run time, so
//! [`Fonts::report`] says what each entry became; the `font` example draws it
//! as a table, and the tests assert on it instead of on pixels.
//!
//! Every arrival calls `set_fonts`, which rebuilds the glyph atlas once. Two
//! or three arrivals at startup are fine; a chain of ten URLs would flicker
//! ten times. Batching the URLs of a chain is not built until someone needs
//! it.

mod local;
mod resolve;
mod url;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

pub use local::{LocalFontsError, LocalFontsPermission};
use resolve::{Input, Loaded, SourceKey};

/// One of the five CSS generic families.
///
/// fontdb answers a generic with one configured family name: fontconfig's
/// `<alias>` entries on Linux, "Arial" / "Times New Roman" / "Courier New"
/// and friends elsewhere. Whatever it answers, the resolver puts egui's own
/// font for that generic behind it (Ubuntu-Light, or Hack for `Monospace`),
/// so a generic never resolves to nothing. On macOS and Windows these
/// defaults are not CJK-aware: name the CJK fonts before the generic, as CSS
/// is written in practice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Generic {
    SansSerif,
    Serif,
    Monospace,
    Cursive,
    Fantasy,
}

impl Generic {
    /// The fontdb family that stands for this generic.
    pub(crate) fn family(self) -> fontdb::Family<'static> {
        match self {
            Self::SansSerif => fontdb::Family::SansSerif,
            Self::Serif => fontdb::Family::Serif,
            Self::Monospace => fontdb::Family::Monospace,
            Self::Cursive => fontdb::Family::Cursive,
            Self::Fantasy => fontdb::Family::Fantasy,
        }
    }

    /// The egui built-in font that always stands behind this generic.
    ///
    /// These are the keys `FontDefinitions::default()` registers, so they are
    /// present on every target without loading anything.
    pub(crate) fn builtin(self) -> &'static str {
        match self {
            Self::Monospace => "Hack",
            _ => "Ubuntu-Light",
        }
    }

    /// The CSS spelling.
    pub fn css_name(self) -> &'static str {
        match self {
            Self::SansSerif => "sans-serif",
            Self::Serif => "serif",
            Self::Monospace => "monospace",
            Self::Cursive => "cursive",
            Self::Fantasy => "fantasy",
        }
    }

    /// Parse the CSS spelling, case-insensitively as CSS does.
    pub fn from_css(name: &str) -> Option<Self> {
        let name = name.trim();
        [
            Self::SansSerif,
            Self::Serif,
            Self::Monospace,
            Self::Cursive,
            Self::Fantasy,
        ]
        .into_iter()
        .find(|g| g.css_name().eq_ignore_ascii_case(name))
    }
}

/// Where the bytes of one chain entry come from.
#[derive(Clone, PartialEq, Eq)]
pub enum FontSource {
    /// Bytes compiled into the binary (`include_bytes!`). Loaded into the
    /// database on [`Fonts::apply`]; the chain entry is the best face among
    /// the ones this blob had, so a `.ttc` works as well as a `.ttf`. Works
    /// identically on native and wasm; the only cost is binary size, so
    /// subset a CJK font before bundling it.
    Bundled(&'static [u8]),
    /// A font installed on the device, by family name exactly as the font
    /// declares it (any language in its name table works, so both
    /// `"Hiragino Sans"` and `"ヒラギノ角ゴシック"` match; `"hiragino sans"`
    /// does not). Native: whatever `load_system_fonts` found. wasm: what a
    /// Local Font Access grant or another source already put in the database.
    /// Otherwise the entry is `Missing` and skipped.
    System(String),
    /// Fetched over HTTP (TTF / OTF / TTC; WOFF / WOFF2 with the `woff2`
    /// feature). `Pending` until the bytes arrive, then the chain is applied
    /// again. A same-origin relative URL is the normal case on wasm; a
    /// cross-origin one needs `Access-Control-Allow-Origin`.
    Url(String),
    /// A CSS generic: fontdb's answer, then egui's bundled font behind it.
    Generic(Generic),
}

impl fmt::Debug for FontSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The bytes themselves would flood a test failure message.
        match self {
            Self::Bundled(bytes) => write!(f, "Bundled(<{} bytes>)", bytes.len()),
            Self::System(name) => f.debug_tuple("System").field(name).finish(),
            Self::Url(url) => f.debug_tuple("Url").field(url).finish(),
            Self::Generic(g) => f.debug_tuple("Generic").field(g).finish(),
        }
    }
}

impl fmt::Display for FontSource {
    /// The CSS-like spelling, for a report table.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bundled(bytes) => write!(f, "bundled ({} KB)", bytes.len() / 1024),
            Self::System(name) => write!(f, "\"{name}\""),
            Self::Url(url) => write!(f, "url({url})"),
            Self::Generic(g) => f.write_str(g.css_name()),
        }
    }
}

/// A named chain: what `<Text font="name">` and `RichText::family` select.
#[derive(Clone, Debug)]
pub struct FontStack {
    /// The `FontFamily::Name` this chain is registered under.
    pub name: Arc<str>,
    /// The sources, first is primary.
    pub chain: Vec<FontSource>,
    /// The weight every entry is matched with (`NORMAL` by default). This
    /// picks a face inside a family; it is not a `bold` for the text.
    pub weight: fontdb::Weight,
    /// The style every entry is matched with (`Normal` by default).
    pub style: fontdb::Style,
}

impl FontStack {
    /// A regular-weight, upright chain.
    pub fn new(name: impl Into<Arc<str>>, chain: impl IntoIterator<Item = FontSource>) -> Self {
        Self {
            name: name.into(),
            chain: chain.into_iter().collect(),
            weight: fontdb::Weight::NORMAL,
            style: fontdb::Style::Normal,
        }
    }

    /// Match every entry with this weight.
    pub fn weight(mut self, weight: fontdb::Weight) -> Self {
        self.weight = weight;
        self
    }

    /// Match every entry with this style.
    pub fn style(mut self, style: fontdb::Style) -> Self {
        self.style = style;
        self
    }
}

/// What one chain entry became on the last [`Fonts::apply`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A face was registered with egui under `key`; `family` is the family
    /// name the face declares, which is how to find the right spelling for a
    /// `System` entry.
    Loaded { key: String, family: String },
    /// A `Url` whose bytes have not arrived yet.
    Pending,
    /// A `System` name no face in the database has.
    Missing,
    /// A face was found but rejected: skrifa cannot parse it, or epaint could
    /// not draw it. The message says which.
    Invalid(String),
    /// A `Url` fetch or decode failed, with the reason.
    Failed(String),
}

/// One stack's entries and what each became.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackReport {
    pub name: Arc<str>,
    pub entries: Vec<(FontSource, Outcome)>,
}

/// Why a `css(..)` spec could not be parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssError {
    /// The offending entry, as written.
    pub entry: String,
    pub reason: &'static str,
}

impl fmt::Display for CssError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.reason, self.entry)
    }
}

impl std::error::Error for CssError {}

/// Parse a CSS `font-family`-like list into sources.
///
/// Quoted or bare names become [`FontSource::System`] (bare names have their
/// inner whitespace collapsed, as CSS does), `url(..)` with or without quotes
/// becomes [`FontSource::Url`], and the five generics become
/// [`FontSource::Generic`]. Bundled bytes have no string form.
///
/// ```
/// use egui_react_app::fonts::{FontSource, Generic, parse_css};
/// let chain = parse_css(r#""Hiragino Sans", url(fonts/NotoSansJP-Regular.ttf), sans-serif"#).unwrap();
/// assert_eq!(chain[0], FontSource::System("Hiragino Sans".into()));
/// assert_eq!(chain[1], FontSource::Url("fonts/NotoSansJP-Regular.ttf".into()));
/// assert_eq!(chain[2], FontSource::Generic(Generic::SansSerif));
/// ```
pub fn parse_css(spec: &str) -> Result<Vec<FontSource>, CssError> {
    let mut sources = Vec::new();
    for entry in split_entries(spec) {
        let entry = entry.trim();
        let error = |reason| CssError {
            entry: entry.to_owned(),
            reason,
        };
        if entry.is_empty() {
            return Err(error("empty entry"));
        }
        if let Some(rest) = entry.strip_prefix("url(") {
            let Some(inner) = rest.strip_suffix(')') else {
                return Err(error("unterminated url("));
            };
            let url = unquote(inner.trim()).ok_or_else(|| error("unterminated quote"))?;
            if url.is_empty() {
                return Err(error("empty url()"));
            }
            sources.push(FontSource::Url(url.to_owned()));
        } else if entry.starts_with('"') || entry.starts_with('\'') {
            let name = unquote(entry).ok_or_else(|| error("unterminated quote"))?;
            sources.push(FontSource::System(name.to_owned()));
        } else if let Some(generic) = Generic::from_css(entry) {
            sources.push(FontSource::Generic(generic));
        } else {
            let name = entry.split_whitespace().collect::<Vec<_>>().join(" ");
            sources.push(FontSource::System(name));
        }
    }
    Ok(sources)
}

/// Split on the commas that are outside quotes and parentheses.
fn split_entries(spec: &str) -> Vec<&str> {
    let mut entries = Vec::new();
    let mut start = 0;
    let mut quote: Option<char> = None;
    let mut depth = 0usize;
    for (i, c) in spec.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => depth = depth.saturating_sub(1),
            (None, ',') if depth == 0 => {
                entries.push(&spec[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    entries.push(&spec[start..]);
    entries
}

/// Strip one pair of matching quotes, if any. `None` for an unbalanced pair.
fn unquote(s: &str) -> Option<&str> {
    match s.as_bytes() {
        [q @ (b'"' | b'\''), .., last] if last == q => Some(&s[1..s.len() - 1]),
        [b'"' | b'\''] | [b'"' | b'\'', ..] => None,
        _ => Some(s),
    }
}

/// The chains of an app, the database behind them, and what they resolved to.
///
/// It is a handle (`Clone` is a reference count), so the app can hand one
/// copy to [`crate::Options::setup`] and keep another in a `use_context` or a
/// static for [`Fonts::report`] and [`Fonts::request_local_fonts`]. Every
/// method locks the same mutex, and a fetch completing on another thread
/// takes the same lock, which is why the state is not thread-local.
#[derive(Clone, Default)]
pub struct Fonts(Arc<Mutex<Inner>>);

#[derive(Default)]
struct Inner {
    db: fontdb::Database,
    /// `load_system_fonts` is a scan of every installed font's name table;
    /// once per process is enough.
    system_loaded: bool,
    stacks: Vec<FontStack>,
    default_proportional: Option<Arc<str>>,
    default_monospace: Option<Arc<str>>,
    /// Which faces each `Bundled` / `Url` produced, so a chain entry can be
    /// matched among them, and the state of every URL seen so far.
    loaded: HashMap<SourceKey, Loaded>,
    /// One `Arc<FontData>` per registered face, shared across applies and
    /// across stacks that resolve to the same file, so a re-apply copies no
    /// bytes and `set_fonts`'s equality check sees the same data.
    font_data: HashMap<String, Arc<egui::FontData>>,
    report: Vec<StackReport>,
    /// The definitions the last apply produced, to count real changes.
    last: Option<egui::FontDefinitions>,
    generation: u64,
    /// Set by `apply`, used by completions to apply again and repaint.
    ctx: Option<egui::Context>,
}

impl Fonts {
    /// No stacks yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a regular-weight, upright chain under `name`.
    pub fn stack(
        self,
        name: impl Into<Arc<str>>,
        chain: impl IntoIterator<Item = FontSource>,
    ) -> Self {
        self.add_stack(FontStack::new(name, chain))
    }

    /// Add a chain with its own weight and style.
    pub fn add_stack(self, stack: FontStack) -> Self {
        self.lock().stacks.push(stack);
        self
    }

    /// Add a chain written as CSS: `Fonts::new().css("jp", r#""Hiragino
    /// Sans", url(fonts/NotoSansJP-Regular.ttf), sans-serif"#)`.
    ///
    /// See [`parse_css`]. A spec that does not parse registers `name` with an
    /// empty chain (so `<Text font=name>` still draws, with egui's defaults)
    /// and logs the error; use `parse_css` directly to get it as a value.
    pub fn css(self, name: impl Into<Arc<str>>, spec: &str) -> Self {
        let name = name.into();
        let chain = match parse_css(spec) {
            Ok(chain) => chain,
            Err(err) => {
                log::warn!("egui-react fonts: stack {name:?}: {err}");
                Vec::new()
            }
        };
        self.stack(name, chain)
    }

    /// Make `name`'s list the `FontFamily::Proportional` list too, so every
    /// widget and every plain `<Text>` draws with it.
    pub fn default_proportional(self, name: impl Into<Arc<str>>) -> Self {
        self.lock().default_proportional = Some(name.into());
        self
    }

    /// Make `name`'s list the `FontFamily::Monospace` list too.
    pub fn default_monospace(self, name: impl Into<Arc<str>>) -> Self {
        self.lock().default_monospace = Some(name.into());
        self
    }

    /// Resolve every stack and hand the result to `ctx.set_fonts`.
    ///
    /// Call it once, from [`crate::Options::setup`]; sources that arrive
    /// later call it again by themselves. It is not a per-frame call:
    /// `set_fonts` compares the font bytes to decide whether anything
    /// changed. The first call loads the installed fonts on native, which
    /// takes tens to a few hundred milliseconds on a machine with many fonts.
    ///
    /// The new fonts take effect at the start of the next pass.
    pub fn apply(&self, ctx: &egui::Context) {
        let urls = {
            let mut inner = self.lock();
            inner.ctx = Some(ctx.clone());
            let urls = inner.prepare();
            inner.reapply();
            urls
        };
        // Outside the lock: a fetch may complete on another thread and take
        // it, and on wasm the callback would find the mutex free anyway, but
        // there is no reason to hold it while a request is set up.
        for url in urls {
            self.start_fetch(url);
        }
    }

    /// What each entry of each stack became on the last apply.
    pub fn report(&self) -> Vec<StackReport> {
        self.lock().report.clone()
    }

    /// How many applies produced definitions different from the previous
    /// ones. Zero before the first apply; one after it; one more per source
    /// that arrived and changed a chain. For tests and status displays.
    pub fn generation(&self) -> u64 {
        self.lock().generation
    }

    /// Whether any `Url` source is still being fetched. False before the
    /// first apply (nothing has been asked for) and once every URL has
    /// arrived or failed. For a loading screen: draw a placeholder while this
    /// is true, or nothing special and let the chains draw with what stands
    /// behind the URL (the `font-display: swap` way).
    pub fn pending(&self) -> bool {
        let inner = self.lock();
        inner.loaded.values().any(|l| matches!(l, Loaded::Pending))
    }

    /// Load font bytes obtained some other way (a file picker, an app's own
    /// loader) into the database, and apply again if a context is known.
    ///
    /// Returns how many faces the bytes had; zero means fontdb could not parse
    /// them. The faces are reachable through `System(family)` entries, exactly
    /// like an installed font.
    pub fn load_font_data(&self, bytes: Vec<u8>) -> usize {
        let mut inner = self.lock();
        let faces = inner
            .db
            .load_font_source(fontdb::Source::Binary(Arc::new(bytes)))
            .len();
        if faces > 0 {
            inner.reapply();
            inner.repaint();
        }
        faces
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A panic while holding the lock leaves the data consistent enough
        // to keep drawing with: nothing here is half-written across a call.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Inner {
    /// First-time work: the installed fonts, the bundled blobs, and the URLs
    /// that have never been asked for (returned, marked pending; the caller
    /// starts them outside the lock).
    fn prepare(&mut self) -> Vec<String> {
        if !self.system_loaded {
            self.system_loaded = true;
            #[cfg(not(target_arch = "wasm32"))]
            self.db.load_system_fonts();
        }
        let mut urls = Vec::new();
        for stack in &self.stacks {
            for source in &stack.chain {
                match source {
                    FontSource::Bundled(bytes) => {
                        if let Entry::Vacant(slot) = self.loaded.entry(SourceKey::bundled(bytes)) {
                            slot.insert(resolve::load_bundled(&mut self.db, bytes));
                        }
                    }
                    FontSource::Url(url) => {
                        if let Entry::Vacant(slot) = self.loaded.entry(SourceKey::Url(url.clone()))
                        {
                            slot.insert(Loaded::Pending);
                            urls.push(url.clone());
                        }
                    }
                    FontSource::System(_) | FontSource::Generic(_) => {}
                }
            }
        }
        urls
    }

    /// Resolve and `set_fonts` if a context is known. Counts a generation
    /// only when the definitions actually changed, which is also the only
    /// case in which egui rebuilds its atlas.
    fn reapply(&mut self) {
        let output = resolve::resolve(
            Input {
                db: &self.db,
                stacks: &self.stacks,
                loaded: &self.loaded,
                default_proportional: self.default_proportional.as_deref(),
                default_monospace: self.default_monospace.as_deref(),
            },
            &mut self.font_data,
        );
        self.report = output.report;
        if self.last.as_ref() != Some(&output.definitions) {
            self.generation += 1;
            if let Some(ctx) = &self.ctx {
                ctx.set_fonts(output.definitions.clone());
            }
            self.last = Some(output.definitions);
        }
    }

    /// A source arrived between frames: egui draws on demand, so ask for one.
    fn repaint(&self) {
        if let Some(ctx) = &self.ctx {
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_parses_names_urls_and_generics() {
        let chain = parse_css(
            r#""Hiragino Sans", 'Yu Gothic UI', url(fonts/NotoSansJP-Regular.ttf), url("a, b.ttf"), Sans-Serif, Noto  Sans CJK JP"#,
        )
        .unwrap();
        assert_eq!(
            chain,
            vec![
                FontSource::System("Hiragino Sans".into()),
                FontSource::System("Yu Gothic UI".into()),
                FontSource::Url("fonts/NotoSansJP-Regular.ttf".into()),
                FontSource::Url("a, b.ttf".into()),
                FontSource::Generic(Generic::SansSerif),
                FontSource::System("Noto Sans CJK JP".into()),
            ]
        );
    }

    #[test]
    fn css_rejects_unbalanced_input() {
        assert!(parse_css(r#""Hiragino Sans"#).is_err());
        assert!(parse_css("url(fonts/x.ttf").is_err());
        assert!(parse_css("a,,b").is_err());
        assert!(parse_css("").is_err());
    }

    #[test]
    fn css_builder_registers_the_name_even_when_the_spec_is_bad() {
        let fonts = Fonts::new().css("bad", r#""unterminated"#);
        let inner = fonts.lock();
        assert_eq!(&*inner.stacks[0].name, "bad");
        assert!(inner.stacks[0].chain.is_empty());
    }
}
