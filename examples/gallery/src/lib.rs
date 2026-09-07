//! Every example, its code and a tag filter, in one binary.
//!
//! Three flex columns and no `Panel`: the gallery follows the same rule it
//! imposes on the examples it embeds, so the centre column can hand an example
//! an area to fill. Switching examples changes the `key` on that column, which
//! makes the previous example's hooks unreachable; the pass-end sweep drops
//! them and the new example starts clean.

use std::sync::Arc;

use egui_extras::syntax_highlighting::{CodeTheme, highlight};
use egui_react::prelude::*;
use egui_react_elements::prelude::*;
use example_meta::Meta;

use board::App as BoardApp;
use clock::App as ClockApp;
use counter::App as CounterApp;
use custom_hook::App as CustomHookApp;
use escape_hatch::App as EscapeHatchApp;
use fetch::App as FetchApp;
use form::App as FormApp;
use layout::App as LayoutApp;
use list_10k::App as ListApp;
use patch::App as PatchApp;
use shader::App as ShaderApp;
use showcase::App as ShowcaseApp;
use theme::App as ThemeApp;
use todo::App as TodoApp;

/// Every example the gallery can run, in list order.
///
/// The list, the tags and the code column all come from these; only
/// [`Running`] needs to know the concrete `App` behind a name, because each
/// `#[component]` has a props type of its own and they cannot share a `fn`
/// pointer.
pub const EXAMPLES: &[Meta] = &[
    showcase::META,
    board::META,
    patch::META,
    counter::META,
    todo::META,
    form::META,
    theme::META,
    clock::META,
    custom_hook::META,
    escape_hatch::META,
    shader::META,
    list_10k::META,
    layout::META,
    fetch::META,
];

/// Where the source links point.
const REPO: &str = "https://github.com/fand/egui-react/blob/main/examples";

/// The gallery: list, running example, code.
///
/// `start` is the example to open first; [`initial_example`] is where the
/// runner gets it. It is a prop and not something the component reads for
/// itself, because reading the process arguments here would make the gallery
/// open a different example under `cargo test <filter>`.
#[component]
pub fn App(cx: &mut Cx, #[prop(default = EXAMPLES[0].name)] start: &'static str) {
    let mut selected = use_state(cx, move || start);
    let mut tags = use_state(cx, Vec::<&'static str>::new);
    let mut plain = use_state(cx, || false);

    // Read the states once, so the handlers below are free to take them `&mut`
    // without tripping over a live borrow.
    let active: Vec<&'static str> = tags.to_vec();
    let current: &'static Meta = find(*selected).unwrap_or(&EXAMPLES[0]);
    // An example with no plain version is always shown as egui-react, whatever
    // the toggle was left on.
    let showing_plain = *plain && current.plain.is_some();
    let shown: Vec<&'static Meta> = EXAMPLES
        .iter()
        .filter(|meta| matches_tags(meta, &active))
        .collect();

    // Keep `#todo` in the address bar in step with the selection.
    use_effect(cx, current.name, || set_hash(current.name));

    rsx! {
        <View direction="row" grow={1.0} gap={8}>
            <List
                shown={&shown}
                active={&active}
                selected={current.name}
                on_select={|name: &'static str| {
                    *selected = name;
                    // A new example starts on its egui-react version.
                    *plain = false;
                }}
                on_tag={|tag: &'static str| toggle(&mut tags, tag)}
                on_clear={|| tags.clear()}
            />
            <Separator vertical/>
            // `min_w={0}` makes the centre the column that gives way: taffy
            // may otherwise take a flex item's content as its automatic
            // minimum, and a wide example would push the code column off the
            // right edge instead of being cut off itself.
            <View direction="column" grow={1.0} min_w={0.0} gap={4}>
                // The summary, not the name: the name is already the label of
                // the list button and two widgets with one label are ambiguous
                // to a screen reader (and to kittest).
                <Text strong>{current.summary}</Text>
                // The `key` is the whole point: change it and the previous
                // example's hooks are swept, so state does not leak across.
                <View key={current.name} direction="column" grow={1.0}>
                    <Running name={current.name} plain={showing_plain}/>
                </View>
            </View>
            <Separator vertical/>
            <Code
                meta={*current}
                plain={showing_plain}
                on_pick={|pick: bool| *plain = pick}
            />
        </View>
    }
}

/// The example list and the tag filter.
#[component]
fn List(
    cx: &mut Cx,
    shown: &[&'static Meta],
    active: &[&'static str],
    selected: &'static str,
    #[event] on_select: &'static str,
    #[event] on_tag: &'static str,
    #[event] on_clear: (),
) {
    rsx! {
        // `shrink={0}` so the list keeps its width when the window is narrow;
        // the centre column is the one that gives way.
        <View direction="column" w={200.0} shrink={0.0} gap={6}>
            <Text strong size={18.0}>"examples"</Text>
            <ScrollArea grow={1.0}>
                <View direction="column" gap={4} w="100%">
                    for meta in shown.iter() {
                        <View key={meta.name} direction="row" gap={4} align="center" w="100%">
                            <Text w={12.0}>{marker(meta.name == selected)}</Text>
                            <Button grow={1.0} on_click={|| on_select.emit(meta.name)}>
                                {meta.name}
                            </Button>
                        </View>
                    }

                    <Separator/>

                    <View direction="row" gap={4} align="center" w="100%">
                        <Text grow={1.0} strong>"tags"</Text>
                        if !active.is_empty() {
                            <Chip label="clear" on_click={|| on_clear.emit(())}/>
                        }
                    </View>
                    <View direction="row" wrap gap={(4.0, 4.0)} w="100%">
                        for tag in all_tags() {
                            <Chip
                                key={tag}
                                label={tag}
                                active={active.contains(&tag)}
                                on_click={|| on_tag.emit(tag)}
                            />
                        }
                    </View>
                </View>
            </ScrollArea>
        </View>
    }
}

/// A small button, optionally showing whether it is on.
///
/// The escape hatch, because `egui-react-elements` has no toggle element and a
/// tag wants to look pressed while it is filtering. `egui::SelectableLabel` has
/// no `wrap_mode` builder, so the mode goes on the leaf's `Ui`: a taffy leaf is
/// measured from its first draw, which happens in a zero-width `Ui`, and a
/// widget left to wrap reports one character wide and stays that way. The
/// elements set this for themselves (see `widgets`); a hand-written leaf has
/// to do it too.
#[component]
fn Chip(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    label: &str,
    // `active` is `Some` for a toggle that shows that state, `None` for a
    // plain button.
    active: Option<bool>,
    #[event] on_click: (),
) {
    let clicked = cx.leaf(&style.shrink(0.0), |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        match active {
            Some(active) => ui.selectable_label(active, label).clicked(),
            None => ui.button(label).clicked(),
        }
    });
    if clicked {
        on_click.emit(());
    }
}

/// The selected example, in the version the toggle asks for.
///
/// A `match` rather than a table of function pointers: `#[component]` gives
/// every component its own props type, so the four `App`s have four different
/// signatures and no common `fn` type to store. The plain versions have the
/// same problem for the opposite reason — each has its own `PlainState`.
#[component]
fn Running(cx: &mut Cx, name: &'static str, plain: bool) {
    rsx! {
        if plain {
            match name {
                "board" => { <BoardPlain/> }
                "todo" => { <TodoPlain/> }
                "form" => { <FormPlain/> }
                "list-10k" => { <ListPlain/> }
                "layout" => { <LayoutPlain/> }
                _ => { <CounterPlain/> }
            }
        } else {
            match name {
                "board" => { <BoardApp/> }
                "patch" => { <PatchApp/> }
                "counter" => { <CounterApp/> }
                "todo" => { <TodoApp/> }
                "form" => { <FormApp/> }
                "theme" => { <ThemeApp/> }
                "clock" => { <ClockApp/> }
                "custom-hook" => { <CustomHookApp/> }
                "escape-hatch" => { <EscapeHatchApp/> }
                "shader" => { <ShaderApp/> }
                // A thousand, not the hundred thousand the binary opens with,
                // to match the plain column next to it (`ListPlain`). The rows
                // are virtualised, so the slider still reaches 100k at no cost.
                "list-10k" => { <ListApp initial_count={1_000}/> }
                "layout" => { <LayoutApp/> }
                "fetch" => { <FetchApp/> }
                _ => { <ShowcaseApp/> }
            }
        }
    }
}

/// A plain egui example: one `use_state` for its whole state, drawn into a
/// leaf that taffy sizes.
///
/// This is what running a non-egui-react UI inside a egui-react tree looks
/// like: the state is a hook, the drawing is a closure over `&mut egui::Ui`.
///
/// `bind()` rather than `&mut *state`, for the same reason a bound `TextEdit`
/// uses it: the plain `ui` writes into the state every frame whether anything
/// changed or not, and `&mut *state` would read that as a change and ask for
/// another repaint, forever.
macro_rules! plain_example {
    ($name:ident, $module:path) => {
        #[component]
        fn $name(cx: &mut Cx) {
            use $module as plain;
            let mut state = use_state(cx, plain::PlainState::default);
            cx.leaf_fill(&ItemStyle::default().grow(1.0), |ui| {
                plain::ui(ui, state.bind());
            });
        }
    };
}

plain_example!(BoardPlain, board::plain);
plain_example!(CounterPlain, counter::plain);
plain_example!(TodoPlain, todo::plain);
plain_example!(FormPlain, form::plain);

/// The plain list, written out rather than through [`plain_example!`], so both
/// versions show the same number of rows and the comparison is fair. The
/// virtualised one would not have minded ten thousand.
#[component]
fn ListPlain(cx: &mut Cx) {
    let mut state = use_state(cx, || list_10k::plain::PlainState::with_count(1_000));
    cx.leaf_fill(&ItemStyle::default().grow(1.0), |ui| {
        list_10k::plain::ui(ui, state.bind());
    });
}
plain_example!(LayoutPlain, layout::plain);

/// The code column, and the toggle between the two versions.
///
/// The toggle lives here because the line counts are the point of it: the two
/// versions draw the same thing, and the numbers next to the buttons say what
/// that costs in each.
///
/// `pub` for `tests/bench.rs`, which times this column on its own.
#[component]
pub fn Code(cx: &mut Cx, meta: Meta, plain: bool, #[event] on_pick: bool) {
    let source = if plain {
        meta.plain.unwrap_or(meta.source)
    } else {
        meta.source
    };
    let file = if plain { "plain.rs" } else { "lib.rs" };
    let link = format!("{REPO}/{}/src/{file}", meta.name);
    let mut lines = use_state(cx, || None::<(GalleyKey, CodeLines)>);
    let lines: &CodeLines = code_lines(cx.ui(), lines.bind(), (meta.name, plain), source);
    let react_lines = format!("{} lines", meta.source.lines().count());
    let plain_lines = meta.plain.map_or(String::new(), |p| {
        format!("{} lines plain", p.lines().count())
    });

    rsx! {
        // A fixed share of the window, never squeezed by what the running
        // example wants: `shrink={0}` sends the whole overflow to the centre
        // column, which is the one with `min_w={0}`.
        <View direction="column" w="40%" min_w={360.0} shrink={0.0} gap={6}>
            if meta.plain.is_some() {
                <View direction="row" gap={4} align="center" w="100%">
                    <Chip
                        label="egui-react"
                        active={!plain}
                        on_click={|| on_pick.emit(false)}
                    />
                    <Chip label="plain egui" active={plain} on_click={|| on_pick.emit(true)}/>
                </View>
            }
            <View direction="row" gap={8} align="center" w="100%">
                <Text>{react_lines.as_str()}</Text>
                <Text grow={1.0}>{plain_lines.as_str()}</Text>
                {view(move |cx| {
                    // Same measuring trap as [`Chip`]: left to wrap, the link
                    // is measured at one character wide and 200px tall, and
                    // `align="center"` then pushes the line count down with it.
                    cx.leaf(&ItemStyle::default().shrink(0.0), |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        ui.hyperlink_to("source on GitHub", &link);
                    });
                })}
            </View>
            // One row per line, and only the rows in view are drawn: a
            // selectable label reports every row of its galley to accesskit
            // each frame, so a label per line keeps that to a screenful. The
            // outer scroll area is the sideways one; the list scrolls down.
            <ScrollArea grow={1.0} horizontal vertical={false}>
                <VirtualList
                    w={lines.width}
                    rows={lines.galleys.len()}
                    row_h={lines.row_h}
                    render={|cx: &mut Cx<'_, '_>, row: usize| {
                        let galley = lines.galleys[row].clone();
                        cx.leaf(&ItemStyle::default(), |ui| {
                            ui.add(egui::Label::new(galley).selectable(true));
                        });
                    }}
                />
            </ScrollArea>
        </View>
    }
}

/// What the cached code lines were laid out for. A new key means a new layout.
///
/// The source is named, not hashed: hashing 66 KB a frame was the cost this
/// cache is here to remove. The atlas size is in because a galley stores atlas
/// pixel coordinates: growing the atlas keeps them, a reset changes the size.
type GalleyKey = ((&'static str, bool), bool, f32, f32, [usize; 2]);

/// The highlighted source, one galley per line, laid out once per
/// [`GalleyKey`].
struct CodeLines {
    galleys: Vec<Arc<egui::Galley>>,
    /// The pitch of the list: one monospace row.
    row_h: f32,
    /// The widest line, which is how wide the list is inside the sideways
    /// scroll area.
    width: f32,
}

/// The lines of the source as galleys, from the cache or laid out now.
///
/// `code_view_ui` highlights and lays out from scratch every frame: it hashes
/// the whole source for the highlight cache, then hashes the `LayoutJob` (a
/// section per token) for the galley cache. A `Label` handed an `Arc<Galley>`
/// does neither. The source is highlighted whole and the job cut at each
/// newline, so a block comment or a multi-line string is coloured the same as
/// it would be in one piece.
fn code_lines<'a>(
    ui: &mut egui::Ui,
    cache: &'a mut Option<(GalleyKey, CodeLines)>,
    which: (&'static str, bool),
    source: &str,
) -> &'a CodeLines {
    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
    let key: GalleyKey = (
        which,
        ui.visuals().dark_mode,
        font_id.size,
        ui.pixels_per_point(),
        ui.fonts(|f| f.font_image_size()),
    );
    if cache.as_ref().is_none_or(|(k, _)| *k != key) {
        let theme = CodeTheme::from_style(ui.style());
        let job = highlight(ui.ctx(), ui.style(), &theme, source, "rs");
        let (galleys, row_h) = ui.fonts_mut(|fonts| {
            let galleys: Vec<Arc<egui::Galley>> = split_lines(&job)
                .into_iter()
                .map(|job| fonts.layout_job(job))
                .collect();
            (galleys, fonts.row_height(&font_id))
        });
        let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
        *cache = Some((
            key,
            CodeLines {
                galleys,
                row_h,
                width,
            },
        ));
    }
    &cache.as_ref().expect("just filled").1
}

/// One `LayoutJob` per line of `job`, each with the sections that fall on that
/// line, clipped to it, with byte ranges relative to the line's own text.
///
/// The lines come from the text, not from the sections: the highlighter is
/// free to leave a newline outside every section, and a line it skipped would
/// otherwise go missing. A trailing newline does not make an empty last line,
/// so the count matches `str::lines`.
fn split_lines(job: &egui::text::LayoutJob) -> Vec<egui::text::LayoutJob> {
    use egui::text::{ByteIndex, LayoutJob, LayoutSection};

    let text = &job.text;
    let mut sections = job.sections.iter().peekable();
    let mut lines = Vec::new();
    let mut line_start = 0;
    for text_line in text.lines() {
        let line_end = line_start + text_line.len();
        let mut line = LayoutJob {
            text: text_line.to_owned(),
            ..Default::default()
        };
        // A row is a fixed pitch in the list; a line is never wrapped.
        line.wrap.max_width = f32::INFINITY;
        // Sections are in text order. Take every one that overlaps this line;
        // one that runs past its end also belongs to the next line, so it is
        // peeked, not consumed.
        while let Some(section) = sections.peek() {
            let (start, end) = (section.byte_range.start.0, section.byte_range.end.0);
            if start >= line_end {
                break;
            }
            let piece = start.max(line_start)..end.min(line_end);
            if piece.start < piece.end {
                line.sections.push(LayoutSection {
                    leading_space: 0.0,
                    byte_range: ByteIndex(piece.start - line_start)
                        ..ByteIndex(piece.end - line_start),
                    format: section.format.clone(),
                });
            }
            if end > line_end {
                break;
            }
            sections.next();
        }
        lines.push(line);
        // Past the line and its `\n` (or `\r\n`: `lines` strips both, and the
        // next line starts after whatever was stripped).
        line_start = match text[line_end..].find('\n') {
            Some(i) => line_end + i + 1,
            None => text.len(),
        };
    }
    lines
}

/// The example with this name, if there is one.
pub fn find(name: &str) -> Option<&'static Meta> {
    EXAMPLES.iter().find(|meta| meta.name == name)
}

/// Every hook and element tag, deduplicated, hooks before elements.
pub fn all_tags() -> Vec<&'static str> {
    let mut tags = dedup(EXAMPLES.iter().flat_map(|meta| meta.hooks));
    tags.extend(dedup(EXAMPLES.iter().flat_map(|meta| meta.elements)));
    tags
}

fn dedup<'a>(tags: impl Iterator<Item = &'a &'static str>) -> Vec<&'static str> {
    let mut tags: Vec<&'static str> = tags.copied().collect();
    tags.sort_unstable();
    tags.dedup();
    tags
}

/// An example is shown when it carries *every* active tag, so each click
/// narrows the list.
fn matches_tags(meta: &Meta, active: &[&'static str]) -> bool {
    active
        .iter()
        .all(|tag| meta.hooks.contains(tag) || meta.elements.contains(tag))
}

fn toggle(tags: &mut Vec<&'static str>, tag: &'static str) {
    match tags.iter().position(|t| *t == tag) {
        Some(i) => {
            tags.remove(i);
        }
        None => tags.push(tag),
    }
}

/// The bullet in front of a selected row or an active tag.
fn marker(on: bool) -> &'static str {
    if on { "*" } else { " " }
}

/// The example the URL hash (web) or the first argument (native) asks for.
///
/// An unknown or missing name falls back to the first example.
pub fn initial_example() -> &'static str {
    requested()
        .as_deref()
        .and_then(find)
        .map_or(EXAMPLES[0].name, |meta| meta.name)
}

#[cfg(target_arch = "wasm32")]
fn requested() -> Option<String> {
    let hash = web_sys::window()?.location().hash().ok()?;
    let name = hash.trim_start_matches('#').to_owned();
    (!name.is_empty()).then_some(name)
}

#[cfg(not(target_arch = "wasm32"))]
fn requested() -> Option<String> {
    std::env::args().nth(1)
}

#[cfg(target_arch = "wasm32")]
fn set_hash(name: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_hash(name);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn set_hash(_name: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every line of the source comes out, in order, with its text intact.
    #[test]
    fn split_lines_keeps_every_line() {
        for meta in EXAMPLES {
            let ctx = egui::Context::default();
            let style = ctx.global_style();
            let theme = CodeTheme::from_style(&style);
            let job = highlight(&ctx, &style, &theme, meta.source, "rs");
            let lines = split_lines(&job);
            let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
            let expected: Vec<&str> = meta.source.lines().collect();
            assert_eq!(texts, expected, "{}", meta.name);
            for (line, job) in lines.iter().enumerate() {
                for section in &job.sections {
                    assert!(
                        section.byte_range.end.0 <= job.text.len(),
                        "{} line {line}: section past the line",
                        meta.name
                    );
                }
            }
        }
    }
}
