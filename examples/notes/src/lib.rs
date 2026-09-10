//! A small notes app: most of the library, doing a job.
//!
//! Nothing here is new. It is the pieces the other examples show one at a time,
//! put together the way an application would put them:
//!
//! - `use_reducer` owns the notes and `use_persisted` keeps them across
//!   restarts. The reducer holds the state and the persisted slot mirrors it —
//!   a reducer cannot reduce *into* someone else's slot, so the two lines that
//!   copy one to the other are the price of having both.
//! - `use_memo` filters and sorts the list, keyed on the search text and a
//!   version of the notes, so it is not rebuilt sixty times a second.
//! - `provide_context` hands a `Theme` to both columns without either of the
//!   `<View>`s in between carrying it. The theme is egui's own — dark or
//!   light, whichever the window is in — read at the top and published once.
//! - `bind` for editing in place, `on_change` for the note's timestamp — the
//!   widget holds the only `&mut` to the text, so the clock is updated through
//!   a message rather than a second borrow.
//! - A font with Japanese in it. egui's own fonts have no CJK glyphs, so a
//!   note in Japanese would be boxes; `egui_react_app::fonts::Fonts` puts a
//!   subset of Noto Sans JP in front of egui's font (the `font` example is
//!   the long version of this).
//!
//! No `Panel`, so the gallery can run it in a column of its own.

use egui_react::prelude::*;
use egui_react_app::fonts::{FontSource, Fonts, Generic};
use egui_react_elements::prelude::*;
use example_meta::Meta;

pub mod notes;

use notes::{Msg, Note, Notes, reduce, visible};

pub const META: Meta = Meta {
    name: "notes",
    summary: "A notes app: reducer, persistence, context, memo and an editor, together.",
    hooks: &[
        "use_state",
        "use_reducer",
        "use_persisted",
        "use_memo",
        "use_effect",
        "use_handle",
        "provide_context",
        "use_context",
    ],
    elements: &[
        "View",
        "Text",
        "TextEdit",
        "Button",
        "Separator",
        "ScrollArea",
    ],
    source: include_str!("lib.rs"),
    plain: None,
};

/// Provided to the whole tree; the leaves read it with `use_context`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub dark: bool,
}

impl Theme {
    /// The colour the selected note and the headings are drawn in.
    pub fn accent(self) -> egui::Color32 {
        if self.dark {
            egui::Color32::from_rgb(0x7f, 0xb2, 0xf0)
        } else {
            egui::Color32::from_rgb(0x1c, 0x54, 0x9c)
        }
    }
}

/// How tall a note's row is.
const ROW_PAD: f32 = 6.0;

/// The subset of Noto Sans JP the `font` example ships: kana, the common
/// kanji, and Latin. 433 KB, so it is compiled in rather than fetched.
const NOTO_SANS_JP_SUBSET: &[u8] = include_bytes!("../../font/fonts/NotoSansJP-Subset.ttf");

/// One chain, `jp`: the subset first, then egui's own font for whatever the
/// subset lacks. Made the default proportional family, so every widget draws
/// with it and no `<Text font>` is needed.
pub fn fonts() -> Fonts {
    Fonts::new()
        .stack(
            "jp",
            [
                FontSource::Bundled(NOTO_SANS_JP_SUBSET),
                FontSource::Generic(Generic::SansSerif),
            ],
        )
        .default_proportional("jp")
}

#[component]
pub fn App(cx: &mut Cx) {
    // Applied once, on the first frame. `set_fonts` is per `egui::Context`,
    // so inside the gallery this restyles the gallery too, the way the `font`
    // example does; an app would do it from `Options::setup` instead.
    let ctx = cx.ctx().clone();
    use_effect(cx, (), move || fonts().apply(&ctx));

    rsx! {
        <Themed>
            <Notebook/>
        </Themed>
    }
}

/// Reads egui's theme and publishes it. The same shape as the `theme`
/// example: a provider has to make the value it provides, because a `Handle`
/// cannot be a prop.
///
/// The handle is written only when the theme changed. A write on every frame
/// would ask for a repaint on every frame, and the app would never go idle.
#[component(shares_ui)]
fn Themed(cx: &mut Cx, children: impl View) {
    let dark = cx.ctx().theme() == egui::Theme::Dark;
    let theme = use_handle(cx, || Theme { dark });
    if theme.get().dark != dark {
        theme.set(Theme { dark });
    }

    provide_context(cx, theme, |cx| children.show(cx));
}

#[component]
fn Notebook(cx: &mut Cx) {
    // The reducer owns the notes; the persisted slot is the copy that survives
    // a restart. `use_persisted` first, so its value is there to seed the
    // reducer on the very first frame.
    let mut saved = use_persisted(cx, "notes/notes", Notes::default);
    let (mut notes, dispatch) = use_reducer(cx, reduce, || saved.clone());
    if *saved != *notes {
        *saved = notes.clone();
    }

    let mut search = use_state(cx, String::new);
    let mut selected = use_state(cx, || None::<u64>);
    // The note whose title should take the caret, with its text selected:
    // the one `new` just made, until the editor has done it.
    let mut fresh = use_state(cx, || None::<u64>);

    let now = cx.ui().input(|i| i.time);
    let theme = use_context::<Theme>(cx).map_or(Theme { dark: true }, |theme| theme.get());

    // Filtering and sorting every frame would be wasted work. `next_id` stands
    // in for "the notes changed": it moves on every add, and the length covers
    // deletes. Editing a body sends `Touched`, which moves `updated` and so the
    // sort order, so that is in the deps too.
    let version = (notes.items.len(), notes.next_id);
    let touched = notes
        .items
        .iter()
        .map(|note| note.updated.to_bits())
        .fold(0u64, |acc, bits| acc ^ bits);
    let listed: &Vec<Note> = use_memo(cx, (search.as_str(), version, touched), || {
        visible(&notes, search.as_str())
    });

    // The selection follows the list: a note that was deleted or filtered away
    // is not selectable, and there is always something open if there is
    // anything to open.
    let current = selected
        .filter(|id| listed.iter().any(|note| note.id == *id))
        .or_else(|| listed.first().map(|note| note.id));
    if *selected != current {
        *selected = current;
    }
    // Read once: an element may not hold a shared borrow of a state *and* a
    // handler that writes it (ARCHITECTURE 3.7).
    let focus_title = current.is_some() && *fresh == current;

    rsx! {
        <View direction="row" grow={1.0} w="100%" h="100%">
            <Sidebar
                notes={listed}
                selected={&current}
                search={search.bind()}
                theme={theme}
                on_select={|id: u64| *selected = Some(id)}
                on_new={|| {
                    dispatch.send(Msg::Add { now });
                    // Clearing the selection makes the newest note open
                    // itself: the rule below picks the first of the list
                    // whenever nothing valid is selected. The reducer numbers
                    // notes from `next_id`, so the new one's id is known here.
                    *selected = None;
                    *fresh = Some(notes.next_id + 1);
                }}
            />

            <Separator vertical/>

            <Editor
                notes={notes.bind()}
                selected={&current}
                now={now}
                theme={theme}
                focus_title={focus_title}
                on_touch={|id: u64| dispatch.send(Msg::Touched { id, now })}
                on_delete={|id: u64| dispatch.send(Msg::Delete { id })}
                on_focused={|| *fresh = None}
            />

        </View>
    }
}

/// The list, the search box, and the button that makes a note.
#[component]
fn Sidebar(
    cx: &mut Cx,
    notes: &[Note],
    // `&Option<..>`, not `Option<..>`: an `Option` prop is the optional kind,
    // whose setter takes the inner value and defaults to `None`. A reference
    // is an ordinary prop that happens to hold one.
    selected: &Option<u64>,
    search: &mut String,
    theme: Theme,
    #[event] on_select: u64,
    #[event] on_new: (),
) {
    rsx! {
        <View direction="column" w={220.0} shrink={0.0} h="100%" gap={6} p={8}>
            <View direction="row" gap={6} align="center" w="100%">
                <Text grow={1.0} strong color={theme.accent()}>"notes"</Text>
                <Button on_click={|| on_new.emit(())}>"new"</Button>
            </View>

            <TextEdit w="100%" bind={search} hint="search"/>
            <Separator/>

            <ScrollArea grow={1.0}>
                <View direction="column" w="100%">
                    for note in notes.iter() {
                        // A selectable row: `egui-react-elements` has no toggle
                        // element, so this is the escape hatch, one leaf deep.
                        {view(|cx| {
                            let picked = cx.leaf(&ItemStyle::default().w("100%").my(ROW_PAD), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.selectable_label(
                                    Some(note.id) == *selected,
                                    note.title.as_str(),
                                )
                                .clicked()
                            });
                            if picked {
                                on_select.emit(note.id);
                            }
                        })}
                    }
                    if notes.is_empty() {
                        <Text>"no notes"</Text>
                    }
                </View>
            </ScrollArea>
        </View>
    }
}

/// The open note: its title, its body, and what the app knows about it.
#[component]
#[allow(clippy::too_many_arguments)]
fn Editor(
    cx: &mut Cx,
    notes: &mut Notes,
    selected: &Option<u64>,
    now: f64,
    theme: Theme,
    // Put the caret in the title with the text selected, this frame, and say
    // so: a new note is named first.
    focus_title: bool,
    #[event] on_touch: u64,
    #[event] on_delete: u64,
    #[event] on_focused: (),
) {
    let Some(id) = *selected else {
        return rsx! {
            <View direction="column" grow={1.0} p={12}>
                <Text>"nothing open. make a note."</Text>
            </View>
        }
        .show(cx);
    };
    let Some(index) = notes.items.iter().position(|note| note.id == id) else {
        return;
    };

    let words = notes.items[index].words();
    let age = (now - notes.items[index].updated).max(0.0);

    rsx! {
        <View direction="column" grow={1.0} min_w={0.0} h="100%" gap={8} p={12}>
            <View direction="row" gap={8} align="center" w="100%">
                // The title, by hand rather than `<TextEdit bind>`: the one
                // thing the element cannot do is take the focus on request,
                // and a new note wants its name typed before anything else.
                // The field still holds the only `&mut` to the title, so the
                // edit is recorded by a message, not by a second borrow.
                {view(|cx| {
                    let style = ItemStyle::default().grow(1.0).min_w(0.0);
                    let response = cx.leaf(&style, |ui| {
                        let title = &mut notes.items[index].title;
                        let response = ui.add(
                            egui::TextEdit::singleline(title).desired_width(ui.available_width()),
                        );
                        if focus_title {
                            response.request_focus();
                            let mut state = egui::TextEdit::load_state(ui.ctx(), response.id)
                                .unwrap_or_default();
                            let end = egui::text::CCursor::new(title.chars().count());
                            let all =
                                egui::text::CCursorRange::two(egui::text::CCursor::new(0), end);
                            state.cursor.set_char_range(Some(all));
                            state.store(ui.ctx(), response.id);
                        }
                        response
                    });
                    if focus_title {
                        on_focused.emit(());
                    }
                    if response.changed() {
                        on_touch.emit(id);
                    }
                })}
                <Button on_click={|| on_delete.emit(id)}>"delete"</Button>
            </View>

            <View direction="row" gap={12} align="center" w="100%">
                <Text color={theme.accent()}>{format!("{words} words")}</Text>
                <Text>{format!("updated {age:.0}s ago")}</Text>
            </View>

            // The body: `egui::ScrollArea` around `egui::TextEdit`, by hand.
            // `<TextEdit multiline>` on its own is as tall as its text, and a
            // long note would run off the bottom of the window; here the
            // field is as tall as the column and a longer note scrolls.
            // `leaf_fill` with `h={0}` beside `grow`, because a scroll area
            // reports "all the room there is" as its size, and the column
            // would otherwise be that tall plus the rows above it.
            {view(|cx| {
                let style = ItemStyle::default().grow(1.0).h(0.0).w("100%");
                let changed = cx.leaf_fill(&style, |ui| {
                    // As many rows as fit, so a short note still gets the
                    // whole column to be typed into.
                    let row = ui.text_style_height(&egui::TextStyle::Body);
                    let rows = ((ui.available_height() - 8.0) / row).max(1.0) as usize;
                    egui::ScrollArea::vertical()
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut notes.items[index].body)
                                    .hint_text("write something")
                                    .desired_rows(rows)
                                    .desired_width(f32::INFINITY),
                            )
                            .changed()
                        })
                        .inner
                });
                if changed {
                    on_touch.emit(id);
                }
            })}
        </View>
    }
}
