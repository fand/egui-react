//! The same board written with egui alone, for the gallery's side by side.
//!
//! What is shared: `board.rs` — the columns, the cards, the messages, the
//! reducer, the filter, and the arithmetic that turns a drop on a card into a
//! position in a column — and `look.rs`, the colours and the dragged card's
//! ghost. Both versions use every line of both. Nothing that is merely
//! drawing is counted as a difference.
//!
//! What is not shared is the whole of this file, and the difference it is here
//! to show is one field:
//!
//! ```ignore
//! ui: HashMap<CardId, CardUi>,
//! ```
//!
//! A card's editing draft and its expanded flag have to live *somewhere*, and
//! in immediate mode with no per-item state that somewhere is the caller. So:
//! a map keyed by card id (not by index — an index would hand a card its
//! neighbour's draft the moment anything moved), an entry made on demand, and
//! a line that deletes the entries of cards that are gone. Forget that line
//! and the map grows for as long as the program runs.
//!
//! The react-egui version has no such field and no such line. Each `<Card>`
//! holds its own state, and the pass-end sweep frees it when the card stops
//! being drawn. That is the trade the two files are here to price: the react
//! side pays with a scope and an id per card, the plain side pays with the
//! bookkeeping below.
//!
//! Everything else — the drag session, the undo stacks, the debounce — is the
//! same code the hooks contain, moved into `PlainState`. Those are not the
//! difference; they are here to keep the comparison honest.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::board::{Board, CardId, ColumnId, DropTarget, Label, Msg, reduce, visible};
use crate::look::{Theme, ghost};

/// The key the standalone binary stores the board under.
pub const STORAGE_KEY: &str = "board_plain";

/// How long the search box has to be quiet, and how far back undo goes. The
/// same numbers as the react-egui version.
const DEBOUNCE: f64 = 0.3;
const DEPTH: usize = 64;

/// The height of a column's footer, which is also its "drop at the end" zone.
const FOOTER_H: f32 = 26.0;
const SLOT_PAD: f32 = 4.0;

/// One card's own state. The react-egui version has this too — as three
/// `use_identity` hooks inside `<Card>`, where nothing else can see them.
#[derive(Clone, Debug, Default)]
struct CardUi {
    editing: bool,
    expanded: bool,
    draft_title: String,
    draft_body: String,
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
    #[serde(skip)]
    label: Option<Label>,

    /// The drag session: `use_dnd` written out.
    #[serde(skip)]
    carrying: Option<CardId>,
    #[serde(skip)]
    slots: Vec<(egui::Rect, DropTarget)>,
    #[serde(skip)]
    hovered: Option<DropTarget>,
    #[serde(skip)]
    pointer: Option<egui::Pos2>,

    /// Which column is being renamed, and to what.
    ///
    /// One at a time, unlike the react-egui version, where each `<Column>` has
    /// a `use_state` of its own and two could be open at once. A second map
    /// here would be a second thing to sweep, for a case nobody asked for —
    /// which is exactly the choice a caller is forced to make when the state
    /// of the parts has to live in the whole.
    #[serde(skip)]
    renaming: Option<(ColumnId, String)>,

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
            label: None,
            carrying: None,
            slots: Vec::new(),
            hovered: None,
            pointer: None,
            renaming: None,
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

/// The search box, the label filter, the counts and the history buttons.
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

        for tag in Label::ALL {
            let text = egui::RichText::new(tag.name())
                .small()
                .color(theme.label(tag));
            if ui
                .selectable_label(state.label == Some(tag), text)
                .clicked()
            {
                state.label = (state.label != Some(tag)).then_some(tag);
            }
        }

        ui.label(format!("{} cards", state.board.len()));
        // `<Text grow={1.0}>` before the buttons: here the buttons go in a
        // right-to-left `Ui` filling the rest of the row.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if state.dark { "light" } else { "dark" };
            if icon_button(ui, label, true) {
                state.dark = !state.dark;
                ui.ctx().set_visuals(if state.dark {
                    egui::Visuals::dark()
                } else {
                    egui::Visuals::light()
                });
            }
            *redo = icon_button(ui, "redo", !state.future.is_empty());
            *undo = icon_button(ui, "undo", !state.past.is_empty());
        });
    });
}

/// The four columns, side by side and equally wide.
fn columns(ui: &mut egui::Ui, state: &mut PlainState, theme: Theme, pending: &mut Vec<Msg>) {
    let ids: Vec<ColumnId> = state.board.columns.iter().map(|column| column.id).collect();
    let query = state.query.clone();
    let label = state.label;

    ui.columns(ids.len(), |uis| {
        for (ui, id) in uis.iter_mut().zip(ids) {
            column(ui, state, theme, id, &query, label, pending);
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
    label: Option<Label>,
    pending: &mut Vec<Msg>,
) {
    let Some(index) = state.board.columns.iter().position(|c| c.id == id) else {
        return;
    };
    let name = state.board.columns[index].name.clone();
    let total = state.board.columns[index].cards.len();
    let shown = visible(&state.board.columns[index], query, label);

    ui.spacing_mut().item_spacing.y = 6.0;
    ui.horizontal(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.spacing_mut().item_spacing.x = 4.0;
        match &mut state.renaming {
            Some((renaming, draft)) if *renaming == id => {
                let edit = ui.add(egui::TextEdit::singleline(draft));
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    pending.push(Msg::RenameColumn {
                        column: id,
                        name: draft.clone(),
                    });
                    state.renaming = None;
                }
            }
            _ => {
                ui.label(egui::RichText::new(&name).strong().color(theme.accent()));
            }
        }
        // The same reading order as the react-egui column header, where the
        // name has `grow={1.0}` and pushes the rest to the right.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("{}/{}", shown.len(), total));
            if icon_button(ui, "rename", true) {
                state.renaming = Some((id, name.clone()));
            }
        });
    });

    // The cards and, after them, the footer that adds one and catches a drop
    // meant for the end of the column — the same order as the react-egui
    // version, where the footer is inside the `<ScrollArea>` too.
    egui::ScrollArea::vertical().id_salt(id).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        for (i, card) in shown.iter().enumerate() {
            self::card(
                ui,
                state,
                theme,
                id,
                *card,
                shown.get(i + 1).copied(),
                pending,
            );
        }
        if shown.is_empty() {
            ui.label("nothing here");
        }

        let rect =
            egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), FOOTER_H));
        if state.hovered
            == Some(DropTarget {
                column: id,
                before: None,
            })
        {
            ui.painter().hline(
                rect.x_range(),
                rect.top(),
                egui::Stroke::new(2.0, theme.accent()),
            );
        }
        if state.carrying.is_some() {
            state.slots.push((
                rect,
                DropTarget {
                    column: id,
                    before: None,
                },
            ));
        }
        let button = egui::Button::new("+ card").wrap_mode(egui::TextWrapMode::Extend);
        if ui.put(rect, button).clicked() {
            pending.push(Msg::AddCard { column: id });
        }
    });
}

fn card(
    ui: &mut egui::Ui,
    state: &mut PlainState,
    theme: Theme,
    column: ColumnId,
    id: CardId,
    next: Option<CardId>,
    pending: &mut Vec<Msg>,
) {
    let Some(card) = state.board.card(id).cloned() else {
        return;
    };
    let before_me = state.hovered
        == Some(DropTarget {
            column,
            before: Some(id),
        });
    let carried = state.carrying == Some(id);
    // Every read of a card's own state goes through the map, and every write
    // has to put it back. This is the shape all of `plain.rs` takes.
    let ui_state = state.card_ui(id).clone();

    egui::Frame::new()
        .fill(theme.card())
        .corner_radius(4.0)
        .inner_margin(6.0)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.horizontal(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.spacing_mut().item_spacing.x = 6.0;
                let chip = egui::RichText::new(card.label.name())
                    .small()
                    .color(theme.label(card.label));
                if ui.selectable_label(false, chip).clicked() {
                    pending.push(Msg::SetLabel {
                        card: id,
                        label: card.label.next(),
                    });
                }

                let title = if carried {
                    egui::RichText::new(&card.title).weak()
                } else {
                    egui::RichText::new(&card.title)
                };
                let handle = ui.add(
                    egui::Label::new(title)
                        .truncate()
                        .sense(egui::Sense::click_and_drag()),
                );
                if handle.drag_started() {
                    state.carrying = Some(id);
                }
                if handle.clicked() {
                    state.card_ui(id).expanded = !ui_state.expanded;
                }
                if before_me {
                    let rect = handle.rect.expand(SLOT_PAD);
                    ui.painter().hline(
                        rect.x_range(),
                        rect.top(),
                        egui::Stroke::new(2.0, theme.accent()),
                    );
                }
                if state.carrying.is_some() {
                    let (top, bottom) = handle
                        .rect
                        .expand(SLOT_PAD)
                        .split_top_bottom_at_fraction(0.5);
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

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, "x", true) {
                        pending.push(Msg::RemoveCard { card: id });
                    }
                    let label = if ui_state.editing { "close" } else { "edit" };
                    if icon_button(ui, label, true) {
                        let entry = state.card_ui(id);
                        if !entry.editing {
                            entry.draft_title = card.title.clone();
                            entry.draft_body = card.body.clone();
                        }
                        entry.editing = !entry.editing;
                    }
                });
            });

            if ui_state.editing {
                let entry = state.card_ui(id);
                ui.add(
                    egui::TextEdit::singleline(&mut entry.draft_title)
                        .hint_text("title")
                        .desired_width(f32::INFINITY),
                );
                ui.add_sized(
                    egui::vec2(ui.available_width(), 54.0),
                    egui::TextEdit::multiline(&mut entry.draft_body).hint_text("body"),
                );
                let (title, body) = (entry.draft_title.clone(), entry.draft_body.clone());
                ui.horizontal(|ui| {
                    if ui.button("save").clicked() {
                        pending.push(Msg::EditCard {
                            card: id,
                            title,
                            body,
                        });
                        state.card_ui(id).editing = false;
                    }
                    if ui.button("cancel").clicked() {
                        state.card_ui(id).editing = false;
                    }
                });
            } else if ui_state.expanded && !card.body.is_empty() {
                ui.add(egui::Label::new(&card.body).wrap());
            }
        });
}

/// The plain twin of `<IconButton>`.
fn icon_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    let button = egui::Button::new(label)
        .small()
        .frame(false)
        .wrap_mode(egui::TextWrapMode::Extend);
    ui.add_enabled(enabled, button).clicked()
}
