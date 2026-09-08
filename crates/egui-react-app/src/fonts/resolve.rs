//! The resolver: stacks plus a database in, `FontDefinitions` plus a report
//! out. Pure, so the unit tests build a database from bytes and never touch
//! a `Context`.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

use super::{FontSource, FontStack, Outcome, StackReport};

/// Identity of a `Bundled` or `Url` source, for the table of what each one
/// loaded. Bundled bytes are keyed by address: hashing megabytes on every
/// lookup would be pointless, and two `include_bytes!` of the same file are
/// the same static anyway.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum SourceKey {
    Bundled { ptr: usize, len: usize },
    Url(String),
}

impl SourceKey {
    pub(super) fn bundled(bytes: &'static [u8]) -> Self {
        Self::Bundled {
            ptr: bytes.as_ptr() as usize,
            len: bytes.len(),
        }
    }
}

/// What a `Bundled` or `Url` source has produced so far.
#[derive(Clone, Debug)]
pub(super) enum Loaded {
    /// The fetch has been started and has not answered.
    Pending,
    /// The fetch or the decode failed; the message goes to the report.
    Failed(String),
    /// The faces fontdb found in the bytes (empty when it could parse none).
    /// `static_bytes` is the blob itself when it is compiled in, so that the
    /// `FontData` can borrow it instead of copying a font that already sits
    /// in the binary.
    Faces {
        ids: Vec<fontdb::ID>,
        static_bytes: Option<&'static [u8]>,
    },
}

/// Put a compiled-in blob into the database.
pub(super) fn load_bundled(db: &mut fontdb::Database, bytes: &'static [u8]) -> Loaded {
    let ids = db.load_font_source(fontdb::Source::Binary(Arc::new(bytes)));
    Loaded::Faces {
        ids: ids.to_vec(),
        static_bytes: Some(bytes),
    }
}

/// Everything one resolution reads.
pub(super) struct Input<'a> {
    /// What to build on: egui's own definitions, normally
    /// `FontDefinitions::default()`. With egui's `default_fonts` feature off
    /// that *is* `FontDefinitions::empty()` — no `font_data` at all, and the
    /// two default families bound to empty lists — so nothing below may assume
    /// a built-in key exists.
    pub base: FontDefinitions,
    pub db: &'a fontdb::Database,
    pub stacks: &'a [FontStack],
    pub loaded: &'a HashMap<SourceKey, Loaded>,
    pub default_proportional: Option<&'a str>,
    pub default_monospace: Option<&'a str>,
}

pub(super) struct Output {
    pub definitions: FontDefinitions,
    pub report: Vec<StackReport>,
}

/// The six steps of the design: start from egui's defaults, turn every stack
/// entry into zero or one font key, append egui's own list as the tail,
/// register the stack under its name, and let a default stack replace
/// `Proportional` / `Monospace`.
///
/// `font_data` is the cache of faces already registered; a face is copied out
/// of the database and checked only the first time it is seen.
///
/// Nothing here writes a key into a family unless `defs.font_data` has bytes
/// under it, and every family the input names ends up bound, empty or not:
/// those are the two things epaint panics on. So with egui's `default_fonts`
/// off (`Input::base` empty) a chain that resolved to nothing is an empty
/// family, which lays text out with zero glyphs instead of panicking.
pub(super) fn resolve(input: Input<'_>, font_data: &mut HashMap<String, Arc<FontData>>) -> Output {
    let Input {
        base,
        db,
        stacks,
        loaded,
        default_proportional,
        default_monospace,
    } = input;
    let mut defs = base;
    // The tails, taken before a default stack rewrites either family: the
    // two emoji fonts at the end of these are what draw icons in every chain.
    // Both are empty when egui's fonts are not in the build.
    let builtin_proportional = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let builtin_monospace = defs
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();

    let mut report = Vec::with_capacity(stacks.len());
    let mut lists: HashMap<Arc<str>, Vec<String>> = HashMap::new();

    for stack in stacks {
        let mut keys: Vec<String> = Vec::new();
        let mut entries = Vec::with_capacity(stack.chain.len());
        let mut push = |keys: &mut Vec<String>, key: String| {
            if !keys.contains(&key) {
                keys.push(key);
            }
        };

        for source in &stack.chain {
            let outcome = match source {
                FontSource::Bundled(bytes) => {
                    let loaded = loaded.get(&SourceKey::bundled(bytes));
                    resolve_blob(
                        db, stack, loaded, &mut defs, font_data, &mut keys, &mut push,
                    )
                }
                FontSource::Url(url) => {
                    let loaded = loaded.get(&SourceKey::Url(url.clone()));
                    resolve_blob(
                        db, stack, loaded, &mut defs, font_data, &mut keys, &mut push,
                    )
                }
                FontSource::System(name) => {
                    let query = fontdb::Query {
                        families: &[fontdb::Family::Name(name)],
                        weight: stack.weight,
                        style: stack.style,
                        stretch: fontdb::Stretch::Normal,
                    };
                    match db.query(&query) {
                        None => Outcome::Missing,
                        Some(id) => match register(db, id, None, &mut defs, font_data) {
                            Ok(face) => {
                                push(&mut keys, face.key.clone());
                                face.into_outcome()
                            }
                            Err(reason) => {
                                log::warn!("egui-react fonts: {source}: {reason}");
                                Outcome::Invalid(reason)
                            }
                        },
                    }
                }
                FontSource::Generic(generic) => {
                    let query = fontdb::Query {
                        families: &[generic.family()],
                        weight: stack.weight,
                        style: stack.style,
                        stretch: fontdb::Stretch::Normal,
                    };
                    // The device's answer first, when it has one and epaint
                    // can use it; egui's own font behind it, so the generic
                    // draws something even on a device with no answer.
                    let mut first: Option<Face> = None;
                    if let Some(id) = db.query(&query) {
                        match register(db, id, None, &mut defs, font_data) {
                            Ok(face) => {
                                push(&mut keys, face.key.clone());
                                first = Some(face);
                            }
                            Err(reason) => {
                                log::warn!("egui-react fonts: {source}: {reason}");
                            }
                        }
                    }
                    // Unless egui's fonts are not in the build: then there is
                    // no floor under the generic, and it contributes nothing.
                    let builtin = builtin_key(&defs, generic.builtin());
                    if let Some(builtin) = builtin {
                        push(&mut keys, builtin.to_owned());
                    }
                    match (first, builtin) {
                        (Some(face), _) => face.into_outcome(),
                        (None, Some(builtin)) => Outcome::Loaded {
                            key: builtin.to_owned(),
                            family: builtin.to_owned(),
                        },
                        (None, None) => Outcome::Missing,
                    }
                }
            };
            entries.push((source.clone(), outcome));
        }

        let tail = if default_monospace == Some(&*stack.name) {
            &builtin_monospace
        } else {
            &builtin_proportional
        };
        for key in tail {
            // The tail is `base`'s own family list, so the bytes are normally
            // right there in `base.font_data`; the check is what keeps a
            // family egui listed a key for without shipping it from reaching
            // epaint, which panics on exactly that.
            if let Some(key) = builtin_key(&defs, key) {
                push(&mut keys, key.to_owned());
            }
        }

        defs.families
            .insert(FontFamily::Name(Arc::clone(&stack.name)), keys.clone());
        lists.insert(Arc::clone(&stack.name), keys);
        report.push(StackReport {
            name: Arc::clone(&stack.name),
            entries,
        });
    }

    for (name, family) in [
        (default_proportional, FontFamily::Proportional),
        (default_monospace, FontFamily::Monospace),
    ] {
        let Some(name) = name else {
            continue;
        };
        match lists.get(name) {
            Some(list) => {
                defs.families.insert(family, list.clone());
            }
            None => log::warn!(
                "egui-react fonts: default {family:?} names stack {name:?}, which does not exist"
            ),
        }
    }

    Output {
        definitions: defs,
        report,
    }
}

/// One of egui's built-in keys, but only when the bytes are there: with
/// egui's `default_fonts` feature off nothing is registered under them, and a
/// family naming a key with no `font_data` is one of epaint's two panics.
fn builtin_key<'a>(defs: &FontDefinitions, key: &'a str) -> Option<&'a str> {
    defs.font_data.contains_key(key).then_some(key)
}

/// A `Bundled` or `Url` entry: pick the best face among the blob's own.
#[allow(clippy::too_many_arguments)]
fn resolve_blob(
    db: &fontdb::Database,
    stack: &FontStack,
    loaded: Option<&Loaded>,
    defs: &mut FontDefinitions,
    font_data: &mut HashMap<String, Arc<FontData>>,
    keys: &mut Vec<String>,
    push: &mut impl FnMut(&mut Vec<String>, String),
) -> Outcome {
    match loaded {
        None | Some(Loaded::Pending) => Outcome::Pending,
        Some(Loaded::Failed(reason)) => Outcome::Failed(reason.clone()),
        Some(Loaded::Faces { ids, static_bytes }) => {
            let Some(id) = best_of(db, ids, stack.weight, stack.style) else {
                return Outcome::Invalid(String::from("no font face could be parsed"));
            };
            match register(db, id, *static_bytes, defs, font_data) {
                Ok(face) => {
                    push(keys, face.key.clone());
                    face.into_outcome()
                }
                Err(reason) => {
                    log::warn!("egui-react fonts: {reason}");
                    Outcome::Invalid(reason)
                }
            }
        }
    }
}

/// fontdb's CSS matching over a subset of faces.
///
/// `Database::query` cannot be told "only these IDs", and its matching
/// function is private, so the subset goes into a database of its own (a
/// `FaceInfo` clone is a few strings and an `Arc`) and the answer is mapped
/// back. This keeps the matching for a bundled `.ttc` the same algorithm the
/// installed fonts get.
fn best_of(
    db: &fontdb::Database,
    ids: &[fontdb::ID],
    weight: fontdb::Weight,
    style: fontdb::Style,
) -> Option<fontdb::ID> {
    match ids {
        [] => return None,
        [only] => return Some(*only),
        _ => {}
    }
    let mut subset = fontdb::Database::new();
    let mut families: Vec<String> = Vec::new();
    let mut back: Vec<(fontdb::ID, fontdb::ID)> = Vec::with_capacity(ids.len());
    for id in ids {
        let Some(info) = db.face(*id) else {
            continue;
        };
        if let Some((family, _)) = info.families.first()
            && !families.contains(family)
        {
            families.push(family.clone());
        }
        back.push((subset.push_face_info(info.clone()), *id));
    }
    let families: Vec<fontdb::Family<'_>> =
        families.iter().map(|f| fontdb::Family::Name(f)).collect();
    let hit = subset.query(&fontdb::Query {
        families: &families,
        weight,
        style,
        stretch: fontdb::Stretch::Normal,
    })?;
    back.iter().find(|(sub, _)| *sub == hit).map(|(_, id)| *id)
}

/// A face registered with egui.
struct Face {
    key: String,
    family: String,
}

impl Face {
    fn into_outcome(self) -> Outcome {
        Outcome::Loaded {
            key: self.key,
            family: self.family,
        }
    }
}

/// Put one face into `defs.font_data` under `"{post_script_name}#{index}"`,
/// copying and checking the bytes only the first time this face is seen.
///
/// The same file reached twice (two stacks naming it, or a system path and a
/// URL serving the same bytes) shares one key and one copy, which is what
/// keeps `set_fonts` from holding the font twice. Two *different* files that
/// declare the same PostScript name do not: a subset of Noto Sans JP compiled
/// in and the full font fetched later both call themselves
/// `NotoSansJP-Regular`, and a chain that names the full one has to get it.
/// The second and later distinct files under a name get `~2`, `~3`, .. after
/// the key. "Same file" is decided by the bytes, compared only when the names
/// collide.
fn register(
    db: &fontdb::Database,
    id: fontdb::ID,
    static_bytes: Option<&'static [u8]>,
    defs: &mut FontDefinitions,
    font_data: &mut HashMap<String, Arc<FontData>>,
) -> Result<Face, String> {
    let info = db
        .face(id)
        .ok_or_else(|| String::from("the face is no longer in the database"))?;
    let base = format!("{}#{}", info.post_script_name, info.index);
    let index = info.index;
    let family = info
        .families
        .first()
        .map(|(name, _)| name.clone())
        .unwrap_or_else(|| info.post_script_name.clone());

    let (key, data) = match static_bytes {
        Some(bytes) => keyed(font_data, &base, bytes, index, || {
            check(bytes, index)?;
            Ok(Cow::Borrowed(bytes))
        })?,
        None => db
            .with_face_data(id, |bytes, index| {
                keyed(font_data, &base, bytes, index, || {
                    // Check before copying, so a rejected face costs no
                    // allocation; the copy is what epaint needs, since
                    // `FontData` owns or borrows `'static`.
                    check(bytes, index)?;
                    Ok(Cow::Owned(bytes.to_vec()))
                })
            })
            .ok_or_else(|| format!("{base}: the font file could not be read"))??,
    };
    defs.font_data.entry(key.clone()).or_insert(data);
    Ok(Face { key, family })
}

/// The key for `bytes` under `base`: the cached entry that holds these very
/// bytes, or the first free `base`, `base~2`, `base~3`, .. filled with what
/// `make` produces (checked and copied only then).
fn keyed(
    font_data: &mut HashMap<String, Arc<FontData>>,
    base: &str,
    bytes: &[u8],
    index: u32,
    make: impl FnOnce() -> Result<Cow<'static, [u8]>, String>,
) -> Result<(String, Arc<FontData>), String> {
    let mut key = base.to_owned();
    let mut n = 1u32;
    loop {
        match font_data.get(&key) {
            Some(data) if data.index == index && same_bytes(&data.font, bytes) => {
                return Ok((key, Arc::clone(data)));
            }
            Some(_) => {
                n += 1;
                key = format!("{base}~{n}");
            }
            None => {
                let font = make().map_err(|e| format!("{key}: {e}"))?;
                let data = Arc::new(FontData {
                    font,
                    index,
                    tweak: Default::default(),
                });
                font_data.insert(key.clone(), Arc::clone(&data));
                return Ok((key, data));
            }
        }
    }
}

/// Pointer equality first: a compiled-in blob is the same static every time,
/// and a file is compared in full only when two different files share a name.
fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    std::ptr::eq(a, b) || a == b
}

/// The check epaint's `Fonts::new` makes, before egui makes it and panics,
/// plus the one thing epaint 0.36 accepts and then cannot draw.
///
/// epaint draws outlines only: `FontFace::new` (`epaint/src/text/font.rs`)
/// keeps skrifa's `charmap()` and `outline_glyphs()` and nothing else,
/// `allocate_glyph_uncached` gives up when `outline_glyphs.get(id)` has
/// nothing, and `has_glyph` asks the charmap alone. A bitmap-only emoji font
/// (Apple Color Emoji is `sbix`, Noto Color Emoji is `CBDT`, both with an
/// empty or absent `glyf`) therefore claims every emoji and draws nothing for
/// it, and the chain never falls through to NotoEmoji. Such a face is
/// rejected here, as `Invalid` with the reason. `COLR` fonts pass: their base
/// glyphs are outlines and draw in one colour. This is the one place that
/// knows what the renderer can draw; it goes when the renderer changes.
fn check(bytes: &[u8], index: u32) -> Result<(), String> {
    use skrifa::MetadataProvider as _;
    let font = skrifa::FontRef::from_index(bytes, index)
        .map_err(|err| format!("skrifa cannot parse it: {err}"))?;
    if font.outline_glyphs().format().is_none() {
        return Err(String::from(
            "no outline glyphs (a bitmap or colour-only font); epaint 0.36 draws outlines only",
        ));
    }
    for tag in [b"CBDT", b"sbix"] {
        if font.table_data(skrifa::Tag::new(tag)).is_some() {
            return Err(format!(
                "bitmap emoji font ({}); epaint 0.36 draws outlines only",
                str::from_utf8(tag).unwrap_or("?")
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use epaint_default_fonts::{HACK_REGULAR, UBUNTU_LIGHT};

    /// Hack is the font "the app bundled" (it goes through `loaded`), Ubuntu
    /// is the font "installed on the device" (it sits in the database under
    /// its family name, as `load_system_fonts` would put it).
    struct Fixture {
        db: fontdb::Database,
        loaded: HashMap<SourceKey, Loaded>,
        font_data: HashMap<String, Arc<FontData>>,
    }

    impl Fixture {
        fn new() -> Self {
            let mut db = fontdb::Database::new();
            db.load_font_source(fontdb::Source::Binary(Arc::new(UBUNTU_LIGHT)));
            let mut loaded = HashMap::new();
            loaded.insert(
                SourceKey::bundled(HACK_REGULAR),
                load_bundled(&mut db, HACK_REGULAR),
            );
            Self {
                db,
                loaded,
                font_data: HashMap::new(),
            }
        }

        fn with_bundled(mut self, bytes: &'static [u8]) -> Self {
            self.loaded
                .insert(SourceKey::bundled(bytes), load_bundled(&mut self.db, bytes));
            self
        }

        fn resolve(&mut self, stacks: &[FontStack]) -> Output {
            self.resolve_with(stacks, None, None)
        }

        fn resolve_with(
            &mut self,
            stacks: &[FontStack],
            default_proportional: Option<&str>,
            default_monospace: Option<&str>,
        ) -> Output {
            self.resolve_base(
                FontDefinitions::default(),
                stacks,
                default_proportional,
                default_monospace,
            )
        }

        /// Resolve against a base of the test's choosing: `empty()` is what
        /// egui's `FontDefinitions::default()` *is* when its `default_fonts`
        /// feature is off, whatever features this test run was built with.
        fn resolve_base(
            &mut self,
            base: FontDefinitions,
            stacks: &[FontStack],
            default_proportional: Option<&str>,
            default_monospace: Option<&str>,
        ) -> Output {
            resolve(
                Input {
                    base,
                    db: &self.db,
                    stacks,
                    loaded: &self.loaded,
                    default_proportional,
                    default_monospace,
                },
                &mut self.font_data,
            )
        }
    }

    fn family(defs: &FontDefinitions, name: &str) -> Vec<String> {
        defs.families[&FontFamily::Name(name.into())].clone()
    }

    /// epaint's two panics, as an assertion: a family that names a font with
    /// no `font_data` ("No font data found for .."), and a family that is used
    /// but not bound ("is not bound to any fonts"). An *empty* family is fine.
    fn assert_epaint_would_accept(defs: &FontDefinitions) {
        for (family, list) in &defs.families {
            for key in list {
                assert!(
                    defs.font_data.contains_key(key),
                    "{family:?} names {key:?}, which has no font data"
                );
            }
        }
        assert!(defs.families.contains_key(&FontFamily::Proportional));
        assert!(defs.families.contains_key(&FontFamily::Monospace));
    }

    const HACK: &str = "Hack-Regular#0";
    const UBUNTU: &str = "Ubuntu-Light#0";

    #[test]
    fn chain_order_is_preserved_and_the_tail_is_eguis() {
        let mut fx = Fixture::new();
        let out = fx.resolve(&[FontStack::new(
            "jp",
            [
                FontSource::Bundled(HACK_REGULAR),
                FontSource::System("Ubuntu".into()),
            ],
        )]);
        let list = family(&out.definitions, "jp");
        assert_eq!(&list[..2], [HACK, UBUNTU]);
        let builtin = FontDefinitions::default().families[&FontFamily::Proportional].clone();
        assert_eq!(&list[2..], &builtin[..]);
        assert_eq!(
            out.report[0].entries[0].1,
            Outcome::Loaded {
                key: HACK.into(),
                family: "Hack".into()
            }
        );
        assert_eq!(
            out.report[0].entries[1].1,
            Outcome::Loaded {
                key: UBUNTU.into(),
                family: "Ubuntu".into()
            }
        );
    }

    #[test]
    fn a_missing_system_font_is_reported_and_skipped() {
        let mut fx = Fixture::new();
        let out = fx.resolve(&[FontStack::new(
            "jp",
            [
                FontSource::System("This Font Does Not Exist".into()),
                FontSource::Bundled(HACK_REGULAR),
            ],
        )]);
        assert_eq!(out.report[0].entries[0].1, Outcome::Missing);
        assert_eq!(family(&out.definitions, "jp")[0], HACK);
    }

    #[test]
    fn a_default_stack_rewrites_the_builtin_family() {
        let mut fx = Fixture::new();
        let stacks = [
            FontStack::new("jp", [FontSource::Bundled(HACK_REGULAR)]),
            FontStack::new("code", [FontSource::System("Ubuntu".into())]),
        ];
        let out = fx.resolve_with(&stacks, Some("jp"), Some("code"));
        assert_eq!(
            out.definitions.families[&FontFamily::Proportional],
            family(&out.definitions, "jp")
        );
        assert_eq!(
            out.definitions.families[&FontFamily::Monospace],
            family(&out.definitions, "code")
        );
        // The monospace default gets egui's monospace tail: Hack first.
        #[cfg(feature = "default_fonts")]
        assert_eq!(family(&out.definitions, "code")[1], "Hack");
        assert_epaint_would_accept(&out.definitions);
    }

    #[test]
    fn duplicates_collapse_to_one_key_and_one_font_data() {
        let mut fx = Fixture::new();
        let stacks = [
            FontStack::new(
                "a",
                [
                    FontSource::Bundled(HACK_REGULAR),
                    FontSource::Bundled(HACK_REGULAR),
                ],
            ),
            FontStack::new("b", [FontSource::Bundled(HACK_REGULAR)]),
        ];
        let out = fx.resolve(&stacks);
        let a = family(&out.definitions, "a");
        assert_eq!(a.iter().filter(|k| *k == HACK).count(), 1);
        assert_eq!(
            out.definitions
                .font_data
                .keys()
                .filter(|k| *k == HACK)
                .count(),
            1
        );
        assert_eq!(fx.font_data.len(), 1);
        // Bundled bytes are borrowed, never copied.
        assert!(matches!(
            out.definitions.font_data[HACK].font,
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn resolving_twice_with_nothing_changed_is_equal() {
        let mut fx = Fixture::new();
        let stacks = [FontStack::new(
            "jp",
            [
                FontSource::Bundled(HACK_REGULAR),
                FontSource::System("Ubuntu".into()),
                FontSource::Generic(super::super::Generic::SansSerif),
            ],
        )];
        let first = fx.resolve(&stacks);
        let second = fx.resolve(&stacks);
        assert_eq!(first.definitions, second.definitions);
        assert!(Arc::ptr_eq(
            &first.definitions.font_data[UBUNTU],
            &second.definitions.font_data[UBUNTU]
        ));
    }

    #[test]
    fn invalid_bytes_are_reported_never_a_panic() {
        static GARBAGE: &[u8] = b"this is not a font file at all, not even close";
        let mut fx = Fixture::new().with_bundled(GARBAGE);
        let out = fx.resolve(&[FontStack::new(
            "jp",
            [
                FontSource::Bundled(GARBAGE),
                FontSource::Bundled(HACK_REGULAR),
            ],
        )]);
        assert!(matches!(out.report[0].entries[0].1, Outcome::Invalid(_)));
        assert_eq!(family(&out.definitions, "jp")[0], HACK);
    }

    #[test]
    fn a_pending_url_is_skipped_and_reported() {
        let mut fx = Fixture::new();
        let out = fx.resolve(&[FontStack::new(
            "web",
            [
                FontSource::Url("fonts/x.ttf".into()),
                FontSource::Bundled(HACK_REGULAR),
            ],
        )]);
        assert_eq!(out.report[0].entries[0].1, Outcome::Pending);
        assert_eq!(family(&out.definitions, "web")[0], HACK);
    }

    #[test]
    fn a_failed_url_is_skipped_and_carries_its_reason() {
        let mut fx = Fixture::new();
        fx.loaded.insert(
            SourceKey::Url("fonts/x.ttf".into()),
            Loaded::Failed(String::from("HTTP 404 Not Found")),
        );
        let out = fx.resolve(&[FontStack::new(
            "web",
            [
                FontSource::Url("fonts/x.ttf".into()),
                FontSource::Bundled(HACK_REGULAR),
            ],
        )]);
        assert_eq!(
            out.report[0].entries[0].1,
            Outcome::Failed("HTTP 404 Not Found".into())
        );
        assert_eq!(family(&out.definitions, "web")[0], HACK);
    }

    /// Only with egui's fonts in the build; the empty-base test below is the
    /// other half of this one.
    #[cfg(feature = "default_fonts")]
    #[test]
    fn generic_monospace_puts_hack_in_even_without_a_monospace_face() {
        let mut fx = Fixture::new();
        fx.db = fontdb::Database::new();
        let out = fx.resolve(&[FontStack::new(
            "code",
            [FontSource::Generic(super::super::Generic::Monospace)],
        )]);
        assert_eq!(family(&out.definitions, "code")[0], "Hack");
        assert_eq!(
            out.report[0].entries[0].1,
            Outcome::Loaded {
                key: "Hack".into(),
                family: "Hack".into()
            }
        );
    }

    /// egui built with `default_fonts` off: `FontDefinitions::default()` is
    /// `empty()`, so there is no floor under a generic and no tail behind a
    /// chain. Nothing may dangle and no family may go unbound, whatever this
    /// test run's own features are.
    #[test]
    fn an_empty_base_resolves_to_empty_families_and_never_a_dangling_key() {
        let mut fx = Fixture::new();
        let stacks = [
            FontStack::new(
                "code",
                [
                    FontSource::Bundled(HACK_REGULAR),
                    FontSource::Generic(super::super::Generic::Monospace),
                ],
            ),
            FontStack::new(
                "nothing",
                [
                    FontSource::System("This Font Does Not Exist".into()),
                    FontSource::Generic(super::super::Generic::SansSerif),
                ],
            ),
        ];
        let out = fx.resolve_base(
            FontDefinitions::empty(),
            &stacks,
            Some("nothing"),
            Some("code"),
        );
        assert_epaint_would_accept(&out.definitions);
        // The bundled face and nothing else: no built-in behind the generic.
        assert_eq!(family(&out.definitions, "code"), [HACK]);
        assert_eq!(out.report[0].entries[1].1, Outcome::Missing);
        // And a chain that found nothing is an empty family, which draws no
        // glyphs. Invisible text, not a panic.
        assert!(family(&out.definitions, "nothing").is_empty());
        assert_eq!(out.report[1].entries[0].1, Outcome::Missing);
        assert_eq!(out.report[1].entries[1].1, Outcome::Missing);
        assert!(out.definitions.families[&FontFamily::Proportional].is_empty());
        assert_eq!(out.definitions.families[&FontFamily::Monospace], [HACK]);
    }

    #[test]
    fn a_stack_with_no_hits_still_registers_its_name() {
        let mut fx = Fixture::new();
        let out = fx.resolve(&[FontStack::new("empty", [FontSource::System("Nope".into())])]);
        assert_eq!(
            family(&out.definitions, "empty"),
            FontDefinitions::default().families[&FontFamily::Proportional]
        );
    }

    /// A subset compiled in and the full font fetched later declare the same
    /// PostScript name. They are different files, so they must not share a
    /// key: the chain that names the second one has to get the second one.
    #[test]
    fn two_files_with_one_postscript_name_get_two_keys() {
        // The same font with a byte appended: fontdb and skrifa read the
        // table directory and never look at the tail, so it is "a different
        // file that calls itself Hack-Regular".
        let mut other = HACK_REGULAR.to_vec();
        other.push(0);
        let other: &'static [u8] = Box::leak(other.into_boxed_slice());
        let mut fx = Fixture::new().with_bundled(other);
        let out = fx.resolve(&[
            FontStack::new("a", [FontSource::Bundled(HACK_REGULAR)]),
            FontStack::new("b", [FontSource::Bundled(other)]),
        ]);
        assert_eq!(family(&out.definitions, "a")[0], HACK);
        assert_eq!(family(&out.definitions, "b")[0], "Hack-Regular#0~2");
        assert_eq!(
            out.definitions.font_data["Hack-Regular#0~2"].font.len(),
            HACK_REGULAR.len() + 1,
            "the second key holds the second file's bytes"
        );
        assert_eq!(fx.font_data.len(), 2);
    }

    /// The installed fonts, on whatever machine runs the tests. A runner with
    /// no fonts is not a failure of this code, so the test says so and passes.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn system_fonts_resolve_or_the_machine_has_none() {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        if db.is_empty() {
            eprintln!("no installed fonts on this machine; skipping the system font check");
            return;
        }
        let mut fx = Fixture::new();
        fx.db = db;
        let out = fx.resolve(&[FontStack::new(
            "sys",
            [
                FontSource::System("This Font Does Not Exist".into()),
                FontSource::Generic(super::super::Generic::SansSerif),
            ],
        )]);
        assert_eq!(out.report[0].entries[0].1, Outcome::Missing);
        let Outcome::Loaded { key, family } = &out.report[0].entries[1].1 else {
            panic!(
                "sans-serif did not resolve: {:?}",
                out.report[0].entries[1].1
            );
        };
        eprintln!("sans-serif resolved to {family:?} ({key})");
    }

    /// A table directory with these tables and nothing else: enough for
    /// skrifa to open, which is all the outline check needs.
    fn sfnt(tables: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        out.extend_from_slice(&(tables.len() as u16).to_be_bytes());
        out.extend_from_slice(&[0u8; 6]);
        let mut offset = 12 + 16 * tables.len() as u32;
        let mut data = Vec::new();
        for (tag, bytes) in tables {
            out.extend_from_slice(*tag);
            out.extend_from_slice(&0u32.to_be_bytes());
            out.extend_from_slice(&offset.to_be_bytes());
            out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            offset += bytes.len() as u32;
            data.extend_from_slice(bytes);
        }
        out.extend_from_slice(&data);
        out
    }

    #[test]
    fn the_outline_check_keeps_outline_emoji_and_rejects_bitmap_only_fonts() {
        // egui's own emoji fonts are outlines and must stay usable.
        assert_eq!(check(epaint_default_fonts::NOTO_EMOJI_REGULAR, 0), Ok(()));
        assert_eq!(check(epaint_default_fonts::EMOJI_ICON, 0), Ok(()));
        // A font that only has bitmaps has nothing epaint can draw.
        let bitmap_only = sfnt(&[(b"CBDT", &[0u8; 8]), (b"CBLC", &[0u8; 8])]);
        let err = check(&bitmap_only, 0).unwrap_err();
        assert!(err.contains("outline"), "{err}");
        // And garbage is a parse error, not a panic.
        assert!(check(b"nope", 0).is_err());
    }
}
