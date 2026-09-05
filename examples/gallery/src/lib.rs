//! Every example, its code and a tag filter, in one binary.
//!
//! Three flex columns and no `Panel`: the gallery follows the same rule it
//! imposes on the examples it embeds, so the centre column can hand an example
//! an area to fill. Switching examples changes the `key` on that column, which
//! makes the previous example's hooks unreachable; the pass-end sweep drops
//! them and the new example starts clean.

use egui_extras::syntax_highlighting::{CodeTheme, code_view_ui};
use example_meta::Meta;
use react_egui::prelude::*;
use react_egui_elements::prelude::*;

use clock::App as ClockApp;
use counter::App as CounterApp;
use custom_hook::App as CustomHookApp;
use escape_hatch::App as EscapeHatchApp;
use fetch::App as FetchApp;
use form::App as FormApp;
use layout::App as LayoutApp;
use list_10k::App as ListApp;
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
const REPO: &str = "https://github.com/fand/react-egui/blob/main/examples";

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
    // An example with no plain version is always shown as react-egui, whatever
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
                    // A new example starts on its react-egui version.
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
/// The escape hatch, because `react-egui-elements` has no toggle element and a
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
                "todo" => { <TodoPlain/> }
                "form" => { <FormPlain/> }
                "list-10k" => { <ListPlain/> }
                "layout" => { <LayoutPlain/> }
                _ => { <CounterPlain/> }
            }
        } else {
            match name {
                "counter" => { <CounterApp/> }
                "todo" => { <TodoApp/> }
                "form" => { <FormApp/> }
                "theme" => { <ThemeApp/> }
                "clock" => { <ClockApp/> }
                "custom-hook" => { <CustomHookApp/> }
                "escape-hatch" => { <EscapeHatchApp/> }
                "shader" => { <ShaderApp/> }
                // A thousand, not the ten thousand the binary opens with: at
                // 10k a frame takes ~85ms, and the gallery around it would
                // crawl too. The slider still reaches 10k for anyone curious.
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
/// This is what running a non-react-egui UI inside a react-egui tree looks
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
#[component]
fn Code(cx: &mut Cx, meta: Meta, plain: bool, #[event] on_pick: bool) {
    let source = if plain {
        meta.plain.unwrap_or(meta.source)
    } else {
        meta.source
    };
    let file = if plain { "plain.rs" } else { "lib.rs" };
    let link = format!("{REPO}/{}/src/{file}", meta.name);
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
                        label="react-egui"
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
            <ScrollArea grow={1.0} horizontal>
                {view(move |cx| {
                    // `leaf_fill`, not `leaf`: the highlighted block should take
                    // the scroll area's width rather than be measured by its
                    // longest line.
                    cx.leaf_fill(&ItemStyle::default(), |ui| {
                        let theme = CodeTheme::from_style(ui.style());
                        code_view_ui(ui, &theme, source, "rs");
                    });
                })}
            </ScrollArea>
        </View>
    }
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
