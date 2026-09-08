//! Every example, its code and a tag filter, in one binary.
//!
//! A header and three flex columns, and no `Panel`: the gallery follows the
//! same rule it imposes on the examples it embeds, so the centre column can
//! hand an example an area to fill. Switching examples changes the `key` on
//! that column, which makes the previous example's hooks unreachable; the
//! pass-end sweep drops them and the new example starts clean.
//!
//! Under [`COMPACT_WIDTH`] (a phone) there is no room for three columns, so
//! the page is one pane: the example or its code, swapped by a button floating
//! in the bottom-right corner. The list and the tag filter move into a menu
//! that slides down over the whole window from the button in the header.

use std::sync::Arc;

mod highlight;
use egui_react::prelude::*;
use egui_react_app::root_style;
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

/// Below this window width the gallery is one pane and a menu rather than
/// three columns.
///
/// The columns need 200 points for the list, 360 for the code and something
/// left over for the example; a phone in portrait has about 400 in all, and a
/// tablet on its side has more than this.
pub const COMPACT_WIDTH: f32 = 720.0;

/// How long the menu takes to slide down, or back up, in seconds.
const MENU_TIME: f32 = 0.25;

/// What the compact layout shows: the running example, or its code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pane {
    /// The running example.
    #[default]
    Example,
    /// The example's source.
    Code,
}

/// The gallery: header, list, running example, code.
///
/// `start` is the example to open first; [`initial_example`] is where the
/// runner gets it. It is a prop and not something the component reads for
/// itself, because reading the process arguments here would make the gallery
/// open a different example under `cargo test <filter>`.
///
/// The layout follows the window: three columns above [`COMPACT_WIDTH`], one
/// pane and a menu below it. The states are the same either way, so turning a
/// phone keeps the example, the filter and the version that was picked.
#[component]
pub fn App(cx: &mut Cx, #[prop(default = EXAMPLES[0].name)] start: &'static str) {
    let mut selected = use_state(cx, move || start);
    let mut tags = use_state(cx, Vec::<&'static str>::new);
    let mut plain = use_state(cx, || false);
    let mut pane = use_state(cx, Pane::default);
    let mut menu_open = use_state(cx, || false);

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
    let compact = cx.ctx().content_rect().width() < COMPACT_WIDTH;
    // A menu left open when the window widens is simply gone: the list is a
    // column again.
    let open = compact && *menu_open;
    let showing: Pane = *pane;

    // Keep `#todo` in the address bar in step with the selection.
    use_effect(cx, current.name, || set_hash(current.name));

    rsx! {
        <View direction="column" grow={1.0} min_h={0.0} gap={8}>
            <Header compact={compact} on_menu={|| *menu_open = !*menu_open}/>
            if compact {
                // One pane, and the summary above it whichever it is.
                <View direction="column" grow={1.0} min_h={0.0} gap={4}>
                    <Text strong wrap>{current.summary}</Text>
                    match showing {
                        // The same `key` as the wide layout: switching
                        // examples sweeps the previous one's hooks. Switching
                        // to the code does too, so the example starts over
                        // when it comes back.
                        Pane::Example => {
                            <View key={current.name} direction="column" grow={1.0} min_h={0.0}>
                                <Running name={current.name} plain={showing_plain}/>
                            </View>
                        }
                        Pane::Code => {
                            <Code
                                w="100%"
                                grow={1.0}
                                min_h={0.0}
                                meta={*current}
                                plain={showing_plain}
                                on_pick={|pick: bool| *plain = pick}
                            />
                        }
                    }
                </View>
                // Not while the menu is down: it covers the corner the
                // button floats in, and nothing behind it should be pressed.
                if !open {
                    <PaneToggle showing={showing} on_toggle={|next: Pane| *pane = next}/>
                }
                <Menu
                    open={open}
                    shown={&shown}
                    active={&active}
                    selected={current.name}
                    on_select={|name: &'static str| {
                        *selected = name;
                        // A new example starts on its egui-react version.
                        *plain = false;
                        // Picked, so the menu has done its job.
                        *menu_open = false;
                    }}
                    on_tag={|tag: &'static str| toggle(&mut tags, tag)}
                    on_clear={|| tags.clear()}
                    on_close={|| *menu_open = false}
                />
            } else {
                <View direction="row" grow={1.0} min_h={0.0} gap={8}>
                    // `shrink={0}` so the list keeps its width when the window
                    // is narrow; the centre column is the one that gives way.
                    <List
                        w={200.0}
                        shrink={0.0}
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
                    // `min_w={0}` makes the centre the column that gives way:
                    // taffy may otherwise take a flex item's content as its
                    // automatic minimum, and a wide example would push the
                    // code column off the right edge instead of being cut off
                    // itself.
                    <View direction="column" grow={1.0} min_w={0.0} gap={4}>
                        // The summary, not the name: the name is already the
                        // label of the list button and two widgets with one
                        // label are ambiguous to a screen reader (and to
                        // kittest).
                        <Text strong>{current.summary}</Text>
                        // The `key` is the whole point: change it and the
                        // previous example's hooks are swept, so state does
                        // not leak across.
                        <View key={current.name} direction="column" grow={1.0}>
                            <Running name={current.name} plain={showing_plain}/>
                        </View>
                    </View>
                    <Separator vertical/>
                    // A fixed share of the window, never squeezed by what the
                    // running example wants: `shrink={0}` sends the whole
                    // overflow to the centre column, which is the one with
                    // `min_w={0}`.
                    <Code
                        w="40%"
                        min_w={360.0}
                        shrink={0.0}
                        meta={*current}
                        plain={showing_plain}
                        on_pick={|pick: bool| *plain = pick}
                    />
                </View>
            }
        </View>
    }
}

/// The page header: the title on the left and, on a compact screen, the menu
/// button on the right.
///
/// A wide window has the list in a column of its own, so it has nothing for
/// the button to open.
#[component]
fn Header(cx: &mut Cx, compact: bool, #[event] on_menu: ()) {
    rsx! {
        <View direction="row" align="center" gap={8} w="100%">
            <Text strong size={20.0} grow={1.0}>"egui-react"</Text>
            if compact {
                <Button label="menu" on_click={|| on_menu.emit(())}>"☰"</Button>
            }
        </View>
    }
}

/// The button floating in the bottom-right corner of a compact screen, which
/// swaps the running example for its code and back.
///
/// An `egui::Area` rather than a node of the tree: it sits over the pane,
/// where a thumb reaches, and takes no room from it. The label says what a
/// press does; the glyph is what is drawn.
#[component]
fn PaneToggle(cx: &mut Cx, showing: Pane, #[event] on_toggle: Pane) {
    let ctx = cx.ctx().clone();
    let (glyph, label, next) = match showing {
        Pane::Example => ("</>", "show code", Pane::Code),
        Pane::Code => ("⏵", "show example", Pane::Example),
    };
    let clicked = egui::Area::new(cx.scope_id().with("pane_toggle"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
        .show(&ctx, |ui| {
            egui::Frame::new()
                .shadow(ui.visuals().window_shadow)
                .corner_radius(24.0)
                .show(ui, |ui| {
                    ui.spacing_mut().button_padding = egui::vec2(16.0, 12.0);
                    let button = egui::Button::new(egui::RichText::new(glyph).size(18.0))
                        .corner_radius(24.0)
                        .wrap_mode(egui::TextWrapMode::Extend);
                    let response = ui.add(button);
                    // The glyph is not a name; see `<Button label>`.
                    ui.ctx()
                        .accesskit_node_builder(response.id, |node| node.set_label(label));
                    response.clicked()
                })
                .inner
        })
        .inner;
    if clicked {
        on_toggle.emit(next);
    }
}

/// The menu of a compact screen: the example list and the tag filter, over
/// the whole window.
///
/// It slides down from the top edge when it opens and back up when it closes,
/// and it is not drawn at all once it is away. While it is on screen it is an
/// `egui::Area` in the foreground that spans the window, so the pane beneath
/// gets no clicks. Inside, the list is laid out by a tree of its own, rooted
/// the way the runner roots the app, so it fills the sheet and the example
/// list scrolls in what the header leaves.
#[component]
fn Menu(
    cx: &mut Cx,
    open: bool,
    shown: &[&'static Meta],
    active: &[&'static str],
    selected: &'static str,
    #[event] on_select: &'static str,
    #[event] on_tag: &'static str,
    #[event] on_clear: (),
    #[event] on_close: (),
) {
    let (store, scope) = (cx.store, cx.scope_id());
    let ctx = cx.ctx().clone();
    // 0 when the menu is away, 1 when it is down; in between it is moving.
    let down = ctx.animate_bool_with_time_and_easing(
        scope.with("slide"),
        open,
        MENU_TIME,
        egui::emath::easing::cubic_out,
    );
    if down <= 0.0 {
        return;
    }
    let screen = ctx.content_rect();
    let top = screen.top() - screen.height() * (1.0 - down);
    let id = scope.with("menu");
    // Above the pane toggle and anything else in the foreground, every frame:
    // egui keeps the areas in the order they were first shown, and a click
    // brings one to the front.
    ctx.move_to_top(egui::LayerId::new(egui::Order::Foreground, id));
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(screen.left(), top))
        // Partly above the window while it slides.
        .constrain(false)
        .default_size(screen.size())
        .show(&ctx, move |ui| {
            let sheet = egui::Rect::from_min_size(ui.max_rect().min, screen.size());
            ui.painter()
                .rect_filled(sheet, 0.0, ui.visuals().panel_fill);
            // The whole sheet, not just the widgets on it, is what stops a
            // press from reaching the pane beneath.
            ui.allocate_rect(sheet, egui::Sense::click());
            let inner = sheet.shrink(8.0);
            let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(inner));
            let mut cx = Cx::new(store, &mut ui, scope);
            cx.root_container(scope.with("menu_root"), root_style(), |cx| {
                rsx! {
                    <View direction="column" gap={8} w="100%" h="100%">
                        // The same header, with the button that closes the
                        // menu where the one that opened it was.
                        <View direction="row" align="center" gap={8} w="100%">
                            <Text strong size={20.0} grow={1.0}>"egui-react"</Text>
                            <Button label="close menu" on_click={|| on_close.emit(())}>"×"</Button>
                        </View>
                        <List
                            w="100%"
                            grow={1.0}
                            min_h={0.0}
                            shown={shown}
                            active={active}
                            selected={selected}
                            on_select={|name: &'static str| on_select.emit(name)}
                            on_tag={|tag: &'static str| on_tag.emit(tag)}
                            on_clear={|| on_clear.emit(())}
                        />
                    </View>
                }
                .show(cx);
            });
        });
}

/// The example list and the tag filter.
///
/// The width is the caller's: a column of its own in the wide layout, the
/// whole sheet in the menu.
#[component]
fn List(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    shown: &[&'static Meta],
    active: &[&'static str],
    selected: &'static str,
    #[event] on_select: &'static str,
    #[event] on_tag: &'static str,
    #[event] on_clear: (),
) {
    rsx! {
        <View style={style} direction="column" gap={6}>
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
/// The width is the caller's, like [`List`]'s: 40% of the window in the wide
/// layout, all of it in the compact one.
///
/// `pub` for `tests/bench.rs`, which times this column on its own.
#[component]
pub fn Code(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    meta: Meta,
    plain: bool,
    #[event] on_pick: bool,
) {
    let file = if plain { "plain.rs" } else { "lib.rs" };
    let link = format!("{REPO}/{}/src/{file}", meta.name);
    // What is shown, and counted: the source minus the gallery's own plumbing.
    let (react, plain_source) = use_memo(cx, meta.name, || {
        (shown_source(meta.source), meta.plain.map(shown_source))
    });
    let source: &str = if plain {
        plain_source.as_deref().unwrap_or(react)
    } else {
        react
    };
    let mut lines = use_state(cx, || None::<(GalleyKey, CodeLines)>);
    let lines: &CodeLines = code_lines(cx.ui(), lines.bind(), (meta.name, plain), source);
    let react_lines = format!("{} lines", react.lines().count());
    let plain_lines = plain_source.as_deref().map_or(String::new(), |p| {
        format!("{} lines plain", p.lines().count())
    });

    rsx! {
        <View style={style} direction="column" gap={6}>
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
            // each frame, so a label per line keeps that to a screenful.
            // The row is a bare leaf, drawn straight into the list (a `<View>`
            // per row measured at twice the cost), so its height is the
            // galley's: every line's job asks for one row's height even when
            // the line is blank. The list is as wide as the widest line, so
            // the sideways scroll range is the same whichever rows are in view.
            <VirtualList
                grow={1.0}
                horizontal
                rows={lines.galleys.len()}
                row_h={lines.row_h}
                render={|cx: &mut Cx<'_, '_>, row: usize| {
                    let galley = lines.galleys[row].clone();
                    cx.leaf(&ItemStyle::default(), |ui| {
                        ui.set_min_width(lines.width);
                        ui.add(egui::Label::new(galley).selectable(true));
                    });
                }}
            />
        </View>
    }
}

/// The source as the code column shows it: the example, without the gallery's
/// own plumbing.
///
/// Dropped: the crate doc comment at the top (`//!` — design notes, for the
/// repository, not for a pane next to the running example), the `META` block
/// and its import, and anything between a `// gallery:hide` line and the next
/// `// gallery:show` (the standalone binary's root in list-10k, say). Runs of
/// blank lines that leaves behind are folded into one.
pub fn shown_source(source: &str) -> String {
    let mut out = String::new();
    let mut lines = source.lines().peekable();
    while lines.peek().is_some_and(|l| l.starts_with("//!")) {
        lines.next();
    }
    let mut hidden = false;
    let mut in_meta = false;
    let mut blank = true;
    for line in lines {
        match line.trim() {
            "// gallery:hide" => hidden = true,
            "// gallery:show" => hidden = false,
            _ if hidden => {}
            "use example_meta::Meta;" => {}
            _ if in_meta => in_meta = line != "};",
            _ if line.starts_with("pub const META: Meta = Meta {") => in_meta = true,
            "" if blank => {}
            _ => {
                blank = line.is_empty();
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out.truncate(out.trim_end().len());
    out.push('\n');
    out
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
/// `egui_extras::code_view_ui` highlights and lays out from scratch every
/// frame: it hashes the whole source for the highlight cache, then hashes the
/// `LayoutJob` (a section per token) for the galley cache. A `Label` handed an
/// `Arc<Galley>` does neither. The source is highlighted whole and the job cut
/// at each newline, so a block comment or a multi-line string is coloured the
/// same as it would be in one piece.
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
        let job = highlight::highlight(source, ui.visuals().dark_mode, font_id.clone());
        let (galleys, row_h) = ui.fonts_mut(|fonts| {
            let row_h = fonts.row_height(&font_id);
            let galleys: Vec<Arc<egui::Galley>> = split_lines(&job)
                .into_iter()
                .map(|mut job| {
                    // A blank line has no glyph to be tall by; the list still
                    // moves on by what this galley measures.
                    job.first_row_min_height = row_h;
                    fonts.layout_job(job)
                })
                .collect();
            (galleys, row_h)
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

    /// The header, the `META` block and the hidden region go; the rest stays,
    /// with no run of blank lines.
    #[test]
    fn shown_source_drops_the_plumbing() {
        let shown = shown_source(list_10k::META.source);
        assert!(!shown.contains("//!"), "{shown}");
        assert!(!shown.contains("pub const META"), "{shown}");
        assert!(!shown.contains("example_meta"), "{shown}");
        assert!(!shown.contains("fn Compare"), "{shown}");
        assert!(!shown.contains("fn PlainApp"), "{shown}");
        assert!(shown.contains("pub fn App"), "{shown}");
        assert!(shown.contains("pub fn Row"), "{shown}");
        assert!(!shown.contains("\n\n\n"), "{shown}");
        assert!(!shown.starts_with('\n'), "{shown:?}");
        assert!(shown.ends_with("}\n"), "{shown:?}");
    }

    /// Every line of the source comes out, in order, with its text intact.
    #[test]
    fn split_lines_keeps_every_line() {
        for meta in EXAMPLES {
            let job = highlight::highlight(meta.source, true, egui::FontId::monospace(12.0));
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
