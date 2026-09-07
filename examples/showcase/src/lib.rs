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
//!   `<View>`s in between carrying it.
//! - A `<Window>` for settings, opened by a button, with a two-step confirm
//!   that is just an `if` in the middle of the tree.
//! - `bind` for editing in place, `on_change` for the note's timestamp — the
//!   widget holds the only `&mut` to the text, so the clock is updated through
//!   a message rather than a second borrow.
//!
//! No `Panel`, so the gallery can run it in a column of its own.

use egui_react::prelude::*;
use egui_react_elements::prelude::*;
use example_meta::Meta;

pub mod notes;

use notes::{Msg, Note, Notes, reduce, visible};

pub const META: Meta = Meta {
    name: "showcase",
    summary: "A notes app: reducer, persistence, context, memo and a settings window, together.",
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
        "Checkbox",
        "Separator",
        "ScrollArea",
        "Window",
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

/// How tall a note's row is, with and without `compact rows`.
const ROW_PAD: f32 = 6.0;
const ROW_PAD_COMPACT: f32 = 1.0;

#[component]
pub fn App(cx: &mut Cx) {
    rsx! {
        <Themed>
            <Notebook/>
        </Themed>
    }
}

/// Owns the theme and publishes it. The same shape as the `theme` example: a
/// provider has to make the value it provides, because a `Handle` cannot be a
/// prop.
#[component(shares_ui)]
fn Themed(cx: &mut Cx, children: impl View) {
    let theme = use_handle(cx, || Theme { dark: true });
    let dark = theme.get().dark;

    let ctx = cx.ctx().clone();
    use_effect(cx, dark, move || {
        ctx.set_visuals(if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
    });

    provide_context(cx, theme, |cx| children.show(cx));
}

#[component]
fn Notebook(cx: &mut Cx) {
    // The reducer owns the notes; the persisted slot is the copy that survives
    // a restart. `use_persisted` first, so its value is there to seed the
    // reducer on the very first frame.
    let mut saved = use_persisted(cx, "showcase/notes", Notes::default);
    let (mut notes, dispatch) = use_reducer(cx, reduce, || saved.clone());
    if *saved != *notes {
        *saved = notes.clone();
    }

    let mut search = use_state(cx, String::new);
    let mut selected = use_state(cx, || None::<u64>);
    let mut settings_open = use_state(cx, || false);
    let mut compact = use_state(cx, || false);
    let mut confirming = use_state(cx, || false);

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

    rsx! {
        <View direction="row" grow={1.0} w="100%" h="100%">
            <Sidebar
                notes={listed}
                selected={&current}
                search={search.bind()}
                compact={*compact}
                theme={theme}
                on_select={|id: u64| *selected = Some(id)}
                on_new={|| {
                    dispatch.send(Msg::Add { now });
                    // Clearing the selection makes the newest note open
                    // itself: the rule below picks the first of the list
                    // whenever nothing valid is selected.
                    *selected = None;
                }}
                on_settings={|| *settings_open = true}
            />

            <Separator vertical/>

            <Editor
                notes={notes.bind()}
                selected={&current}
                now={now}
                theme={theme}
                on_touch={|id: u64| dispatch.send(Msg::Touched { id, now })}
                on_delete={|id: u64| dispatch.send(Msg::Delete { id })}
            />

            <Window
                title="settings"
                open={settings_open.bind()}
                default_pos={egui::pos2(220.0, 120.0)}
                default_size={egui::vec2(220.0, 120.0)}
            >
                <View direction="column" gap={8}>
                    <Checkbox bind={compact.bind()} label="compact rows"/>
                    <Separator/>
                    // A two-step confirm is an `if` in the middle of the tree.
                    if *confirming {
                        <Text>{format!("delete all {} notes?", notes.items.len())}</Text>
                        <View direction="row" gap={8}>
                            <Button on_click={|| {
                                dispatch.send(Msg::ClearAll);
                                *confirming = false;
                            }}>"yes, clear all"</Button>
                            <Button on_click={|| *confirming = false}>"cancel"</Button>
                        </View>
                    } else {
                        <Button on_click={|| *confirming = true}>"clear all"</Button>
                    }
                </View>
            </Window>
        </View>
    }
}

/// The list, the search box, and the buttons that make and configure notes.
#[component]
#[allow(clippy::too_many_arguments)]
fn Sidebar(
    cx: &mut Cx,
    notes: &[Note],
    // `&Option<..>`, not `Option<..>`: an `Option` prop is the optional kind,
    // whose setter takes the inner value and defaults to `None`. A reference
    // is an ordinary prop that happens to hold one.
    selected: &Option<u64>,
    search: &mut String,
    compact: bool,
    theme: Theme,
    #[event] on_select: u64,
    #[event] on_new: (),
    #[event] on_settings: (),
) {
    let pad = if compact { ROW_PAD_COMPACT } else { ROW_PAD };

    rsx! {
        <View direction="column" w={220.0} shrink={0.0} gap={6} p={8}>
            <View direction="row" gap={6} align="center" w="100%">
                <Text grow={1.0} strong color={theme.accent()}>"notes"</Text>
                <Button on_click={|| on_new.emit(())}>"new"</Button>
                <Button on_click={|| on_settings.emit(())}>"settings"</Button>
            </View>

            <TextEdit w="100%" bind={search} hint="search"/>
            <ThemeToggle/>
            <Separator/>

            <ScrollArea grow={1.0}>
                <View direction="column" w="100%">
                    for note in notes.iter() {
                        // A selectable row: `egui-react-elements` has no toggle
                        // element, so this is the escape hatch, one leaf deep.
                        {view(|cx| {
                            let picked = cx.leaf(&ItemStyle::default().w("100%").my(pad), |ui| {
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

/// Reads the theme from the context and writes it back, the way the `theme`
/// example's `Toggles` does.
#[component]
fn ThemeToggle(cx: &mut Cx) {
    let Some(theme) = use_context::<Theme>(cx) else {
        return;
    };
    let dark = theme.get().dark;
    rsx! {
        <Button on_click={move || theme.set(Theme { dark: !dark })}>
            {if dark { "light theme" } else { "dark theme" }}
        </Button>
    }
}

/// The open note: its title, its body, and what the app knows about it.
#[component]
fn Editor(
    cx: &mut Cx,
    notes: &mut Notes,
    selected: &Option<u64>,
    now: f64,
    theme: Theme,
    #[event] on_touch: u64,
    #[event] on_delete: u64,
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
                // `bind` hands the field the only `&mut` to the title, so the
                // handler that records the edit cannot touch it as well. It
                // sends a message instead.
                <TextEdit
                    grow={1.0}
                    bind={&mut notes.items[index].title}
                    on_change={|| on_touch.emit(id)}
                />
                <Button on_click={|| on_delete.emit(id)}>"delete"</Button>
            </View>

            // The counts go above the editor, not below it. A filling field
            // measures itself as "all the room there is", so anything after it
            // in the column is pushed past the bottom of the window.
            <View direction="row" gap={12} align="center" w="100%">
                <Text color={theme.accent()}>{format!("{words} words")}</Text>
                <Text>{format!("updated {age:.0}s ago")}</Text>
            </View>

            <TextEdit
                multiline
                grow={1.0}
                w="100%"
                bind={&mut notes.items[index].body}
                hint="write something"
                on_change={|| on_touch.emit(id)}
            />
        </View>
    }
}
