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
pub(super) fn resolve(input: Input<'_>, font_data: &mut HashMap<String, Arc<FontData>>) -> Output {
    let mut defs = FontDefinitions::default();
    // The tails, taken before a default stack rewrites either family: the
    // two emoji fonts at the end of these are what draw icons in every chain.
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

    let mut report = Vec::with_capacity(input.stacks.len());
    let mut lists: HashMap<Arc<str>, Vec<String>> = HashMap::new();

    for stack in input.stacks {
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
                    let loaded = input.loaded.get(&SourceKey::bundled(bytes));
                    resolve_blob(
                        &input, stack, loaded, &mut defs, font_data, &mut keys, &mut push,
                    )
                }
                FontSource::Url(url) => {
                    let loaded = input.loaded.get(&SourceKey::Url(url.clone()));
                    resolve_blob(
                        &input, stack, loaded, &mut defs, font_data, &mut keys, &mut push,
                    )
                }
                FontSource::System(name) => {
                    let query = fontdb::Query {
                        families: &[fontdb::Family::Name(name)],
                        weight: stack.weight,
                        style: stack.style,
                        stretch: fontdb::Stretch::Normal,
                    };
                    match input.db.query(&query) {
                        None => Outcome::Missing,
                        Some(id) => match register(input.db, id, None, &mut defs, font_data) {
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
                    // can use it; egui's own font behind it regardless, so
                    // the generic always draws something.
                    let mut first: Option<Face> = None;
                    if let Some(id) = input.db.query(&query) {
                        match register(input.db, id, None, &mut defs, font_data) {
                            Ok(face) => {
                                push(&mut keys, face.key.clone());
                                first = Some(face);
                            }
                            Err(reason) => {
                                log::warn!("egui-react fonts: {source}: {reason}");
                            }
                        }
                    }
                    let builtin = generic.builtin();
                    push(&mut keys, builtin.to_owned());
                    match first {
                        Some(face) => face.into_outcome(),
                        None => Outcome::Loaded {
                            key: builtin.to_owned(),
                            family: builtin.to_owned(),
                        },
                    }
                }
            };
            entries.push((source.clone(), outcome));
        }

        let tail = if input.default_monospace == Some(&*stack.name) {
            &builtin_monospace
        } else {
            &builtin_proportional
        };
        for key in tail {
            push(&mut keys, key.clone());
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
        (input.default_proportional, FontFamily::Proportional),
        (input.default_monospace, FontFamily::Monospace),
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

/// A `Bundled` or `Url` entry: pick the best face among the blob's own.
#[allow(clippy::too_many_arguments)]
fn resolve_blob(
    input: &Input<'_>,
    stack: &FontStack,
    loaded: Option<&Loaded>,
    defs: &mut FontDefinitions,
    font_data: &mut HashMap<String, Arc<FontData>>,
    keys: &mut Vec<String>,
    push: &mut impl FnMut(&mut Vec<String>, String),
) -> Outcome {
    match loaded {
        None | Some(Loaded::Pending) => Outcome::Pending,
        Some(Loaded::Faces { ids, static_bytes }) => {
            let Some(id) = best_of(input.db, ids, stack.weight, stack.style) else {
                return Outcome::Invalid(String::from("no font face could be parsed"));
            };
            match register(input.db, id, *static_bytes, defs, font_data) {
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
/// copying and checking the bytes only the first time this key is seen.
///
/// Two files that declare the same PostScript name share one key, and the
/// first one wins; that is the same font under two paths in every case met
/// so far, and one key per name is what keeps `set_fonts` from holding two
/// copies of it.
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
    let key = format!("{}#{}", info.post_script_name, info.index);
    let family = info
        .families
        .first()
        .map(|(name, _)| name.clone())
        .unwrap_or_else(|| info.post_script_name.clone());

    if !defs.font_data.contains_key(&key) {
        let data = match font_data.get(&key) {
            Some(data) => Arc::clone(data),
            None => {
                let font: Cow<'static, [u8]> = match static_bytes {
                    Some(bytes) => {
                        check(bytes, info.index).map_err(|e| format!("{key}: {e}"))?;
                        Cow::Borrowed(bytes)
                    }
                    None => {
                        // Check before copying, so a rejected face costs no
                        // allocation; the copy is what epaint needs, since
                        // `FontData` owns or borrows `'static`.
                        let copied = db
                            .with_face_data(id, |bytes, index| {
                                check(bytes, index).map(|()| bytes.to_vec())
                            })
                            .ok_or_else(|| format!("{key}: the font file could not be read"))?;
                        Cow::Owned(copied.map_err(|e| format!("{key}: {e}"))?)
                    }
                };
                let data = Arc::new(FontData {
                    font,
                    index: info.index,
                    tweak: Default::default(),
                });
                font_data.insert(key.clone(), Arc::clone(&data));
                data
            }
        };
        defs.font_data.insert(key.clone(), data);
    }
    Ok(Face { key, family })
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
            resolve(
                Input {
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
        assert_eq!(family(&out.definitions, "code")[1], "Hack");
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

    #[test]
    fn a_stack_with_no_hits_still_registers_its_name() {
        let mut fx = Fixture::new();
        let out = fx.resolve(&[FontStack::new("empty", [FontSource::System("Nope".into())])]);
        assert_eq!(
            family(&out.definitions, "empty"),
            FontDefinitions::default().families[&FontFamily::Proportional]
        );
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
