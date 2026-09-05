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

use counter::App as CounterApp;
use fetch::App as FetchApp;
use layout::App as LayoutApp;
use todo::App as TodoApp;

/// Every example the gallery can run, in list order.
///
/// The list, the tags and the code column all come from these; only
/// [`Running`] needs to know the concrete `App` behind a name, because each
/// `#[component]` has a props type of its own and they cannot share a `fn`
/// pointer.
pub const EXAMPLES: &[Meta] = &[counter::META, todo::META, layout::META, fetch::META];

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

    // Read both states once, so the handlers below are free to take them
    // `&mut` without tripping over a live borrow.
    let active: Vec<&'static str> = tags.to_vec();
    let current: &'static Meta = find(*selected).unwrap_or(&EXAMPLES[0]);
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
                on_select={|name: &'static str| *selected = name}
                on_tag={|tag: &'static str| toggle(&mut tags, tag)}
                on_clear={|| tags.clear()}
            />
            <Separator vertical/>
            // `min_w={0}`: without it the centre's automatic minimum size is
            // its content, and a wide example would push the code column off
            // the right edge instead of getting a scroll bar of its own.
            <View direction="column" grow={1.0} min_w={0.0} gap={4}>
                // The summary, not the name: the name is already the label of
                // the list button and two widgets with one label are ambiguous
                // to a screen reader (and to kittest).
                <Text strong>{current.summary}</Text>
                // The `key` is the whole point: change it and the previous
                // example's hooks are swept, so state does not leak across.
                <View key={current.name} direction="column" grow={1.0}>
                    <Running name={current.name}/>
                </View>
            </View>
            <Separator vertical/>
            <Code meta={*current}/>
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

/// A small button that keeps its label on one line.
///
/// `<Button>` would do, except for how a taffy leaf is measured. egui_taffy
/// caches the size a leaf reported the last time it was drawn and hands that
/// back to taffy as both the min- and the max-content size. The first draw
/// happens in a zero-width `Ui`, so a widget that wraps its text reports one
/// character there and the node never grows again: the label ends up written
/// downwards, one letter per line. `<Text>` avoids this by setting
/// `TextWrapMode::Extend` (ARCHITECTURE 6); `<Button>` keeps egui's default, so
/// the gallery sets the wrap mode on the leaf's `Ui` itself. Buttons with
/// `grow` are unaffected, because then taffy, not the content, sets the width.
///
/// This belongs in `react-egui-elements` — see the report for step 2.
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

/// The selected example.
///
/// A `match` rather than a table of function pointers: `#[component]` gives
/// every component its own props type, so the four `App`s have four different
/// signatures and no common `fn` type to store.
#[component]
fn Running(cx: &mut Cx, name: &'static str) {
    rsx! {
        match name {
            "todo" => { <TodoApp/> }
            "layout" => { <LayoutApp/> }
            "fetch" => { <FetchApp/> }
            _ => { <CounterApp/> }
        }
    }
}

/// The code column.
///
/// One component so that the react-egui / plain egui toggle can be added here
/// without touching the layout above.
#[component]
fn Code(cx: &mut Cx, meta: Meta) {
    let source = meta.source;
    let link = format!("{REPO}/{}/src/lib.rs", meta.name);

    rsx! {
        // Proportional with a floor, and free to shrink further only once the
        // centre has given up all of its width, so a narrow window never
        // leaves the running example with nothing.
        <View direction="column" w="40%" min_w={360.0} gap={6}>
            <View direction="row" gap={8} align="center" w="100%">
                <Text grow={1.0}>{format!("{} lines", source.lines().count())}</Text>
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
