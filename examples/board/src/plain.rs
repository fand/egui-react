//! The same board written with egui alone, for the gallery's side by side.
//!
//! What is shared: `board.rs` — the columns, the cards, the messages, the
//! reducer, the filter, and the arithmetic that turns a drop on a card into a
//! position in a column — and `look.rs`, the colours, the dragged card's ghost
//! and the gap it would drop into. Both versions use every line of both.
//! Nothing that is merely drawing is counted as a difference.
//!
//! What is not shared is the whole of this file, and the difference it is here
//! to show is one field:
//!
//! ```ignore
//! ui: HashMap<CardId, CardUi>,
//! ```
//!
//! A card's editing flag and the title being typed into it have to live
//! *somewhere*, and in immediate mode with no per-item state that somewhere is
//! the caller. So: a map keyed by card id (not by index — an index would hand a
//! card its neighbour's draft the moment anything moved), an entry made on
//! demand, and a line that deletes the entries of cards that are gone. Forget
//! that line and the map grows for as long as the program runs.
//!
//! The react-egui version has no such field and no such line. Each `<Card>`
//! holds its own state, and the pass-end sweep frees it when the card stops
//! being drawn. That is the trade the two files are here to price: the react
//! side pays with a scope and an id per card, the plain side pays with the
//! bookkeeping below.
//!
//! Two smaller entries on the same bill. A card is dragged and dropped on by
//! its *whole rectangle*, and here that rectangle is only known once the card
//! has been drawn — so it is remembered from the frame before, in the same map.
//! taffy has already worked it out for the other version. And the field that is
//! opened focused and selected needs its id known one frame ahead, which is the
//! `fresh` field; over there it is a `use_state` inside the editor itself.
//!
//! Everything else — the drag session, the undo stacks, the debounce — is the
//! same code the hooks contain, moved into `PlainState`. Those are not the
//! difference; they are here to keep the comparison honest.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::board::{Board, CardId, ColumnId, DropTarget, Msg, reduce, visible};
use crate::look::{PLACEHOLDER_H, Theme, gap_amount, ghost, lifted, placeholder};

/// The key the standalone binary stores the board under.
pub const STORAGE_KEY: &str = "board_plain";

/// How long the search box has to be quiet, and how far back undo goes. The
/// same numbers as the react-egui version.
const DEBOUNCE: f64 = 0.3;
const DEPTH: usize = 64;

/// The height of a column's footer, which is also its "drop at the end" zone.
const FOOTER_H: f32 = 26.0;

/// The space between two cards. Drawn by the gap that sits between them rather
/// than by the column's item spacing, for the reason [`drop_gap`] gives.
const CARD_GAP: f32 = 6.0;

/// One card's own state. The react-egui version has this too — as two
/// `use_identity` hooks inside `<Card>`, where nothing else can see them.
#[derive(Clone, Debug, Default)]
struct CardUi {
    editing: bool,
    draft_title: String,
    /// Where the card was drawn last frame. Immediate mode draws a card before
    /// it knows how big it is, and the background that senses the drag has to
    /// be registered *before* the widgets on top of it, so it senses last
    /// frame's rectangle. The other version reads the one taffy already has.
    rect: Option<egui::Rect>,
}

/// Everything the plain version keeps between frames.
#[derive(Serialize, Deserialize)]
pub struct PlainState {
    pub board: Board,

    /// **The difference.** Per-card UI state, keyed by identity so that it
    /// follows a card that is moved, and swept by hand below so that it does
    /// not outlive one that is deleted.
    #[serde(skip)]
    ui: HashMap<CardId, CardUi>,

    /// Undo and redo: `use_undoable` in the other version.
    #[serde(skip)]
    past: Vec<Board>,
    #[serde(skip)]
    future: Vec<Board>,

    /// The search box, and `use_debounced` written out.
    #[serde(skip)]
    search: String,
    #[serde(skip)]
    query: String,
    #[serde(skip)]
    typed_at: f64,
    /// `None` shows every card, `Some(true)` only the ticked ones.
    #[serde(skip)]
    filter: Option<bool>,

    /// The drag session: `use_dnd` written out.
    #[serde(skip)]
    carrying: Option<CardId>,
    #[serde(skip)]
    slots: Vec<(egui::Rect, DropTarget)>,
    #[serde(skip)]
    hovered: Option<DropTarget>,
    #[serde(skip)]
    pointer: Option<egui::Pos2>,

    /// Which column is being renamed, and to what. And which column is having a
    /// card added to it, and what is being typed as its title.
    ///
    /// One at a time each, unlike the react-egui version, where every
    /// `<Column>` has a `use_state` of its own and two could be open at once. A
    /// map here would be another thing to sweep, for a case nobody asked for —
    /// which is exactly the choice a caller is forced to make when the state of
    /// the parts has to live in the whole.
    #[serde(skip)]
    renaming: Option<(ColumnId, String)>,
    #[serde(skip)]
    adding: Option<(ColumnId, String)>,

    /// The one-line editor that should take focus and select its text on the
    /// next frame. `<TitleEdit>` keeps this as a `use_state` of its own and
    /// never has to name the field; here the field has to be named, so its id
    /// is spelled out at both ends.
    #[serde(skip)]
    fresh: Option<egui::Id>,

    #[serde(skip)]
    dark: bool,
}

impl Default for PlainState {
    fn default() -> Self {
        Self {
            board: Board::demo(),
            ui: HashMap::new(),
            past: Vec::new(),
            future: Vec::new(),
            search: String::new(),
            query: String::new(),
            typed_at: f64::NEG_INFINITY,
            filter: None,
            carrying: None,
            slots: Vec::new(),
            hovered: None,
            pointer: None,
            renaming: None,
            adding: None,
            fresh: None,
            dark: true,
        }
    }
}

impl PlainState {
    /// Read the board back. `use_persisted` is this, plus the key.
    pub fn load(json: &str) -> Self {
        serde_json::from_str::<Board>(json).map_or_else(
            |_| Self::default(),
            |board| Self {
                board,
                ..Self::default()
            },
        )
    }

    /// Serialize the board for the caller to write into eframe's storage.
    pub fn save(&self) -> String {
        serde_json::to_string(&self.board).unwrap_or_else(|_| String::from("{}"))
    }

    /// A card's own state, made the first time it is asked for.
    fn card_ui(&mut self, card: CardId) -> &mut CardUi {
        self.ui.entry(card).or_default()
    }

    /// Apply one message, remembering the board it changed.
    fn apply(&mut self, msg: Msg) {
        let before = self.board.clone();
        reduce(&mut self.board, msg);
        if before == self.board {
            return;
        }
        self.past.push(before);
        if self.past.len() > DEPTH {
            self.past.remove(0);
        }
        self.future.clear();
    }

    fn undo(&mut self) {
        if let Some(previous) = self.past.pop() {
            let present = std::mem::replace(&mut self.board, previous);
            self.future.push(present);
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.future.pop() {
            let present = std::mem::replace(&mut self.board, next);
            self.past.push(present);
        }
    }

    /// Read the pointer, resolve a release against the slots the previous frame
    /// offered, and clear them for this one.
    fn begin_frame(&mut self, ctx: &egui::Context) -> Option<Msg> {
        let (pointer, released, cancelled) = ctx.input(|i| {
            (
                i.pointer.interact_pos(),
                i.pointer.any_released(),
                i.key_pressed(egui::Key::Escape),
            )
        });
        self.pointer = pointer;
        self.hovered = pointer.and_then(|pointer| {
            self.slots
                .iter()
                .find(|(rect, _)| rect.contains(pointer))
                .map(|(_, target)| *target)
        });
        self.slots.clear();
        if cancelled {
            self.carrying = None;
        }
        if !released {
            return None;
        }
        let card = self.carrying.take()?;
        let target = self.hovered?;
        Some(Msg::MoveCard {
            card,
            to_column: target.column,
            to_index: self.board.drop_index(card, target),
        })
    }

    /// The search text, once it has been still for [`DEBOUNCE`] seconds.
    fn debounce(&mut self, ctx: &egui::Context, typed: bool) {
        let now = ctx.input(|i| i.time);
        if typed {
            self.typed_at = now;
        }
        if self.query == self.search {
            return;
        }
        let waited = now - self.typed_at;
        if waited >= DEBOUNCE {
            self.query = self.search.clone();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(DEBOUNCE - waited));
        }
    }
}

pub fn ui(ui: &mut egui::Ui, state: &mut PlainState) {
    let theme = Theme { dark: state.dark };
    // A drag that ended over a slot becomes exactly one message, so that undo
    // walks back one drag in one step.
    let mut pending: Vec<Msg> = state.begin_frame(ui.ctx()).into_iter().collect();
    // Undo and redo are not messages, so they are collected separately.
    let (mut undo, mut redo) = (false, false);

    // `p={8}` and `gap={8}` on the react-egui version's root `<View>`.
    egui::Frame::new().inner_margin(8.0).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        toolbar(ui, state, theme, &mut undo, &mut redo);
        ui.separator();
        columns(ui, state, theme, &mut pending);
    });

    for msg in pending {
        state.apply(msg);
    }
    if undo {
        state.undo();
    }
    if redo {
        state.redo();
    }

    // **The line.** Card state is keyed by the card, so it has to be dropped
    // when the card is: nothing else will. The react-egui version's cards are
    // components, and the pass-end sweep frees the hooks of a component that
    // stopped being drawn.
    state.ui.retain(|id, _| state.board.card(*id).is_some());

    if let (Some(card), Some(at)) = (
        state.carrying.and_then(|id| state.board.card(id)),
        state.pointer,
    ) {
        ghost(ui.ctx(), at, &card.title, theme);
    }
}

/// The search box, the done filter, the counts and the history buttons.
fn toolbar(
    ui: &mut egui::Ui,
    state: &mut PlainState,
    theme: Theme,
    undo: &mut bool,
    redo: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        // `gap={6}` on the react-egui version's toolbar row.
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(
            egui::RichText::new("board")
                .size(20.0)
                .strong()
                .color(theme.accent()),
        );
        let typed = ui
            .add_sized(
                egui::vec2(160.0, ui.spacing().interact_size.y),
                egui::TextEdit::singleline(&mut state.search).hint_text("search"),
            )
            .changed();
        state.debounce(ui.ctx(), typed);

        // Two chips, one per answer to the only question a card now has.
        for (name, value) in [("open", false), ("done", true)] {
            let text = egui::RichText::new(name).small().color(theme.accent());
            if ui
                .selectable_label(state.filter == Some(value), text)
                .clicked()
            {
                // Clicking the chip that is already on clears the filter.
                state.filter = (state.filter != Some(value)).then_some(value);
            }
        }

        ui.label(format!("{} cards", state.board.len()));
        // `<Text grow={1.0}>` before the buttons: here the buttons go in a
        // right-to-left `Ui` filling the rest of the row.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if state.dark { "light" } else { "dark" };
            if icon_button(ui, label, None, true) {
                state.dark = !state.dark;
                ui.ctx().set_visuals(if state.dark {
                    egui::Visuals::dark()
                } else {
                    egui::Visuals::light()
                });
            }
            *redo = icon_button(ui, "redo", None, !state.future.is_empty());
            *undo = icon_button(ui, "undo", None, !state.past.is_empty());
        });
    });
}

/// The four columns, side by side and equally wide.
fn columns(ui: &mut egui::Ui, state: &mut PlainState, theme: Theme, pending: &mut Vec<Msg>) {
    let ids: Vec<ColumnId> = state.board.columns.iter().map(|column| column.id).collect();
    let query = state.query.clone();
    let filter = state.filter;

    ui.columns(ids.len(), |uis| {
        for (ui, id) in uis.iter_mut().zip(ids) {
            column(ui, state, theme, id, &query, filter, pending);
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn column(
    ui: &mut egui::Ui,
    state: &mut PlainState,
    theme: Theme,
    id: ColumnId,
    query: &str,
    filter: Option<bool>,
    pending: &mut Vec<Msg>,
) {
    let Some(index) = state.board.columns.iter().position(|c| c.id == id) else {
        return;
    };
    let name = state.board.columns[index].name.clone();
    let total = state.board.columns[index].cards.len();
    let shown = visible(&state.board.columns[index], query, filter);

    // Where the card in hand would land, unless that is where it already is: a
    // drop that moves nothing gets no gap opened for it.
    let carried = state.carrying;
    let carried_at = carried.and_then(|card| shown.iter().position(|shown| *shown == card));
    let gap = state
        .hovered
        .filter(|target| target.column == id)
        .filter(|target| {
            target.before != carried
                && carried_at.is_none_or(|i| target.before != shown.get(i + 1).copied())
        })
        .map(|target| target.before);

    ui.spacing_mut().item_spacing.y = 6.0;
    ui.horizontal(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.spacing_mut().item_spacing.x = 4.0;
        let renaming_here = matches!(&state.renaming, Some((column, _)) if *column == id);
        if renaming_here {
            let (_, mut draft) = state.renaming.take().expect("just looked");
            let field = egui::Id::new(("board_plain/rename", id));
            match title_edit(ui, &mut state.fresh, field, &mut draft, "column name") {
                Edited::Typing => state.renaming = Some((id, draft)),
                Edited::Commit(name) => pending.push(Msg::RenameColumn { column: id, name }),
                Edited::Cancel => {}
            }
        } else {
            ui.label(egui::RichText::new(&name).strong().color(theme.accent()));
        }
        // The same reading order as the react-egui column header, where the
        // name has `grow={1.0}` and pushes the rest to the right.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("{}/{}", shown.len(), total));
            // While the name is an editor there is nothing left to rename.
            if !renaming_here && icon_button(ui, "rename", None, true) {
                state.renaming = Some((id, name.clone()));
                state.fresh = Some(egui::Id::new(("board_plain/rename", id)));
            }
        });
    });

    // The cards and, after them, the footer that adds one and catches a drop
    // meant for the end of the column — the same order as the react-egui
    // version, where the footer is inside the `<ScrollArea>` too.
    egui::ScrollArea::vertical().id_salt(id).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        // No item spacing: the space between two cards is the closed gap that
        // sits between them. See [`drop_gap`].
        ui.spacing_mut().item_spacing.y = 0.0;

        // The gaps are animated in the picture, not in the layout, for the
        // same reason as the other version (a relayout per frame would cost a
        // second pass per frame): each card below a gap is drawn shifted by
        // however far its gap still has to go. `lift` adds those up.
        let carrying = state.carrying.is_some();
        let mut lift = 0.0;
        for (i, card) in shown.iter().enumerate() {
            let target = DropTarget {
                column: id,
                before: Some(*card),
            };
            let open = gap == Some(Some(*card));
            let amount = gap_amount(
                ui.ctx(),
                egui::Id::new(("board_plain/gap", target)),
                open,
                carrying,
            );
            drop_gap(ui, state, theme, target, open, amount, lift);
            lift += (amount - f32::from(u8::from(open))) * PLACEHOLDER_H;
            self::card(
                ui,
                state,
                theme,
                id,
                *card,
                shown.get(i + 1).copied(),
                lift,
                pending,
            );
        }
        if shown.is_empty() {
            ui.add_space(CARD_GAP);
            ui.label("nothing here");
        }

        // The new card, in the same frame the saved ones wear, so that what is
        // being typed looks like what it will become. It is not on the board
        // until it is confirmed: an empty card put there and then taken off
        // again would be two steps of undo for one card, and the storage would
        // hold a nameless card if the window closed in between.
        if matches!(&state.adding, Some((column, _)) if *column == id) {
            let (_, mut draft) = state.adding.take().expect("just looked");
            ui.add_space(CARD_GAP);
            egui::Frame::new()
                .fill(theme.card())
                .corner_radius(4.0)
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    let field = egui::Id::new(("board_plain/new", id));
                    match title_edit(ui, &mut state.fresh, field, &mut draft, "new card") {
                        Edited::Typing => state.adding = Some((id, draft)),
                        Edited::Commit(title) => {
                            if !title.trim().is_empty() {
                                pending.push(Msg::AddCard { column: id, title });
                            }
                        }
                        Edited::Cancel => {}
                    }
                });
        }

        let end = DropTarget {
            column: id,
            before: None,
        };
        let open = gap == Some(None);
        let amount = gap_amount(
            ui.ctx(),
            egui::Id::new(("board_plain/gap", end)),
            open,
            carrying,
        );
        drop_gap(ui, state, theme, end, open, amount, lift);
        lift += (amount - f32::from(u8::from(open))) * PLACEHOLDER_H;

        let rect =
            egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), FOOTER_H));
        if state.carrying.is_some() {
            state.slots.push((rect, end));
        }
        let button = egui::Button::new("+ card").wrap_mode(egui::TextWrapMode::Extend);
        let clicked = ui
            .with_visual_transform(lifted(lift), |ui| ui.put(rect, button).clicked())
            .inner;
        if clicked {
            state.adding = Some((id, String::new()));
            state.fresh = Some(egui::Id::new(("board_plain/new", id)));
        }
    });
}

/// The space between two cards, and the gap a card in hand would drop into.
///
/// One of these goes in front of every card and in front of the footer, open or
/// closed, and closed it is the column's card spacing. The react-egui version
/// has the same rule for a reason that does not apply here — a taffy node that
/// comes and goes has no rectangle on the frame it appears — but the two are
/// laid out to the same numbers, so this one keeps the rule too and the same
/// test measures both.
fn drop_gap(
    ui: &mut egui::Ui,
    state: &mut PlainState,
    theme: Theme,
    target: DropTarget,
    open: bool,
    // How open the picture of the gap is, and how far the picture of the card
    // above it has been shifted; the same numbers as the other version.
    amount: f32,
    lift: f32,
) {
    let extra = if open { PLACEHOLDER_H } else { 0.0 };
    let size = egui::vec2(ui.available_width(), CARD_GAP + extra);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let shown = amount * PLACEHOLDER_H;
    if shown > 0.5 {
        // The spacing stays spacing: the card is drawn in what is new — below
        // the card above, wherever its picture is at the moment.
        let top = egui::pos2(rect.left(), rect.top() + lift + CARD_GAP);
        let seen = egui::Rect::from_min_size(top, egui::vec2(rect.width(), shown));
        placeholder(ui.painter(), seen, theme);
    }
    if !open {
        return;
    }
    // Painted, not a widget, so the name has to be said out loud; the test asks
    // for it to know whether a gap is open.
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Other, ui.is_enabled(), "drop here")
    });
    state.slots.push((rect, target));
}

#[allow(clippy::too_many_arguments)]
fn card(
    ui: &mut egui::Ui,
    state: &mut PlainState,
    theme: Theme,
    column: ColumnId,
    id: CardId,
    next: Option<CardId>,
    // How far the picture of this card is from where it was laid out, while a
    // gap above it is still opening or closing.
    lift: f32,
    pending: &mut Vec<Msg>,
) {
    let Some(card) = state.board.card(id).cloned() else {
        return;
    };
    let carried = state.carrying == Some(id);
    let dragging = state.carrying.is_some();
    // Every read of a card's own state goes through the map, and every write
    // has to put it back. This is the shape all of `plain.rs` takes.
    let ui_state = state.card_ui(id).clone();
    let field = egui::Id::new(("board_plain/title", id));

    // The card is grabbed and dropped on by the whole of itself, so the drag
    // lives on a rectangle behind the widgets rather than on the title. It is
    // registered first, and that order is the feature: egui picks the topmost
    // click candidate and the topmost drag candidate separately, so the
    // checkbox and the buttons still take their clicks while a press that turns
    // into a movement falls through to here. The rectangle is the one the last
    // frame left behind, because this one has not been drawn yet.
    if let Some(rect) = ui_state.rect {
        let bg = ui.interact(
            rect,
            egui::Id::new(("board_plain/card", id)),
            egui::Sense::drag(),
        );
        // A drag handle is a control, so it says what it is a handle for. Not
        // the bare title: that is already the label beside it, and two nodes
        // with one name is the thing a screen reader cannot tell apart.
        ui.ctx().accesskit_node_builder(bg.id, |node| {
            node.set_label(format!("card: {}", card.title));
        });
        if bg.drag_started() {
            state.carrying = Some(id);
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
    }

    // The shift is visual only: the drag surface and the slots are where the
    // card will be, which is where the pointer is aiming.
    let drawn = ui
        .with_visual_transform(lifted(lift), |ui| {
            egui::Frame::new()
                .fill(theme.card())
                .corner_radius(4.0)
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.spacing_mut().item_spacing.x = 6.0;

                        // `done` is read out of the board, so the box is drawn against
                        // a copy and what comes back is a message, not a write.
                        let mut done = card.done;
                        let box_ = ui.add(egui::Checkbox::without_text(&mut done));
                        // A card is found by this name — by a screen reader, and by the
                        // test, which needs a hold on a card whose title has become an
                        // editor.
                        ui.ctx().accesskit_node_builder(box_.id, |node| {
                            node.set_label(format!("done: {}", card.title));
                        });
                        if box_.changed() {
                            pending.push(Msg::SetDone {
                                card: id,
                                done: !card.done,
                            });
                        }

                        // The buttons are pinned to the right and the title fills what
                        // is left, which is `grow={1.0}` on the other side.
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_button(ui, "×", Some("remove"), true) {
                                pending.push(Msg::RemoveCard { card: id });
                            }
                            if icon_button(ui, "✎", Some("edit"), true) {
                                // Opening takes a fresh copy of the saved title, so
                                // closing the editor and opening it again starts from
                                // what was saved rather than from an old draft.
                                let entry = state.card_ui(id);
                                entry.draft_title = card.title.clone();
                                entry.editing = true;
                                state.fresh = Some(field);
                            }
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    if ui_state.editing {
                                        title(ui, state, id, field, pending);
                                    } else {
                                        let mut text = egui::RichText::new(&card.title);
                                        if card.done {
                                            text = text.weak().strikethrough();
                                        }
                                        if carried {
                                            text = text.weak();
                                        }
                                        // Not selectable, and it senses nothing. A label
                                        // that senses a drag still starts a text selection
                                        // on the press, and egui then runs that selection
                                        // across every label the pointer passes over on its
                                        // way. The card is dragged by the background above.
                                        ui.add(egui::Label::new(text).truncate().selectable(false));
                                    }
                                },
                            );
                        });
                    });
                })
                .response
        })
        .inner;

    state.card_ui(id).rect = Some(drawn.rect);
    // The whole card split in two: the top half means "in front of me", the
    // bottom half "in front of the next one", which is how a list of cards is
    // also a list of the gaps between them.
    if dragging {
        let (top, bottom) = drawn.rect.split_top_bottom_at_fraction(0.5);
        state.slots.push((
            top,
            DropTarget {
                column,
                before: Some(id),
            },
        ));
        state.slots.push((
            bottom,
            DropTarget {
                column,
                before: next,
            },
        ));
    }
}

/// The card's title while it is being edited, taken out of the map and put
/// back — the borrow checker's way of saying that this state is the whole
/// board's, not the card's.
fn title(
    ui: &mut egui::Ui,
    state: &mut PlainState,
    id: CardId,
    field: egui::Id,
    pending: &mut Vec<Msg>,
) {
    let mut draft = std::mem::take(&mut state.card_ui(id).draft_title);
    let edited = title_edit(ui, &mut state.fresh, field, &mut draft, "title");
    match edited {
        Edited::Typing => state.card_ui(id).draft_title = draft,
        Edited::Commit(title) => {
            // Confirming an empty title is a cancel: a nameless card would
            // leave nothing to click on to name it again.
            if !title.trim().is_empty() {
                pending.push(Msg::SetTitle { card: id, title });
            }
            state.card_ui(id).editing = false;
        }
        Edited::Cancel => state.card_ui(id).editing = false,
    }
}

/// What a one-line editor did on this frame.
enum Edited {
    Typing,
    Commit(String),
    Cancel,
}

/// The plain twin of `<TitleEdit>`: the card's title, the column's name and the
/// card being added are all the same three keys.
///
/// Enter confirms, Escape puts it back, and clicking elsewhere confirms — the
/// last because a board is clicked around rather than tabbed through, and
/// losing what was typed for looking away is not a thing anyone means.
fn title_edit(
    ui: &mut egui::Ui,
    fresh: &mut Option<egui::Id>,
    field: egui::Id,
    text: &mut String,
    name: &str,
) -> Edited {
    let width = ui.available_width();
    let response = ui.add(
        egui::TextEdit::singleline(text)
            .id(field)
            .desired_width(width),
    );
    // A nameless text field is what the gallery's a11y test counts, and the
    // text says nothing about which of the three this one is.
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(name));

    // Focus and select-all on the frame the field first appears; after that the
    // caret is the user's business. Whoever opened the editor said which field
    // it was, because in immediate mode there is no "first frame" to ask.
    if *fresh == Some(field) {
        response.request_focus();
        let mut state = egui::TextEdit::load_state(ui.ctx(), field).unwrap_or_default();
        let end = egui::text::CCursor::new(text.chars().count());
        let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), end);
        state.cursor.set_char_range(Some(all));
        state.store(ui.ctx(), field);
        *fresh = None;
    }

    if response.lost_focus() {
        // egui hands focus back on Escape, so the two arrive together and the
        // only question is which of them ended the edit.
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            return Edited::Cancel;
        }
        return Edited::Commit(text.clone());
    }
    Edited::Typing
}

/// The plain twin of `<IconButton>`. `name` is what the accessibility tree says
/// when the button shows a picture instead of a word.
fn icon_button(ui: &mut egui::Ui, label: &str, name: Option<&str>, enabled: bool) -> bool {
    let button = egui::Button::new(label)
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
}
