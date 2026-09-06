//! A kanban board: cards that keep their own state while they move.
//!
//! Every other example holds its state in one place and draws it. This one is
//! about the state that belongs to the *items* — the half-typed title, the
//! body someone opened to read — and about what happens to it when the items
//! are reordered, moved to another column, filtered away and brought back.
//!
//! Four things are on show, in the order they matter:
//!
//! - **A card owns its editing draft and its expanded flag.** They are
//!   `use_identity` hooks inside `<Card>`, not entries in a map somewhere
//!   above it. Drag a card someone is editing into another column and the
//!   draft goes with it, because the state is keyed by the card's id rather
//!   than by where the card is drawn (`hooks::use_identity` explains why that
//!   is not the same as `key=`). A card that is deleted — or filtered out of
//!   view, which unmounts it just the same — has its state dropped by the
//!   pass-end sweep, which is the housekeeping `plain.rs` writes out by hand.
//! - **Components of our own, composed.** `<Toolbar>`, `<Column>`, `<Card>`,
//!   `<Chip>` and `<IconButton>` each take a `style: ItemStyle`, so the caller
//!   decides where they sit, and report what the user did with `#[event]`.
//! - **Behaviour in custom hooks.** Undo/redo, the search debounce and the
//!   whole drag session are in [`hooks`], written against the same public API
//!   an application has. None of them needed a change to the library.
//! - **The usual reducer, with the usual company.** `use_reducer` (inside
//!   `use_undoable`) owns the board, `use_persisted` keeps it across restarts,
//!   `use_memo` filters each column, and `provide_context` hands the theme, the
//!   drag session and the `Dispatch` to the tree without the `<View>`s in
//!   between carrying any of them.
//!
//! Which way something travels is a decision, and it is made twice here. What
//! a component *did* goes up as an event: `<Card>` tells its column that the
//! user saved an edit, `<Chip>` tells whoever drew it that it was clicked. A
//! card has never heard of a board message and could be used somewhere else.
//! What the whole tree shares comes down through the context: the theme, the
//! drag in progress, and the `Dispatch` a column turns those events into
//! messages with — which is what saves the four `<Column>`s in the loop from
//! carrying four callbacks each.
//!
//! `plain.rs` is the same board in plain egui, and the two are driven by the
//! same test.
//!
//! No `Panel`, so the gallery can run it in a column of its own.

use example_meta::Meta;
use react_egui::prelude::*;
use react_egui_elements::prelude::*;

pub mod board;
pub mod hooks;
pub mod look;
pub mod plain;

use board::{
    Board, Card as CardData, CardId, Column as ColumnData, ColumnId, DropTarget, Msg, reduce,
    visible,
};
// Shadows the `<Label>` element, which this example does not use: here a label
// is the colour tag on a card.
use board::Label;
use hooks::{Dnd, Undoable, use_debounced, use_dnd, use_identity, use_undoable};
use look::{Theme, ghost};

pub const META: Meta = Meta {
    name: "board",
    summary: "Cards that keep their own editing state while they are dragged between columns.",
    hooks: &[
        "use_state",
        "use_reducer",
        "use_persisted",
        "use_memo",
        "use_effect",
        "use_handle",
        "provide_context",
        "use_context",
        "#[hook]",
    ],
    elements: &[
        "View",
        "Text",
        "TextEdit",
        "Button",
        "ScrollArea",
        "Frame",
        "Separator",
    ],
    source: include_str!("lib.rs"),
    plain: Some(include_str!("plain.rs")),
};

/// The drag session: a `CardId` is picked up, a [`DropTarget`] is where it
/// would land. Named once, because both the type and `use_context` need it.
pub type CardDnd = Dnd<CardId, DropTarget>;

/// What the toolbar, the columns and the cards all send to the reducer.
///
/// `Dispatch` is `Clone + Send + 'static`, which is what makes this the half of
/// the state that can travel by context: a `State` guard borrows the store and
/// could not be put in a `Handle` at all (ARCHITECTURE 3.5). Data goes down as
/// props, changes come back through here — the shape React ends up with too.
pub type Actions = Dispatch<Undoable<Msg>>;

/// How long the search box has to be quiet before the columns are refiltered.
const DEBOUNCE: f64 = 0.3;

/// The height of a column's footer, which is also the "drop it at the end"
/// zone. Big enough to aim at with a card in hand.
const FOOTER_H: f32 = 26.0;

/// How far outside a card's title row still counts as that card, so that the
/// gaps between cards are not dead ground during a drag.
const SLOT_PAD: f32 = 4.0;

/// The theme, or the dark one if this is drawn outside a provider.
///
/// A three-line `#[hook]`, because six components ask the same question.
#[hook]
fn use_theme(cx: &mut Cx) -> Theme {
    use_context::<Theme>(cx).map_or(Theme::DARK, |theme| theme.get())
}

/// The drag session, or an unattached one outside a provider.
#[hook]
fn use_drag(cx: &mut Cx) -> CardDnd {
    use_context::<CardDnd>(cx).map_or_else(CardDnd::new, |dnd| dnd.get())
}

/// Send one message to the board's reducer, if this is drawn under one.
fn send(actions: &Option<Actions>, msg: Msg) {
    if let Some(actions) = actions {
        actions.send(Undoable::Do(msg));
    }
}

#[component]
pub fn App(cx: &mut Cx) {
    rsx! {
        <BoardProvider>
            <BoardView/>
        </BoardProvider>
    }
}

/// Owns the theme and the drag session and publishes both.
///
/// A provider has to make the value it provides: `provide_context` takes a
/// `Handle`, which borrows the store, and a prop may not name that lifetime
/// (ARCHITECTURE 6). The same shape as the `theme` example.
#[component(shares_ui)]
fn BoardProvider(cx: &mut Cx, children: impl View) {
    let theme = use_handle(cx, || Theme::DARK);
    // One session for the whole tree: the card that is picked up and the
    // column it lands in are in different branches of it.
    let dnd = use_dnd::<CardId, DropTarget>(cx);

    let dark = theme.get().dark;
    let ctx = cx.ctx().clone();
    use_effect(cx, dark, move || {
        ctx.set_visuals(if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
    });

    provide_context(cx, theme, |cx| {
        provide_context(cx, dnd, |cx| children.show(cx))
    });
}

/// The board: the toolbar, the columns, and the reducer behind them.
#[component]
fn BoardView(cx: &mut Cx) {
    // The reducer owns the board and the persisted slot mirrors it, the way
    // `showcase` does. `use_persisted` first, so its value is there to seed the
    // history on the very first frame.
    let mut saved = use_persisted(cx, "board/board", Board::demo);
    let (history, dispatch) = use_undoable(cx, reduce, || saved.clone());
    if *saved != history.present {
        *saved = history.present.clone();
    }
    // The `Dispatch` is what goes into the context; see [`Actions`].
    let actions = use_handle(cx, || dispatch.clone());

    let mut search = use_state(cx, String::new);
    let mut label = use_state(cx, || None::<Label>);
    // Read once: an element may not hold a shared borrow of a state *and* a
    // handler that writes it (ARCHITECTURE 3.7), and the toolbar does both.
    let filter = *label;
    // The box types on every keystroke; the columns filter on this instead.
    let live = search.clone();
    let query = use_debounced(cx, &live, DEBOUNCE);

    // The provider owns the theme; the toolbar's button writes it back through
    // the same handle, the way the `theme` example's toggles do.
    let theme_handle = use_context::<Theme>(cx);
    let theme = theme_handle.map_or(Theme::DARK, |theme| theme.get());
    let dnd = use_drag(cx);

    // A drag that ended over a slot becomes exactly one message, which is what
    // makes it exactly one step of the undo history.
    if let Some((card, target)) = dnd.take_drop() {
        let to_index = history.present.drop_index(card, target);
        dispatch.send(Undoable::Do(Msg::MoveCard {
            card,
            to_column: target.column,
            to_index,
        }));
    }

    let board = &history.present;
    let carried = dnd.carrying().and_then(|id| board.card(id));

    let view = rsx! {
        <View direction="column" grow={1.0} w="100%" h="100%" gap={8} p={8}>
            <Toolbar
                search={search.bind()}
                label={&filter}
                count={board.len()}
                can_undo={history.can_undo()}
                can_redo={history.can_redo()}
                on_label={|picked: Label| {
                    // Clicking the chip that is already on clears the filter.
                    *label = (*label != Some(picked)).then_some(picked);
                }}
                on_undo={|| dispatch.send(Undoable::Undo)}
                on_redo={|| dispatch.send(Undoable::Redo)}
                on_theme={move || {
                    if let Some(handle) = theme_handle {
                        handle.set(Theme { dark: !theme.dark });
                    }
                }}
            />
            <Separator/>
            <View direction="row" grow={1.0} min_h={0.0} w="100%" gap={8}>
                for column in board.columns.iter() {
                    // The key is the column's id, so a column keeps its own
                    // scroll position and rename box.
                    <Column
                        key={column.id}
                        grow={1.0}
                        min_w={0.0}
                        column={column}
                        rev={board.rev}
                        search={query.as_str()}
                        label={&filter}
                        on_add={|| dispatch.send(Undoable::Do(Msg::AddCard { column: column.id }))}
                    />
                }
            </View>
        </View>
    };
    provide_context(cx, actions, |cx| view.show(cx));

    // The card in hand, drawn on the top layer at the pointer. Painted rather
    // than built out of widgets: a widget here would put a second copy of the
    // card's title into the accessibility tree, where a screen reader — and
    // `kittest` — would find two of everything.
    if let (Some(card), Some(at)) = (carried, dnd.pointer()) {
        ghost(cx.ctx(), at, &card.title, theme);
    }
}

/// The search box, the label filter, the counts and the history buttons.
///
/// Everything here is the *board's* state rather than the card's, so the
/// toolbar owns none of it: it is handed what to show and reports what was
/// pressed. Undo and redo are events too — the toolbar does not know that a
/// board message exists.
#[component]
#[allow(clippy::too_many_arguments)]
fn Toolbar(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    search: &mut String,
    // `&Option<Label>`, not `Option<Label>`: an `Option` prop is the *optional*
    // kind, whose setter takes the inner value and defaults to `None`.
    label: &Option<Label>,
    count: usize,
    can_undo: bool,
    can_redo: bool,
    #[event] on_label: Label,
    #[event] on_undo: (),
    #[event] on_redo: (),
    #[event] on_theme: (),
) {
    let theme = use_theme(cx);

    rsx! {
        <View style={style} direction="row" w="100%" gap={6} align="center">
            <Text size={20.0} strong color={theme.accent()}>"board"</Text>
            <TextEdit w={160.0} bind={search} hint="search"/>
            for tag in Label::ALL {
                <Chip
                    key={tag.name()}
                    label={tag.name()}
                    color={theme.label(tag)}
                    active={*label == Some(tag)}
                    on_click={|| on_label.emit(tag)}
                />
            }
            <Text grow={1.0}>{format!("{count} cards")}</Text>
            <IconButton enabled={can_undo} on_click={|| on_undo.emit(())}>"undo"</IconButton>
            <IconButton enabled={can_redo} on_click={|| on_redo.emit(())}>"redo"</IconButton>
            <IconButton on_click={|| on_theme.emit(())}>
                {if theme.dark { "light" } else { "dark" }}
            </IconButton>
        </View>
    }
}

/// One column: a name that can be renamed in place, a count, its cards, and a
/// footer that adds one.
///
/// The column reads the `Dispatch` from the context and turns its cards'
/// events into messages. It is the innermost place that knows a column id, and
/// catching the events here rather than passing them on saves the loop above
/// from threading four callbacks through every `<Column>` it writes.
#[component]
fn Column(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    column: &ColumnData,
    // The board's revision, for the memo below: hashing every card on every
    // frame would cost more than the filtering it saves.
    rev: u64,
    search: &str,
    label: &Option<Label>,
    #[event] on_add: (),
) {
    let theme = use_theme(cx);
    let dnd = use_drag(cx);
    let actions = use_context::<Actions>(cx).map(|actions| actions.get());

    let mut renaming = use_state(cx, || false);
    let mut draft = use_state(cx, || column.name.clone());

    let shown: &Vec<CardId> = use_memo(cx, (column.id, rev, search, label), || {
        visible(column, search, *label)
    });
    // Each card with the one below it: dropping on the lower half of a card
    // means "in front of whatever comes next".
    let cards: Vec<&CardData> = shown
        .iter()
        .filter_map(|id| column.cards.iter().find(|card| card.id == *id))
        .collect();
    let after: Vec<Option<CardId>> = (0..cards.len())
        .map(|i| cards.get(i + 1).map(|card| card.id))
        .collect();

    let hovered_end = dnd.hovered()
        == Some(DropTarget {
            column: column.id,
            before: None,
        });

    rsx! {
        <View style={style} direction="column" gap={6}>
            <View direction="row" w="100%" gap={4} align="center">
                if *renaming {
                    <TextEdit
                        grow={1.0}
                        bind={draft.bind()}
                        on_submit={|name: String| {
                            send(&actions, Msg::RenameColumn { column: column.id, name });
                            *renaming = false;
                        }}
                    />
                } else {
                    <Text grow={1.0} strong color={theme.accent()}>{column.name.as_str()}</Text>
                    <IconButton on_click={|| {
                        *draft = column.name.clone();
                        *renaming = true;
                    }}>"rename"</IconButton>
                }
                <Text>{format!("{}/{}", cards.len(), column.cards.len())}</Text>
            </View>

            // Everything that scrolls, including the footer: a `ScrollArea` is
            // a `leaf_fill`, so it measures as all the height there is rather
            // than as what its siblings left over (plan.md section 8). Below
            // it, a footer would be pushed off the bottom of the window; after
            // the last card, it is where the eye is anyway.
            <ScrollArea grow={1.0}>
                <View direction="column" w="100%" gap={6} pr={4}>
                    for (i, card) in cards.iter().enumerate() {
                        // The key is the card's id. It is what tells two cards
                        // apart inside this column — and, together with
                        // `use_identity` in the card itself, what makes a
                        // card's own state follow it out of this column.
                        <Card
                            key={card.id}
                            card={card}
                            column={column.id}
                            next={&after[i]}
                            on_edit={|(title, body): (String, String)| {
                                send(&actions, Msg::EditCard { card: card.id, title, body });
                            }}
                            on_label={|label: Label| {
                                send(&actions, Msg::SetLabel { card: card.id, label });
                            }}
                            on_remove={|| send(&actions, Msg::RemoveCard { card: card.id })}
                        />
                    }
                    if cards.is_empty() {
                        <Text>"nothing here"</Text>
                    }

                    // The footer is both the "add a card" button and the place
                    // a drag ends when it means "at the end of this column".
                    // One leaf, so the rectangle offered to the drag session is
                    // the one the eye sees.
                    {view(|cx| {
                        let (rect, clicked) = cx.leaf_fill(
                            &ItemStyle::default().w("100%").h(FOOTER_H),
                            |ui| {
                                let rect = ui.max_rect();
                                if hovered_end {
                                    let stroke = egui::Stroke::new(2.0, theme.accent());
                                    ui.painter().hline(rect.x_range(), rect.top(), stroke);
                                }
                                let button = egui::Button::new("+ card")
                                    .wrap_mode(egui::TextWrapMode::Extend);
                                (rect, ui.add_sized(rect.size(), button).clicked())
                            },
                        );
                        dnd.slot(rect, DropTarget { column: column.id, before: None });
                        if clicked {
                            on_add.emit(());
                        }
                    })}
                </View>
            </ScrollArea>
        </View>
    }
}

/// One card: a tag, a title that is also the drag handle, and — the point of
/// the example — state of its own.
///
/// `editing`, `draft` and `expanded` belong to *this card*. Nothing above it
/// knows they exist, nothing has to make room for them when a card is added,
/// and nothing has to clean up after them when one is deleted.
#[component]
fn Card(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    card: &CardData,
    column: ColumnId,
    // The card below this one; `None` at the end of the column.
    next: &Option<CardId>,
    #[event] on_edit: (String, String),
    #[event] on_label: Label,
    #[event] on_remove: (),
) {
    let theme = use_theme(cx);
    let dnd = use_drag(cx);

    // Keyed by the card, not by where the card is: `key=` distinguishes
    // siblings under one parent, and a card that moves changes parents. See
    // `hooks::use_identity`.
    let mut editing = use_identity(cx, (card.id, "editing"), || false);
    let mut expanded = use_identity(cx, (card.id, "expanded"), || false);
    let mut draft = use_identity(cx, (card.id, "draft"), || Draft::of(card));

    let before_me = dnd.hovered()
        == Some(DropTarget {
            column,
            before: Some(card.id),
        });
    let carried = dnd.carrying() == Some(card.id);
    let open = *editing;

    rsx! {
        <Frame
            style={style}
            w="100%"
            inner_margin={6.0}
            corner_radius={4.0}
            fill={theme.card()}
        >
            <View direction="column" w="100%" gap={4}>
                <View direction="row" w="100%" gap={6} align="center">
                    <Chip
                        label={card.label.name()}
                        color={theme.label(card.label)}
                        on_click={|| on_label.emit(card.label.next())}
                    />

                    // The title is the drag handle, and one leaf is all a drag
                    // needs: a rectangle to sense in and a rectangle to offer
                    // as a target. `<Text>` would draw the same thing but hand
                    // back no `Response`.
                    {view(|cx| {
                        let response = cx.leaf(
                            &ItemStyle::default().grow(1.0).min_w(0.0),
                            |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                let mut text = egui::RichText::new(card.title.as_str());
                                if carried {
                                    text = text.weak();
                                }
                                let label = egui::Label::new(text)
                                    .sense(egui::Sense::click_and_drag());
                                let response = ui.add(label);
                                if before_me {
                                    let stroke = egui::Stroke::new(2.0, theme.accent());
                                    let rect = response.rect.expand(SLOT_PAD);
                                    ui.painter().hline(rect.x_range(), rect.top(), stroke);
                                }
                                response
                            },
                        );

                        if response.drag_started() {
                            dnd.pick_up(card.id);
                        }
                        if response.clicked() {
                            *expanded = !*expanded;
                        }
                        // Half of the row means "in front of me", half means
                        // "in front of the next one", which is how a list of
                        // cards is also a list of gaps between cards.
                        let (top, bottom) = response
                            .rect
                            .expand(SLOT_PAD)
                            .split_top_bottom_at_fraction(0.5);
                        dnd.slot(top, DropTarget { column, before: Some(card.id) });
                        dnd.slot(bottom, DropTarget { column, before: *next });
                    })}

                    <IconButton on_click={|| {
                        // Opening takes a fresh copy of the saved text, so
                        // closing the editor and opening it again starts from
                        // what was saved rather than from an old draft.
                        if !*editing {
                            *draft = Draft::of(card);
                        }
                        *editing = !*editing;
                    }}>{if open { "close" } else { "edit" }}</IconButton>
                    <IconButton on_click={|| on_remove.emit(())}>"x"</IconButton>
                </View>

                if open {
                    // `bind` hands the field the only `&mut` to the draft, so
                    // an `on_change` here could not touch it as well; the save
                    // button is a different element, and reads it freely.
                    <TextEdit w="100%" bind={&mut draft.bind().title} hint="title"/>
                    <TextEdit
                        multiline
                        w="100%"
                        h={54.0}
                        bind={&mut draft.bind().body}
                        hint="body"
                    />
                    <View direction="row" gap={4}>
                        <Button on_click={|| {
                            on_edit.emit((draft.title.clone(), draft.body.clone()));
                            *editing = false;
                        }}>"save"</Button>
                        <Button on_click={|| *editing = false}>"cancel"</Button>
                    </View>
                } else if *expanded && !card.body.is_empty() {
                    <Text w="100%" wrap>{card.body.as_str()}</Text>
                }
            </View>
        </Frame>
    }
}

/// What a card's editor is holding before it is saved.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Draft {
    title: String,
    body: String,
}

impl Draft {
    fn of(card: &CardData) -> Self {
        Self {
            title: card.title.clone(),
            body: card.body.clone(),
        }
    }
}

/// A coloured tag: the label on a card, and the filter in the toolbar.
///
/// `react-egui-elements` has no chip and no toggle, so this is the escape
/// hatch, one leaf deep — and it is also the smallest example of the shape
/// every component here has: take a `style`, draw one thing, report the click.
#[component]
fn Chip(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    label: &str,
    color: egui::Color32,
    #[prop(default)] active: bool,
    #[event] on_click: (),
) {
    let clicked = cx.leaf(&style.shrink(0.0), |ui| {
        // A hand-written leaf sets the wrap mode itself: measured in the
        // zero-width `Ui` of its first draw, a wrapping widget reports one
        // character wide and taffy keeps it that way (ARCHITECTURE 6).
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        let text = egui::RichText::new(label).small().color(color);
        ui.selectable_label(active, text).clicked()
    });
    if clicked {
        on_click.emit(());
    }
}

/// A small flat button, for the things a card and a column do to themselves.
#[component]
fn IconButton(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    #[prop(default = true)] enabled: bool,
    #[event] on_click: (),
    children: impl Into<egui::WidgetText>,
) {
    let clicked = cx.leaf(&style.shrink(0.0), |ui| {
        let button = egui::Button::new(children)
            .small()
            .frame(false)
            .wrap_mode(egui::TextWrapMode::Extend);
        ui.add_enabled(enabled, button).clicked()
    });
    if clicked {
        on_click.emit(());
    }
}
