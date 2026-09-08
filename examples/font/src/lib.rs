//! Fonts: CSS-style fallback chains, three ways to get the bytes, and what
//! each entry of a chain became.
//!
//! egui draws text from font bytes it was handed. The browser's fonts and the
//! OS's font matching are never involved, and egui's own four fonts have no
//! CJK glyphs, so Japanese draws as boxes in a plain egui app.
//! `egui_react_app::fonts::Fonts` is the answer: named chains of sources,
//! resolved through one `fontdb` database into egui's per-glyph fallback
//! lists, and picked per `<Text>` with the `font` prop.
//!
//! Three stacks, one per kind of source:
//!
//! - `bundled`: a 433 KB subset of Noto Sans JP compiled in with
//!   `include_bytes!` (`fonts/README.md` says what is in it and how it was
//!   made). It is also the `default_proportional`, so every widget of the
//!   example draws with it. Inside the gallery that restyles the gallery too:
//!   `set_fonts` is per `egui::Context` and there is one of those, which is
//!   what the egui API does and the example does not hide it.
//! - `web`: the full Noto Sans JP fetched from the example's own origin,
//!   `Url("fonts/NotoSansJP-Regular.otf")`. trunk copies the file next to
//!   `index.html`; a pre-build hook downloads it (4.5 MB, so it is not
//!   committed). The entry is `Pending` on the first frame and `Loaded` once
//!   the bytes arrive, and the subset stands behind it in the chain so the
//!   text is readable meanwhile. The file is the static Regular OTF from
//!   notofonts/noto-cjk, not the variable font from google/fonts: epaint
//!   draws a variable font's default instance, which for that file is Thin.
//!   Natively the same relative URL has no server to point at, so ehttp
//!   reports it as `Failed` and the chain draws with the subset; the report
//!   shows that too.
//! - `system`: fonts installed on the machine, by family name. Native reads
//!   them through fontdb's `load_system_fonts` on the first `apply`. wasm
//!   cannot read files; the "Use my fonts" button asks the browser for them
//!   through the Local Font Access API, which is Chromium only, shows a
//!   permission prompt, and has to be called from a click. Every other browser
//!   sees the button disabled with the reason. The last entry, `"Noto Sans
//!   JP"`, also matches the subset the `bundled` stack loaded into the same
//!   database (a `System` name finds any face the database has), so this
//!   chain never has to fall through to egui's font for Japanese; the report
//!   shows the same key for both.
//! - `code`: the monospace chain, and the `default_monospace`. Latin comes
//!   from egui's Hack (behind the `monospace` generic), and the subset stands
//!   behind it for the kana and kanji that Hack does not have, so the source
//!   pane of the gallery, which draws this file in `FontFamily::Monospace`,
//!   shows the sample strings instead of boxes. A monospace CJK font would
//!   keep the columns aligned; this one only keeps them readable.
//!
//! The report table under the samples is the point of the example: it lists,
//! for every entry of every chain, the face it resolved to (with the family
//! name the face declares, which is how to find the spelling a `System` entry
//! needs) or why it did not.

use std::sync::LazyLock;

use egui_react::prelude::*;
use egui_react_app::fonts::{FontSource, Fonts, Generic, Outcome};
use egui_react_elements::prelude::*;
use example_meta::Meta;

pub const META: Meta = Meta {
    name: "font",
    summary: "CSS-style font chains: bundled, fetched and installed fonts, and what each entry resolved to.",
    hooks: &["use_state", "use_effect", "use_reducer"],
    elements: &["View", "Text", "Button", "ScrollArea", "Separator"],
    source: include_str!("lib.rs"),
    plain: None,
};

/// The subset of Noto Sans JP compiled into the binary: ASCII, Latin-1, kana,
/// CJK punctuation, fullwidth forms and about five hundred kanji.
const NOTO_SANS_JP_SUBSET: &[u8] = include_bytes!("../fonts/NotoSansJP-Subset.ttf");

/// Where the `web` stack fetches the full font from: next to `index.html`.
pub const WEB_FONT_URL: &str = "fonts/NotoSansJP-Regular.otf";

/// The stacks, in the order the buttons show them.
pub const STACKS: [&str; 4] = ["bundled", "web", "system", "code"];

/// What is drawn with the selected stack. Every character is in the subset.
pub const SAMPLES: [&str; 5] = [
    "日本語のテキストが表示できます。",
    "フォントのフォールバックチェーン",
    "吾輩は猫である。名前はまだ無い。",
    "東京都渋谷区、2026年9月7日",
    "egui-react で CSS の font-family のように書ける",
];

/// The one `Fonts` of the process. `Fonts` is a handle (a clone shares the
/// database), so `main.rs` applies it from `Options::setup`, `App` applies
/// it on its first frame, and the components read its report every frame.
static FONTS: LazyLock<Fonts> = LazyLock::new(build_fonts);

/// A handle to the process-wide [`Fonts`].
pub fn fonts() -> Fonts {
    FONTS.clone()
}

/// The three chains, as an app would write them.
pub fn build_fonts() -> Fonts {
    Fonts::new()
        .stack(
            "bundled",
            [
                FontSource::Bundled(NOTO_SANS_JP_SUBSET),
                FontSource::Generic(Generic::SansSerif),
            ],
        )
        .stack(
            "web",
            [
                FontSource::Url(WEB_FONT_URL.into()),
                FontSource::Bundled(NOTO_SANS_JP_SUBSET),
                FontSource::Generic(Generic::SansSerif),
            ],
        )
        .stack(
            "system",
            [
                FontSource::System("Hiragino Sans".into()),
                FontSource::System("Yu Gothic UI".into()),
                FontSource::System("Noto Sans CJK JP".into()),
                FontSource::System("Noto Sans JP".into()),
                FontSource::Generic(Generic::SansSerif),
            ],
        )
        .stack(
            "code",
            [
                FontSource::Generic(Generic::Monospace),
                FontSource::Bundled(NOTO_SANS_JP_SUBSET),
                FontSource::Generic(Generic::SansSerif),
            ],
        )
        .default_proportional("bundled")
        .default_monospace("code")
}

#[component]
pub fn App(cx: &mut Cx) {
    let mut stack = use_state(cx, || STACKS[0]);
    let current: &'static str = *stack;

    // The gallery runs `App` inside its own runner and has no `setup` hook
    // per example, so the stacks are applied here, once. Under `main.rs` the
    // setup already did it and this changes nothing: `apply` only calls
    // `set_fonts` when the definitions differ from the last time.
    let ctx = cx.ctx().clone();
    use_effect(cx, (), move || {
        fonts().apply(&ctx);
        // New fonts take effect at the start of the next pass.
        ctx.request_repaint();
    });
    // On the frame that applied them the names are not registered yet, and
    // `<Text font>` would warn about that (once) before falling back. The
    // built-in family is always registered, so it stands in for that frame.
    let ready = cx.ctx().fonts(|f| {
        f.definitions()
            .families
            .contains_key(&egui::FontFamily::Name(current.into()))
    });
    let font = if ready { current } else { "proportional" };

    rsx! {
        // The root fills whatever area it is given, so the gallery can drop it
        // into a column of its own.
        <View direction="column" grow={1.0}>
            <ScrollArea grow={1.0}>
                <View direction="column" gap={8} p={12} w="100%">
                    <Text size={22.0} strong>"font"</Text>

                    <View direction="row" gap={8} align="center">
                        <Text>"stack:"</Text>
                        for name in STACKS {
                            <Button key={name} on_click={|| *stack = name}>{name}</Button>
                        }
                        <Text>{format!("<Text font=\"{current}\">")}</Text>
                    </View>
                    // How this stack gets its bytes on the target this build
                    // runs on; `wrap` needs a width, which `w` gives it.
                    <Text wrap w="100%">{how(current)}</Text>

                    <View direction="column" gap={4}>
                        for (i, sample) in SAMPLES.iter().enumerate() {
                            <Text key={i} font={font} size={20.0}>{*sample}</Text>
                        }
                    </View>

                    <Separator/>
                    <LocalFonts/>
                    <Separator/>
                    <Report/>
                </View>
            </ScrollArea>
        </View>
    }
}

/// The Local Font Access button. On wasm it is enabled when the browser has
/// `window.queryLocalFonts`; everywhere else it is disabled with the reason next to
/// it, so the example is one source file.
#[component]
fn LocalFonts(cx: &mut Cx) {
    // The request is `async` and finishes on the browser's event loop (on a
    // thread of its own natively, where it answers `Ok(0)` at once), so the
    // result comes back through a `Dispatch`: it is `Send`, and `send` asks
    // for the repaint that shows it.
    let (status, dispatch) = use_reducer(
        cx,
        |status: &mut String, message: String| *status = message,
        String::new,
    );
    let available = Fonts::local_fonts_available();
    let note = if cfg!(target_arch = "wasm32") {
        if available {
            "Chromium asks for permission once; the \"system\" stack is then resolved from the grant."
        } else {
            "Local Font Access: Chromium only"
        }
    } else {
        "installed fonts are loaded automatically"
    };

    rsx! {
        <View direction="row" gap={8} align="center" w="100%">
            <Button enabled={available} on_click={|| {
                let fonts = fonts();
                let dispatch = dispatch.clone();
                // `spawn` is `wasm_bindgen_futures::spawn_local` on wasm, so
                // the call still counts as coming from the click, which the
                // permission prompt requires.
                spawn(async move {
                    let message = match fonts.request_local_fonts().await {
                        Ok(n) => format!("{n} of the named families found on this machine"),
                        Err(err) => format!("local fonts: {err}"),
                    };
                    dispatch.send(message);
                });
            }}>
                "Use my fonts"
            </Button>
            <Text>{note}</Text>
            <Text strong>{status.as_str()}</Text>
        </View>
    }
}

/// The report: every entry of every stack and what it became on the last
/// `apply`. Read every frame; it is a lock and a clone of a dozen entries, and
/// it is how the `web` entry is seen going from `pending` to `loaded`.
#[component]
fn Report(cx: &mut Cx) {
    let report = fonts().report();

    rsx! {
        <View direction="column" gap={4} w="100%">
            <Text strong>"report"</Text>
            for (i, stack) in report.iter().enumerate() {
                <View key={i} direction="column" gap={2} w="100%" mt={4}>
                    <Text strong>{format!("stack \"{}\"", stack.name)}</Text>
                    for (j, (source, outcome)) in stack.entries.iter().enumerate() {
                        <View key={j} direction="row" gap={8} w="100%">
                            <Text w={280.0}>{source.to_string()}</Text>
                            // A failure message can be long; `wrap` needs a
                            // width, which `grow` gives it in a sized row.
                            <Text grow={1.0} wrap>{describe(outcome)}</Text>
                        </View>
                    }
                </View>
            }
        </View>
    }
}

/// How a stack gets its bytes on the target this build runs on. Shown under
/// the stack buttons, so the report below it can be read as "this is what
/// that produced".
pub fn how(stack: &str) -> &'static str {
    let wasm = cfg!(target_arch = "wasm32");
    match (stack, wasm) {
        ("bundled", true) => {
            "Compiled into the .wasm with include_bytes! (a 433 KB subset of Noto Sans JP) and \
             registered from Options::setup, before the first frame. Nothing to wait for and \
             nothing to fetch; the cost is binary size, which is why it is a subset."
        }
        ("bundled", false) => {
            "Compiled into the binary with include_bytes! (a 433 KB subset of Noto Sans JP) and \
             registered from Options::setup, before the first frame."
        }
        ("web", true) => {
            "fetch() of fonts/NotoSansJP-Regular.otf (4.5 MB) from the page's own origin, \
             started by the first apply. Until the bytes arrive the entry is pending and the \
             subset behind it draws; when they land the chains are applied again and egui \
             rebuilds its glyph atlas once. A cross-origin URL would need CORS headers."
        }
        ("web", false) => {
            "An HTTP fetch (ureq on a thread). Here the relative URL has no server to point at, \
             so the entry is failed and the subset behind it draws; in the browser the same \
             URL is fetched from the page's origin."
        }
        ("system", true) => {
            "The browser cannot read installed fonts, so each name is missing until \"Use my \
             fonts\" asks for them through the Local Font Access API (Chromium only, a \
             permission prompt, has to start from a click). The granted files go into the same \
             database and this chain is resolved again. \"Noto Sans JP\" already matches the \
             bundled subset, which is why Japanese draws before any grant."
        }
        ("system", false) => {
            "fontdb scans the OS font directories once (load_system_fonts) and each name is \
             matched against the installed families as CSS would; a name no font declares is \
             missing and skipped, and the report shows the family each face declares."
        }
        ("code", _) => {
            "The monospace generic first (egui's Hack in the browser, the installed monospace \
             default natively), then the bundled subset for the kana and kanji Hack lacks. It \
             is also the Monospace default, which the gallery's source pane draws with."
        }
        _ => "",
    }
}

/// One line per outcome, for the report table.
pub fn describe(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Loaded { key, family } => format!("loaded: {family} ({key})"),
        Outcome::Pending => String::from("pending: the bytes have not arrived yet"),
        Outcome::Missing => String::from("missing: no face with this family name"),
        Outcome::Invalid(why) => format!("invalid: {why}"),
        Outcome::Failed(why) => format!("failed: {why}"),
    }
}
