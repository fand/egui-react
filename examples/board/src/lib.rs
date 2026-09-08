//! A kanban board: cards that keep their own state while they move.
//!
//! Every other example holds its state in one place and draws it. This one is
//! about the state that belongs to the *items* — the half-typed title someone
//! is in the middle of — and about what happens to it when the items are
//! reordered, moved to another column, filtered away and brought back.
//!
//! Four things are on show, in the order they matter:
//!
//! - **A card owns the title being typed into it.** `editing` and the draft
//!   are `use_identity` hooks inside `<Card>`, not entries in a map somewhere
//!   above it. Drag a card someone is editing into another column and the
//!   draft goes with it, because the state is keyed by the card's id rather
//!   than by where the card is drawn (`hooks::use_identity` explains why that
//!   is not the same as `key=`). A card that is deleted — or filtered out of
//!   view, which unmounts it just the same — has its state dropped by the
//!   pass-end sweep, which is the housekeeping `plain.rs` writes out by hand.
//! - **Components of our own, composed.** `<Toolbar>`, `<Column>`, `<Card>`,
//!   `<TitleEdit>`, `<Placeholder>`, `<Chip>` and `<IconButton>` each take a
//!   `style: ItemStyle`, so the caller decides where they sit, and report what
//!   the user did with `#[event]`.
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
//! a component *did* goes up as an event: `<Card>` tells its column that a
//! title was confirmed or a box was ticked, `<Chip>` tells whoever drew it that
//! it was clicked. A card has never heard of a board message and could be used
//! somewhere else. What the whole tree shares comes down through the context:
//! the theme, the drag in progress, and the `Dispatch` a column turns those
//! events into messages with — which is what saves the four `<Column>`s in the
//! loop from carrying four callbacks each.
//!
//! `plain.rs` is the same board in plain egui, and the two are driven by the
//! same test.
//!
//! No `Panel`, so the gallery can run it in a column of its own.

use egui_react::prelude::*;
use egui_react_elements::prelude::*;
use example_meta::Meta;

pub mod board;
pub mod hooks;
pub mod look;
pub mod plain;

use board::{
    Board, Card as CardData, CardId, Column as ColumnData, ColumnId, DropTarget, Msg, reduce,
    visible,
};
use hooks::{Dnd, Undoable, use_debounced, use_dnd, use_identity, use_undoable};
use look::{PLACEHOLDER_H, Theme, gap_amount, ghost, lifted, placeholder};

pub const META: Meta = Meta {
    name: "board",
    summary: "Cards that keep the title being typed into them while they are dragged between \
              columns.",
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
    elements: &["View", "Text", "TextEdit", "ScrollArea", "Separator"],
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

/// The space between two cards in a column. It is drawn by [`Placeholder`]
/// rather than by the column's `gap`, because it is also the gap that opens
/// when a card is about to land there.
const CARD_GAP: f32 = 6.0;

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
    let mut filter = use_state(cx, || None::<bool>);
    // Read once: an element may not hold a shared borrow of a state *and* a
    // handler that writes it (ARCHITECTURE 3.7), and the toolbar does both.
    let done = *filter;
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
                done={&done}
                count={board.len()}
                can_undo={history.can_undo()}
                can_redo={history.can_redo()}
                on_filter={|picked: bool| {
                    // Clicking the chip that is already on clears the filter.
                    *filter = (*filter != Some(picked)).then_some(picked);
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
                        done={&done}
                        on_add={|title: String| {
                            dispatch.send(Undoable::Do(Msg::AddCard { column: column.id, title }));
                        }}
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

/// The search box, the done filter, the counts and the history buttons.
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
    // `&Option<bool>`, not `Option<bool>`: an `Option` prop is the *optional*
    // kind, whose setter takes the inner value and defaults to `None`.
    done: &Option<bool>,
    count: usize,
    can_undo: bool,
    can_redo: bool,
    #[event] on_filter: bool,
    #[event] on_undo: (),
    #[event] on_redo: (),
    #[event] on_theme: (),
) {
    let theme = use_theme(cx);

    rsx! {
        <View style={style} direction="row" w="100%" gap={6} align="center">
            <Text size={20.0} strong color={theme.accent()}>"board"</Text>
            <TextEdit w={160.0} bind={search} hint="search"/>
            // Two chips, one per answer to the only question a card now has.
            for (name, value) in [("open", false), ("done", true)] {
                <Chip
                    key={name}
                    label={name}
                    color={theme.accent()}
                    active={*done == Some(value)}
                    on_click={|| on_filter.emit(value)}
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
    done: &Option<bool>,
    #[event] on_add: String,
) {
    let theme = use_theme(cx);
    let dnd = use_drag(cx);
    let actions = use_context::<Actions>(cx).map(|actions| actions.get());

    let mut renaming = use_state(cx, || false);
    let mut draft = use_state(cx, || column.name.clone());
    // A new card is written here and only reaches the board when it is
    // confirmed. Putting an empty one on the board to be filled in instead
    // would cost two steps of undo for one card, and `use_persisted` would
    // save the nameless card if the window closed in between.
    let mut adding = use_state(cx, || false);
    let mut new_title = use_state(cx, String::new);

    let shown: &Vec<CardId> = use_memo(cx, (column.id, rev, search, done), || {
        visible(column, search, *done)
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

    let carried = dnd.carrying();
    let carried_at = carried.and_then(|id| cards.iter().position(|card| card.id == id));
    // Where the card in hand would land — unless that is where it already is.
    // A drop that moves nothing gets no gap opened for it, because the eye
    // would read the gap as "it would go *there*" and it would not.
    let preview = dnd
        .hovered()
        .filter(|target| target.column == column.id)
        .filter(|target| {
            target.before != carried && carried_at.is_none_or(|i| target.before != after[i])
        });
    let gap = preview.map(|target| target.before);

    // The gaps are animated, but not in the layout: a taffy node whose height
    // changed makes the layout engine lay the column out again and ask egui for a
    // second pass, every frame, for as long as the animation runs. So the
    // layout jumps to where it will end up, and what slides is the picture —
    // each card below a gap is drawn shifted by however far its gap still has
    // to go. `lift` adds those up on the way down the column.
    let carrying = carried.is_some();
    let ctx = cx.ctx().clone();
    let mut lift = 0.0;
    // Each gap, and the lift of whatever comes right after it.
    let mut animate = |target: DropTarget, open: bool| -> (Gap, f32) {
        let amount = gap_amount(&ctx, egui::Id::new(("board/gap", target)), open, carrying);
        let gap = Gap { open, amount, lift };
        lift += (amount - f32::from(u8::from(open))) * PLACEHOLDER_H;
        (gap, lift)
    };
    let (gaps, lifts): (Vec<Gap>, Vec<f32>) = cards
        .iter()
        .map(|card| {
            let target = DropTarget {
                column: column.id,
                before: Some(card.id),
            };
            animate(target, gap == Some(Some(card.id)))
        })
        .unzip();
    let end = DropTarget {
        column: column.id,
        before: None,
    };
    let (end_gap, footer_lift) = animate(end, gap == Some(None));

    let renaming_now = *renaming;
    let adding_now = *adding;

    rsx! {
        <View style={style} direction="column" gap={6}>
            <View direction="row" w="100%" gap={4} align="center">
                if renaming_now {
                    <TitleEdit
                        grow={1.0}
                        bind={draft.bind()}
                        name="column name"
                        on_commit={|name: String| {
                            send(&actions, Msg::RenameColumn { column: column.id, name });
                            *renaming = false;
                        }}
                        on_cancel={|| *renaming = false}
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
                // No `gap`: the space between two cards is the closed
                // placeholder that lives between them, which is what lets a
                // gap open without any node appearing or disappearing.
                <View direction="column" w="100%" pr={4}>
                    for (i, card) in cards.iter().enumerate() {
                        <Placeholder
                            key={(card.id, "gap")}
                            gap={gaps[i]}
                            target={DropTarget { column: column.id, before: Some(card.id) }}
                        />
                        // The key is the card's id. It is what tells two cards
                        // apart inside this column — and, together with
                        // `use_identity` in the card itself, what makes a
                        // card's own state follow it out of this column.
                        <Card
                            key={card.id}
                            card={card}
                            column={column.id}
                            next={&after[i]}
                            lift={lifts[i]}
                            on_title={|title: String| {
                                send(&actions, Msg::SetTitle { card: card.id, title });
                            }}
                            on_done={|done: bool| {
                                send(&actions, Msg::SetDone { card: card.id, done });
                            }}
                            on_remove={|| send(&actions, Msg::RemoveCard { card: card.id })}
                        />
                    }
                    if cards.is_empty() {
                        <Text mt={CARD_GAP}>"nothing here"</Text>
                    }

                    // The new card, in the same box the saved ones wear, so
                    // that what is being typed looks like what it will become.
                    if adding_now {
                        <View
                            w="100%"
                            mt={CARD_GAP}
                            p={6}
                            bg={theme.card()}
                            radius={4.0}
                        >
                            <TitleEdit
                                w="100%"
                                bind={new_title.bind()}
                                name="new card"
                                on_commit={|title: String| {
                                    if !title.trim().is_empty() {
                                        on_add.emit(title);
                                    }
                                    *adding = false;
                                }}
                                on_cancel={|| *adding = false}
                            />
                        </View>
                    }

                    // The new card's box above sits before this gap, so it
                    // is only ever lifted by the gaps between the cards, and
                    // that is a drag started while typing: not worth a leaf.
                    <Placeholder
                        gap={end_gap}
                        target={DropTarget { column: column.id, before: None }}
                    />

                    // The footer is both the "add a card" button and the place
                    // a drag ends when it means "at the end of this column".
                    // One leaf, so the rectangle offered to the drag session is
                    // the one the eye sees.
                    {view(|cx| {
                        let (rect, clicked) = cx.leaf_fill(
                            &ItemStyle::default().w("100%").h(FOOTER_H),
                            |ui| {
                                let rect = ui.max_rect();
                                let button = egui::Button::new("+ card")
                                    .wrap_mode(egui::TextWrapMode::Extend);
                                let clicked = ui
                                    .with_visual_transform(lifted(footer_lift), |ui| {
                                        ui.add_sized(rect.size(), button).clicked()
                                    })
                                    .inner;
                                (rect, clicked)
                            },
                        );
                        dnd.slot(rect, DropTarget { column: column.id, before: None });
                        if clicked {
                            *new_title = String::new();
                            *adding = true;
                        }
                    })}
                </View>
            </ScrollArea>
        </View>
    }
}

/// One card: a tick box, a title that can be edited in place, and — the point
/// of the example — state of its own.
///
/// `editing` and `draft` belong to *this card*. Nothing above it knows they
/// exist, nothing has to make room for them when a card is added, and nothing
/// has to clean up after them when one is deleted.
///
/// The whole card is one `cx.leaf`, which is what a `<View bg p radius>` would
/// have been anyway, plus the one thing an element cannot hand back: the
/// rectangle. Three things want it — the card is the drag handle, the card is
/// the drop zone, and the card is what the cursor changes over — and none of
/// them can be told where the card is by a `<View>`, which returns no
/// `Response` (plan.md section 8.4).
///
/// The `egui::Frame` inside the leaf stays for a second reason: the lift
/// transform has to move the background with the content, and only what is
/// drawn in the leaf's own `Ui` is under that transform.
#[component]
fn Card(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    card: &CardData,
    column: ColumnId,
    // The card below this one; `None` at the end of the column.
    next: &Option<CardId>,
    // How far the picture of this card is from where the layout put it, while
    // a gap above it is still opening or closing. See `<Column>`.
    #[prop(default)] lift: f32,
    #[event] on_title: String,
    #[event] on_done: bool,
    #[event] on_remove: (),
) {
    let theme = use_theme(cx);
    let dnd = use_drag(cx);

    // Keyed by the card, not by where the card is: `key=` distinguishes
    // siblings under one parent, and a card that moves changes parents. See
    // `hooks::use_identity`.
    let mut editing = use_identity(cx, (card.id, "editing"), || false);
    let mut draft = use_identity(cx, (card.id, "draft"), || card.title.clone());

    let carried = dnd.carrying() == Some(card.id);
    let dragging = dnd.carrying().is_some();
    let editing_now = *editing;
    let next = *next;
    let (store, scope) = (cx.store, cx.scope_id());

    cx.leaf(&style.w("100%"), move |ui| {
        // Taken before anything is drawn, because it is what taffy gave the
        // whole card rather than what the row of widgets ended up covering.
        // On the very first frame the height is not right yet; from the second
        // it is, which is the same deal the footer's `leaf_fill` takes.
        let rect = ui.max_rect();
        // Registered *before* the children, and that order is the feature. egui
        // picks the topmost click candidate and the topmost drag candidate
        // separately, so the checkbox and the buttons — which only click —
        // still take their clicks, while a press that turns into a movement
        // falls through to here. Pressing on the checkbox and moving therefore
        // drags the card, which is what a hand expects and what the test does.
        let bg = ui.interact(
            rect,
            egui::Id::new(("board/card", card.id)),
            egui::Sense::drag(),
        );
        // A drag handle is a control, so it says what it is a handle for. Not
        // the bare title: that is already the label beside it, and two nodes
        // with one name is the thing a screen reader cannot tell apart.
        ui.ctx().accesskit_node_builder(bg.id, |node| {
            node.set_label(format!("card: {}", card.title));
        });
        if bg.drag_started() {
            dnd.pick_up(card.id);
        }
        if bg.contains_pointer() {
            // `contains_pointer`, not `hovered`: the pointer is over the card
            // even when it is over a widget drawn on top of it.
            ui.ctx().set_cursor_icon(if dragging {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::PointingHand
            });
        }
        // The whole card split in two: the top half means "in front of me",
        // the bottom half "in front of the next one", which is how a list of
        // cards is also a list of the gaps between them.
        let (top, bottom) = rect.split_top_bottom_at_fraction(0.5);
        dnd.slot(
            top,
            DropTarget {
                column,
                before: Some(card.id),
            },
        );
        dnd.slot(
            bottom,
            DropTarget {
                column,
                before: next,
            },
        );

        // The shift is visual only: the drag surface above and the slots are
        // where the card will be, which is where the pointer is aiming.
        ui.with_visual_transform(lifted(lift), move |ui| {
            egui::Frame::default()
            .fill(theme.card())
            .inner_margin(6i8)
            .corner_radius(4u8)
            .show(ui, move |ui| {
                let mut cx = Cx::new(store, ui, scope);
                let row = rsx! {
                    <View direction="row" w="100%" gap={6} align="center">
                        {view(|cx| {
                            let toggled = cx.leaf(&ItemStyle::default().shrink(0.0), |ui| {
                                // `done` is a prop, so the box is drawn against
                                // a copy: what comes back out is an event, not
                                // a write. `<Checkbox bind>` wants the `&mut`
                                // this card does not have.
                                let mut done = card.done;
                                let response = ui.add(egui::Checkbox::without_text(&mut done));
                                // A card is found by this name — by a screen
                                // reader, and by the test, which needs a hold
                                // on a card whose title has become an editor.
                                ui.ctx().accesskit_node_builder(response.id, |node| {
                                    node.set_label(format!("done: {}", card.title));
                                });
                                response.changed()
                            });
                            if toggled {
                                on_done.emit(!card.done);
                            }
                        })}

                        if editing_now {
                            <TitleEdit
                                grow={1.0}
                                min_w={0.0}
                                bind={draft.bind()}
                                name="title"
                                on_commit={|title: String| {
                                    // Confirming an empty title is a cancel:
                                    // a nameless card would leave nothing to
                                    // click on to name it again.
                                    if !title.trim().is_empty() {
                                        on_title.emit(title);
                                    }
                                    *editing = false;
                                }}
                                on_cancel={|| *editing = false}
                            />
                        } else {
                            {view(|cx| {
                                cx.leaf(&ItemStyle::default().grow(1.0).min_w(0.0), |ui| {
                                    ui.style_mut().wrap_mode =
                                        Some(egui::TextWrapMode::Truncate);
                                    let mut text = egui::RichText::new(card.title.as_str());
                                    if card.done {
                                        text = text.weak().strikethrough();
                                    }
                                    if carried {
                                        text = text.weak();
                                    }
                                    // Not selectable, and it senses nothing.
                                    // A label that senses a drag still starts a
                                    // text selection on the press, and egui
                                    // then runs that selection across every
                                    // label the pointer passes over on its way
                                    // (`label_text_selection.rs`). The card is
                                    // dragged by the background above.
                                    ui.add(egui::Label::new(text).selectable(false));
                                });
                            })}
                        }

                        <IconButton name="edit" on_click={|| {
                            // Opening takes a fresh copy of the saved title, so
                            // closing the editor and opening it again starts
                            // from what was saved rather than from an old draft.
                            *draft = card.title.clone();
                            *editing = true;
                        }}>"✎"</IconButton>
                        <IconButton name="remove" on_click={|| on_remove.emit(())}>"×"</IconButton>
                    </View>
                };
                row.show(&mut cx);
            });
        });
    });
}

/// A gap's visual state for one frame, worked out by the column: whether it
/// is open in the layout, how open the picture of it is, and how far the
/// picture of everything above it has already been shifted.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Gap {
    open: bool,
    amount: f32,
    lift: f32,
}

/// A one-line editor that opens with its text selected: the card's title, the
/// column's name, and the card being added are all the same three keys.
///
/// Enter confirms, Escape puts it back, and clicking elsewhere confirms — the
/// last because a board is clicked around rather than tabbed through, and
/// losing what was typed for looking away is not a thing anyone means.
#[component]
fn TitleEdit(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    bind: &mut String,
    // The name in the accessibility tree: the text says nothing about which
    // of the three this is, and a nameless text field is what the gallery's
    // a11y test counts.
    name: &str,
    #[event] on_commit: String,
    #[event] on_cancel: (),
) {
    // Focus and select-all happen once, on the frame the field appears: after
    // that the caret is the user's business. The field is unmounted when the
    // editor closes, so the next opening is fresh again — and so is a card that
    // is dragged into another column, which remounts it. The draft survives
    // that (it is a `use_identity` hook); the caret goes back to selecting
    // everything, which is the same place it started.
    let mut fresh = use_state(cx, || true);
    let first = *fresh;

    let response = cx.leaf(&style, |ui| {
        let width = ui.available_width();
        let response = ui.add(egui::TextEdit::singleline(bind).desired_width(width));
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_label(name));
        if first {
            response.request_focus();
            let mut state = egui::TextEdit::load_state(ui.ctx(), response.id).unwrap_or_default();
            let end = egui::text::CCursor::new(bind.chars().count());
            let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), end);
            state.cursor.set_char_range(Some(all));
            state.store(ui.ctx(), response.id);
        }
        response
    });
    if first {
        *fresh = false;
    }

    if response.lost_focus() {
        // egui hands focus back on Escape, so the two arrive together and the
        // only question is which of them ended the edit.
        if cx.ui().input(|i| i.key_pressed(egui::Key::Escape)) {
            on_cancel.emit(());
        } else {
            on_commit.emit(bind.clone());
        }
    }
}

/// The space between two cards, and the gap a card in hand would drop into.
///
/// One of these sits in front of every card and in front of the footer, open
/// or closed. **Closed it is not nothing**: it is the column's card spacing,
/// and that is what keeps it in the tree. A gap that came and went would be a
/// taffy node that came and went, and a node taffy has not laid out yet has an
/// empty rectangle for one frame — the very frame the pointer needs it, since
/// opening the gap is what pushed the card out from under the pointer. The
/// slot would find nothing, the gap would shut, the card would come back, and
/// the column would shake once a frame. A node that only changes height has a
/// rectangle at every moment.
///
/// The open one registers itself as a drop slot for the target it is showing,
/// which is the other half of the same argument: the pointer ends up over the
/// gap, so the gap has to be an answer to "what is under the pointer".
#[component]
fn Placeholder(cx: &mut Cx, #[prop(default)] style: ItemStyle, gap: Gap, target: DropTarget) {
    let theme = use_theme(cx);
    let dnd = use_drag(cx);
    let Gap { open, amount, lift } = gap;
    // The layout opens all at once; the picture catches up. See `<Column>`.
    let extra = if open { PLACEHOLDER_H } else { 0.0 };
    let shown = amount * PLACEHOLDER_H;

    // `leaf_fill`, not `leaf`: a content-measured leaf is measured in a
    // zero-width `Ui` on its first frame and taffy keeps it that way
    // (ARCHITECTURE 6). Here the size is the style's — the whole width, and
    // one card's height once there is a card to make room for.
    let rect = cx.leaf_fill(&style.w("100%").h(CARD_GAP + extra), |ui| {
        let rect = ui.max_rect();
        let response = ui.allocate_rect(rect, egui::Sense::hover());
        if shown > 0.5 {
            // The spacing stays spacing: the card is drawn in what is new —
            // below the card above, wherever its picture is at the moment.
            let top = egui::pos2(rect.left(), rect.top() + lift + CARD_GAP);
            let seen = egui::Rect::from_min_size(top, egui::vec2(rect.width(), shown));
            placeholder(ui.painter(), seen, theme);
        }
        if open {
            // Painted, not a widget, so the name has to be said out loud; the
            // test asks for it to know whether a gap is open.
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Other, ui.is_enabled(), "drop here")
            });
        }
        rect
    });
    if open {
        dnd.slot(rect, target);
    }
}

/// A small toggle: the two filters in the toolbar.
///
/// `egui-react-elements` has no chip and no toggle, so this is the escape
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
    // What the button is called, when what it shows is a picture. The tree
    // says words even where the screen says a glyph.
    name: Option<&str>,
    #[event] on_click: (),
    children: impl Into<egui::WidgetText>,
) {
    let clicked = cx.leaf(&style.shrink(0.0), |ui| {
        let button = egui::Button::new(children)
            .small()
            .frame(false)
            .wrap_mode(egui::TextWrapMode::Extend);
        let response = ui.add_enabled(enabled, button);
        if let Some(name) = name {
            // The widget has already written its node for this pass, so this
            // overwrites the label egui took from the glyph.
            ui.ctx()
                .accesskit_node_builder(response.id, |node| node.set_label(name));
        }
        response.clicked()
    });
    if clicked {
        on_click.emit(());
    }
}
