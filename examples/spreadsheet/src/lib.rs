//! A spreadsheet: formulas over 26 x 10,000 cells.
//!
//! Type a number into a cell and every total that reads it moves. Type a
//! formula and the sheet is parsed again, ordered again, and evaluated again.
//! Those are two different jobs, and telling them apart is what this example
//! is about.
//!
//! **Two revisions, two memos.** `sheet.rs` sorts every message by which
//! counter it bumps: `structure_rev` when the *program* could have changed
//! (which cells are formulas, what they read, in what order they can be
//! evaluated) and `value_rev` when only the *numbers* did. The screen hangs one
//! derivation off each —
//!
//! ```text
//! structure_rev ─▶ use_memo ─▶ parse + dependency graph + order  (Compiled)
//! value_rev     ─▶ use_memo ─▶ a value per cell                  (Values)
//! ```
//!
//! — and the status bar prints how often each has run, so "typing 12 into a
//! literal cell does not re-parse the sheet" is a number a reader (and a test)
//! can see. Nothing watches for it; it falls out of what the deps of each memo
//! are. The same shape as `patch`'s `topology_rev` / `param_rev`, over a grid
//! instead of a graph.
//!
//! **Which state belongs to the item and which to the whole.** This is the
//! other half, and virtualization is what forces the question. A
//! `<VirtualList>` row that scrolls out of view is unmounted and its hooks are
//! swept, so any state kept in a cell is lost when the cell leaves the screen.
//! Three pieces of state, three homes:
//!
//! | state | home | why |
//! |---|---|---|
//! | the sheet: cell text, column widths | `use_undoable` + `use_persisted` at `App` | it is the document — undone, saved |
//! | the selection, the editor and its draft, the clipboard, a column edge being dragged | `use_reducer(ui_reduce)` at `App` | it has to survive the cell leaving the viewport. A draft that vanished because the user scrolled would be a bug |
//! | "focus me on my first frame", "flash, my value just changed" | `use_state` in the cell | state that is *supposed* to reset when the cell is remounted; losing it on scroll is the right answer |
//!
//! So the editor lives above the list and the cell is only told "you are being
//! edited, here is the draft". Scroll a half-typed formula off the screen and
//! back, and it is still there.
//!
//! **The row closure borrows nothing it could not borrow twice.** What
//! `<VirtualList>` calls per row captures the two memo results (both `&'s`, so
//! they coexist with anything), `Copy` snapshots of the selection, the width
//! vector and a draft clone — and no `State` guard at all. What a cell did goes
//! up as an event, the row turns it into a `UiMsg`, and the reducer is the only
//! thing that writes (ARCHITECTURE 3.7).
//!
//! The rest is the usual company: `use_persisted` keeps the sheet across
//! restarts, `board`'s `use_undoable` gives it a history, `provide_context`
//! hands both dispatchers to the tree, and the headers follow the body through
//! `<VirtualList on_scroll>`.
//!
//! No `Panel`, so the gallery can run it in a column of its own.

use std::cell::Cell as MutCell;

use board::hooks::{Undoable, use_undoable};
use egui_react::prelude::*;
use egui_react_elements::prelude::*;
use example_meta::Meta;

pub mod eval;
pub mod formula;
pub mod preset;
pub mod sheet;

use eval::{Compiled, Values};
use formula::Value;
use sheet::{CellRef, Msg, Range, Sheet, reduce};

pub use sheet::{COLS, DEFAULT_COL_W, ROWS};

/// The height of one row, and of the column header.
pub const ROW_H: f32 = 22.0;
/// The width of the row-number gutter on the left.
pub const ROW_HDR_W: f32 = 52.0;
/// How narrow a column may be dragged.
pub const MIN_COL_W: f32 = 24.0;
/// How long a cell stays tinted after its value changed.
pub const FLASH_SECS: f64 = 0.6;

pub const META: Meta = Meta {
    name: "spreadsheet",
    summary: "Formulas over 26 x 10,000 cells: two memo stages, and a draft that survives scrolling out of view.",
    hooks: &[
        "use_state",
        "use_reducer",
        "use_persisted",
        "use_memo",
        "use_handle",
        "provide_context",
        "use_context",
        "#[hook]",
        "Cx::leaf",
    ],
    elements: &[
        "View",
        "Text",
        "Button",
        "TextEdit",
        "VirtualList",
        "Separator",
    ],
    source: include_str!("lib.rs"),
    plain: None,
};

// ---------------------------------------------------------------------------
// 1. The UI state: everything that is not the document
// ---------------------------------------------------------------------------

/// What everything on screen sends to the sheet's reducer.
///
/// `Dispatch` is `Clone + Send + 'static`, which is what lets it travel by
/// context while a `State` guard could not (ARCHITECTURE 3.5).
pub type Actions = Dispatch<Undoable<Msg>>;

/// The same, for the UI state next door.
pub type UiActions = Dispatch<UiMsg>;

/// The cell being edited: which one, what has been typed into it, and whether
/// the field should select everything on its first frame.
///
/// The draft is here rather than in the cell because the cell may be unmounted
/// at any moment: `<VirtualList>` sweeps the hooks of a row that scrolls out of
/// view, and a draft that disappeared because the user looked at row 500 would
/// be a bug rather than a saving.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Editor {
    pub at: CellRef,
    pub draft: String,
    pub select_all: bool,
}

/// A copied rectangle, as raw text at the references it was copied from.
///
/// Snapshotted at copy time, so a paste needs nothing from the sheet: it is
/// `formula::shift` over these, by the distance between `range.from` and where
/// the cursor now is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clip {
    pub range: Range,
    pub cells: Vec<(CellRef, String)>,
}

/// Everything the screen knows that the document does not.
#[derive(Clone, Debug, PartialEq)]
pub struct UiState {
    /// Where a Shift-extend or a drag started.
    pub anchor: CellRef,
    /// The active cell. The selection is `Range::new(anchor, cursor)`.
    pub cursor: CellRef,
    pub editor: Option<Editor>,
    pub clipboard: Option<Clip>,
    /// The column being resized and its live width, while the drag lasts. The
    /// sheet is written once, on release.
    pub dragging_col: Option<(u16, f32)>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            anchor: CellRef::new(0, 0),
            cursor: CellRef::new(0, 0),
            editor: None,
            clipboard: None,
            dragging_col: None,
        }
    }
}

impl UiState {
    /// The rectangle the selection covers.
    pub fn selection(&self) -> Range {
        Range::new(self.anchor, self.cursor)
    }
}

/// How an edit ended, as the cell editor saw it.
///
/// Read from the field's own `Response` rather than from the app-level key
/// handler, because egui decides Tab and Escape in `Memory::begin_pass`, before
/// any of this code runs; see [`read_grid_keys`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    /// Enter: commit and step down.
    Down,
    /// Tab: commit and step right.
    Right,
    /// Escape: throw the draft away.
    Cancel,
    /// The focus went somewhere else: commit where we stand.
    Blur,
}

/// Everything that can change the UI state. One message per user action, the
/// same rule the sheet's own [`Msg`] follows.
#[derive(Clone, Debug, PartialEq)]
pub enum UiMsg {
    /// A click: anchor and cursor both move. Commits an open editor first.
    Select(CellRef),
    /// Shift-click or a drag across the grid: the cursor moves, the anchor
    /// stays where it was.
    Extend(CellRef),
    /// An arrow key. Clamped to the sheet, and commits an open editor first.
    Move {
        dcol: i32,
        drow: i32,
        extend: bool,
    },
    Edit {
        at: CellRef,
        draft: String,
        select_all: bool,
    },
    /// Every keystroke, from the cell editor or from the formula bar — they
    /// edit the same draft, which is why moving between them is not a commit.
    Draft(String),
    /// Write the draft and optionally step. `then: None` is "commit where we
    /// stand", which is what losing focus means.
    Commit {
        then: Option<(i32, i32)>,
    },
    Cancel,
    /// The text is snapshotted by the caller: the reducer has no sheet to read
    /// (see the note on [`ui_reduce`]).
    Copy(Clip),
    Paste,
    Clear,
    /// The live width while a column edge is dragged; `None` on release.
    DragCol(Option<(u16, f32)>),
}

/// Apply one UI message, returning the sheet message it implies.
///
/// The plan has this closure capture the sheet's `Dispatch` and send from
/// inside. Returning the message instead keeps the whole reducer a plain
/// function of its arguments — so the table of "what does Enter in a cell
/// actually do" is a unit test at the bottom of this file rather than a kittest
/// scenario — and `App` does the sending, one line up. No message ever implies
/// two.
///
/// A reducer that feeds another reducer is fine either way: `Dispatch::send`
/// only queues and asks for a repaint, and the sheet's reducer applies it at
/// its next visit, which is the next frame — the same one-frame delay every
/// handler write has (ARCHITECTURE 5.7).
pub fn ui_reduce(ui: &mut UiState, msg: UiMsg) -> Option<Msg> {
    match msg {
        UiMsg::Select(at) => {
            let done = commit(ui);
            ui.anchor = at;
            ui.cursor = at;
            done
        }

        UiMsg::Extend(at) => {
            // A drag across the grid reports this on every frame it moves, so
            // it must be cheap and idempotent: it is one assignment.
            if ui.cursor == at {
                return None;
            }
            let done = commit(ui);
            ui.cursor = at;
            done
        }

        UiMsg::Move { dcol, drow, extend } => {
            let done = commit(ui);
            let col = (i64::from(ui.cursor.col) + i64::from(dcol)).clamp(0, i64::from(COLS) - 1);
            let row = (i64::from(ui.cursor.row) + i64::from(drow)).clamp(0, i64::from(ROWS) - 1);
            ui.cursor = CellRef::new(col as u16, row as u32);
            if !extend {
                ui.anchor = ui.cursor;
            }
            done
        }

        UiMsg::Edit {
            at,
            draft,
            select_all,
        } => {
            // Opening an editor somewhere else commits the one that was open.
            let done = if ui.editor.as_ref().is_some_and(|e| e.at != at) {
                commit(ui)
            } else {
                None
            };
            ui.anchor = at;
            ui.cursor = at;
            ui.editor = Some(Editor {
                at,
                draft,
                select_all,
            });
            done
        }

        UiMsg::Draft(text) => {
            if let Some(editor) = &mut ui.editor {
                editor.draft = text;
                // The first frame is over; whatever the caret does now is the
                // user's business.
                editor.select_all = false;
            }
            None
        }

        UiMsg::Commit { then } => {
            let done = commit(ui);
            if let Some((dcol, drow)) = then {
                ui_reduce(
                    ui,
                    UiMsg::Move {
                        dcol,
                        drow,
                        extend: false,
                    },
                );
            }
            done
        }

        UiMsg::Cancel => {
            ui.editor = None;
            None
        }

        UiMsg::Copy(clip) => {
            ui.clipboard = Some(clip);
            None
        }

        UiMsg::Paste => {
            let clip = ui.clipboard.as_ref()?;
            let dcol = i32::from(ui.cursor.col) - i32::from(clip.range.from.col);
            let drow = ui.cursor.row as i32 - clip.range.from.row as i32;
            let cells: Vec<(CellRef, String)> = clip
                .cells
                .iter()
                .filter_map(|(at, text)| {
                    // A cell that would land outside the sheet is dropped; the
                    // references *inside* the text that would land outside
                    // become `#REF!`, which is `formula::shift`'s business.
                    let to = sheet::shifted(*at, dcol, drow)?;
                    Some((to, formula::shift(text, dcol, drow)))
                })
                .collect();
            (!cells.is_empty()).then_some(Msg::SetMany(cells))
        }

        UiMsg::Clear => {
            ui.editor = None;
            Some(Msg::Clear(ui.selection()))
        }

        UiMsg::DragCol(live) => {
            ui.dragging_col = live;
            None
        }
    }
}

/// Close the editor, if one is open, and say what to write.
fn commit(ui: &mut UiState) -> Option<Msg> {
    let editor = ui.editor.take()?;
    Some(Msg::Set {
        at: editor.at,
        text: editor.draft,
    })
}

/// The raw text of every cell in `range` that holds something.
///
/// This is what `Ctrl+C` puts in the clipboard. It is built by the caller
/// rather than by the reducer because the reducer has no sheet: the UI state
/// and the document are two reducers, and only one of them owns the text.
pub fn clip(sheet: &Sheet, range: Range) -> Clip {
    let mut cells: Vec<(CellRef, String)> = sheet
        .cells
        .iter()
        .filter(|(at, _)| range.contains(**at))
        .map(|(at, text)| (*at, text.clone()))
        .collect();
    cells.sort_by_key(|(at, _)| *at);
    Clip { range, cells }
}

// ---------------------------------------------------------------------------
// 2. The app
// ---------------------------------------------------------------------------

/// How often each memo stage has run.
///
/// `MutCell`s behind a `use_handle`, written through `Handle::with`: a
/// dirtying write would ask for a repaint from inside a memo closure and the
/// app would never idle (ARCHITECTURE 5.6). The numbers are only for the
/// status bar — and for the test that reads it, which is how "a literal edit
/// does not re-parse" is checked from the outside.
#[derive(Clone, Default)]
pub struct Counters {
    compiled: MutCell<u32>,
    evaluated: MutCell<u32>,
}

/// The dispatcher for the document, if this is drawn under one.
#[hook]
fn use_actions(cx: &mut Cx) -> Option<Actions> {
    use_context::<Actions>(cx).map(|actions| actions.get())
}

/// The dispatcher for the screen's own state.
#[hook]
fn use_ui(cx: &mut Cx) -> Option<UiActions> {
    use_context::<UiActions>(cx).map(|ui| ui.get())
}

fn send_ui(ui: &Option<UiActions>, msg: UiMsg) {
    if let Some(ui) = ui {
        ui.send(msg);
    }
}

/// The cell editor's egui id, fixed because there is only ever one.
///
/// Knowing it is what lets the formula bar and the grid tell "the focus went
/// to the other field" from "the focus went away".
fn editor_id() -> egui::Id {
    egui::Id::new("spreadsheet/cell-editor")
}

/// The formula bar's egui id, for the same reason.
fn formula_id() -> egui::Id {
    egui::Id::new("spreadsheet/formula-bar")
}

#[component]
pub fn App(cx: &mut Cx) {
    // The reducer owns the sheet and the persisted slot mirrors it, the way
    // `board` and `patch` do. `use_persisted` first, so its value is there to
    // seed the history on the very first frame.
    let mut saved = use_persisted(cx, "spreadsheet/sheet", Sheet::empty);
    let (history, dispatch) = use_undoable(cx, reduce, || saved.clone());
    if *saved != history.present {
        *saved = history.present.clone();
    }
    let actions = use_handle(cx, || dispatch.clone());

    // The screen's own state, in a reducer of its own. It sends to the sheet's
    // reducer, which is a queue and a repaint request, not a borrow.
    let to_sheet = dispatch.clone();
    let (ui_state, ui_dispatch) = use_reducer(
        cx,
        move |ui: &mut UiState, msg: UiMsg| {
            if let Some(msg) = ui_reduce(ui, msg) {
                to_sheet.send(Undoable::Do(msg));
            }
        },
        UiState::default,
    );
    let ui_handle = use_handle(cx, || ui_dispatch.clone());

    // How many times the history has been walked, which is the one number here
    // that never goes backwards.
    //
    // A revision counter does: undo restores an older sheet and its older
    // counter with it, so two different sheets can carry the same
    // `structure_rev` — undo a formula, then type a different one, and the
    // number is the one it already was. A memo compares its deps with the
    // *previous* value, and in practice a frame is drawn between any two
    // messages, so it would see the intermediate revision and recompute
    // anyway. That is a fact about timing rather than a property of the deps,
    // and it stops being true the moment two messages land in one visit to the
    // reducer. `(rev, epoch)` is monotonic, so the question does not arise.
    let mut epoch = use_state(cx, || 0u64);
    let counters = use_handle(cx, Counters::default);
    // Where the body is scrolled to, reported by the list itself and written
    // only when it changes (ARCHITECTURE 5.6). The headers are drawn before
    // the body, so they use the offset from the frame before — the standard
    // one-frame delay, and invisible at sixty frames a second.
    let mut offset = use_state(cx, egui::Vec2::default);
    let mut seeded = use_state(cx, || false);
    // Set on the frame the grid takes Tab, cleared on the next one; see
    // [`read_grid_keys`].
    let mut tabbed = use_state(cx, || false);

    if *tabbed {
        *tabbed = false;
        // egui gave the focus to the first widget that wanted it when the grid
        // took the Tab. Take it back, so the next key still reaches the grid.
        cx.ctx().memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
    }

    // Seeded once per mount, and only into a sheet nobody has touched, so a
    // saved sheet is never overwritten by the preset.
    if !*seeded {
        *seeded = true;
        if history.present.is_empty() {
            dispatch.send(Undoable::Do(Msg::Load(Box::new(preset::starter()))));
        }
    }

    let sheet = &history.present;
    let steps = *epoch;

    // Stage one: the program. Every formula parsed, every dependency
    // collected, the whole lot put in an order that can be walked once. Only a
    // *formula* can change any of that, which is what `structure_rev` counts.
    let compiled: &Compiled = use_memo(cx, (sheet.structure_rev, steps), || {
        counters.with(|c| c.compiled.set(c.compiled.get() + 1));
        eval::compile(sheet)
    });
    // Stage two: the numbers. Any keystroke can change these, and this is the
    // one that runs when a literal cell is typed into.
    let values: &Values = use_memo(cx, (sheet.value_rev, sheet.structure_rev, steps), || {
        counters.with(|c| c.evaluated.set(c.evaluated.get() + 1));
        eval::evaluate(sheet, compiled)
    });

    // Read once, into locals: an element may not hold a shared borrow of a
    // state *and* a handler that writes it (ARCHITECTURE 3.7).
    let cursor = ui_state.cursor;
    let selection = ui_state.selection();
    let editing = ui_state.editor.as_ref().map(|e| e.at);
    let draft = ui_state
        .editor
        .as_ref()
        .map_or_else(String::new, |e| e.draft.clone());
    let select_all = ui_state.editor.as_ref().is_some_and(|e| e.select_all);
    let dragging_col = ui_state.dragging_col;
    let seen = *offset;
    let (compiled_runs, evaluated_runs) = counters.with(|c| (c.compiled.get(), c.evaluated.get()));

    // The widths the grid is drawn at: the sheet's, with the column being
    // dragged overridden, so the body follows the drag live while the document
    // is written once, on release.
    let widths: Vec<f32> = (0..COLS)
        .map(|col| match dragging_col {
            Some((dragged, w)) if dragged == col => w,
            _ => sheet.width(col),
        })
        .collect();
    let total_w: f32 = widths.iter().sum();

    // The keys the grid takes, before anything is drawn.
    let focused = cx.ctx().memory(|m| m.focused());
    if focused.is_none() {
        let took_tab = {
            let keys = Keys {
                sheet,
                cursor,
                selection,
                ui: &ui_dispatch,
                actions: &dispatch,
            };
            read_grid_keys(cx.ui(), &keys)
        };
        if took_tab.walked {
            *epoch += 1;
        }
        if took_tab.tabbed {
            *tabbed = true;
        }
    }

    let formula_text = if editing.is_some() {
        draft.clone()
    } else {
        sheet.text(cursor).to_owned()
    };
    let status = format!(
        "{COLS} × {ROWS} · {} cells · compiled {compiled_runs} · evaluated {evaluated_runs}",
        sheet.len()
    );
    let inspector = inspect(sheet, compiled, values, cursor);
    let (can_undo, can_redo) = (history.can_undo(), history.can_redo());

    let view = rsx! {
        <View direction="column" grow={1.0} w="100%" h="100%" gap={0}>
            <Toolbar
                cursor={cursor}
                selection={selection}
                formula={formula_text.as_str()}
                status={status.as_str()}
                inspector={inspector.as_str()}
                can_undo={can_undo}
                can_redo={can_redo}
                on_undo={|| {
                    *epoch += 1;
                    dispatch.send(Undoable::Undo);
                }}
                on_redo={|| {
                    *epoch += 1;
                    dispatch.send(Undoable::Redo);
                }}
            />
            <Separator/>
            // The two headers and the body all read the same offset, which is
            // why the corner, the column letters and the row numbers stay
            // lined up with the cells while either axis scrolls.
            <View direction="row" w="100%" h={ROW_H} shrink={0.0}>
                <View w={ROW_HDR_W} h={ROW_H} shrink={0.0}/>
                <ColumnHeader
                    grow={1.0}
                    h={ROW_H}
                    widths={widths.as_slice()}
                    selection={selection}
                    offset_x={seen.x}
                />
            </View>
            <View direction="row" grow={1.0} h={0.0} w="100%">
                <RowHeader
                    w={ROW_HDR_W}
                    shrink={0.0}
                    h="100%"
                    selection={selection}
                    offset_y={seen.y}
                />
                // `grow` for the width and `h="100%"` for the height: inside a
                // row the main axis is horizontal, so `grow` says "take what is
                // left across" and the height has to be spelled out. A filling
                // leaf with an `auto` height would ask for the whole window and
                // push the toolbar off the top (ARCHITECTURE 6).
                <VirtualList
                    grow={1.0}
                    h="100%"
                    rows={ROWS as usize}
                    row_h={ROW_H}
                    row_w={total_w}
                    horizontal
                    on_scroll={|at: egui::Vec2| {
                        // Every frame, so only a change may write.
                        if *offset != at {
                            *offset = at;
                        }
                    }}
                    render={|cx: &mut Cx<'_, '_>, row: usize| {
                        rsx! {
                            <Row
                                row={row as u32}
                                widths={widths.as_slice()}
                                values={values}
                                selection={selection}
                                cursor={cursor}
                                editing={editing == Some(CellRef::new(0, row as u32)) || editing.is_some_and(|at| at.row == row as u32)}
                                editing_col={editing.map_or(COLS, |at| at.col)}
                                draft={draft.as_str()}
                                select_all={select_all}
                            />
                        }
                        .show(cx);
                    }}
                />
            </View>
        </View>
    };

    provide_context(cx, actions, |cx| {
        provide_context(cx, ui_handle, |cx| view.show(cx))
    });
}

/// What [`read_grid_keys`] needs to do its job.
struct Keys<'a> {
    sheet: &'a Sheet,
    cursor: CellRef,
    selection: Range,
    ui: &'a UiActions,
    actions: &'a Actions,
}

/// What one frame of key handling did that the caller has to finish.
struct Took {
    /// The history was walked, so `epoch` must be bumped.
    walked: bool,
    /// Tab was taken, so the focus egui handed out has to be taken back.
    tabbed: bool,
}

/// Every key the grid answers to, in one place, once a frame.
///
/// Only when nothing has focus: the formula bar, the name box and the cell
/// editor are ordinary `TextEdit`s and keep egui's own key handling while they
/// are focused. The gallery's search box is one of those too, so typing into it
/// never reaches the grid. The cells are deliberately `Sense::CLICK |
/// Sense::DRAG` and not `Sense::click_and_drag()`, which is the same thing plus
/// `FOCUSABLE`: a focusable cell would take the keyboard focus the moment it
/// was clicked and this handler would never run again.
///
/// The keys taken here are `consume_key`d so that nothing below reacts to them
/// as well. `Event::Text` is only read, because nothing else wants it.
///
/// Two keys are not here, and both for the same reason: egui's
/// `Memory::begin_pass` reads Tab and Escape out of the raw events *before* any
/// application code runs, and only the event filter of the widget that has
/// focus can stop it. So Escape and Enter inside the editor are read from the
/// editor's own `Response` (see [`Cell`]), and the Tab the grid takes here is
/// handed to egui as well — it gives the focus to the first widget that wants
/// it, and `App` takes it back on the next frame.
///
/// What is deliberately missing: the viewport does not follow the cursor.
/// `<VirtualList>` has no `scroll_to`, arrow keys move the cursor and the name
/// box jumps it, and either can walk it off the screen. That is the second
/// thing the element would need for an application; the first was `on_scroll`.
fn read_grid_keys(ui: &mut egui::Ui, keys: &Keys<'_>) -> Took {
    use egui::{Key, Modifiers};

    let mut took = Took {
        walked: false,
        tabbed: false,
    };
    let redo = Modifiers {
        shift: true,
        command: true,
        ..Modifiers::NONE
    };

    // Collected first and sent afterwards. `Dispatch::send` asks the context
    // for a repaint, and `input_mut` is holding the context's lock while the
    // closure runs: sending from inside it deadlocks.
    let mut messages: Vec<UiMsg> = Vec::new();
    let mut history: Vec<Undoable<Msg>> = Vec::new();

    // `consume_key` matches "at least these modifiers", not "exactly these"
    // (`Modifiers::matches_logically`), so every pair below is tried with the
    // more specific pattern first: asking for a bare arrow would otherwise
    // swallow the Shift-extend, and asking for Ctrl+Z would swallow
    // Ctrl+Shift+Z.
    ui.input_mut(|i| {
        for (key, dcol, drow) in [
            (Key::ArrowLeft, -1, 0),
            (Key::ArrowRight, 1, 0),
            (Key::ArrowUp, 0, -1),
            (Key::ArrowDown, 0, 1),
        ] {
            if i.consume_key(Modifiers::SHIFT, key) {
                messages.push(UiMsg::Move {
                    dcol,
                    drow,
                    extend: true,
                });
            } else if i.consume_key(Modifiers::NONE, key) {
                messages.push(UiMsg::Move {
                    dcol,
                    drow,
                    extend: false,
                });
            }
        }

        if i.consume_key(Modifiers::SHIFT, Key::Tab) {
            took.tabbed = true;
            messages.push(UiMsg::Move {
                dcol: -1,
                drow: 0,
                extend: false,
            });
        } else if i.consume_key(Modifiers::NONE, Key::Tab) {
            took.tabbed = true;
            messages.push(UiMsg::Move {
                dcol: 1,
                drow: 0,
                extend: false,
            });
        }

        // Enter and F2 open the editor on what is already there; typing a
        // character opens it on that character. That difference is the whole of
        // "type to replace, F2 to correct".
        if i.consume_key(Modifiers::NONE, Key::Enter) || i.consume_key(Modifiers::NONE, Key::F2) {
            messages.push(UiMsg::Edit {
                at: keys.cursor,
                draft: keys.sheet.text(keys.cursor).to_owned(),
                select_all: true,
            });
        }

        if i.consume_key(Modifiers::NONE, Key::Delete)
            || i.consume_key(Modifiers::NONE, Key::Backspace)
        {
            messages.push(UiMsg::Clear);
        }

        if i.consume_key(redo, Key::Z) || i.consume_key(Modifiers::COMMAND, Key::Y) {
            took.walked = true;
            history.push(Undoable::Redo);
        } else if i.consume_key(Modifiers::COMMAND, Key::Z) {
            took.walked = true;
            history.push(Undoable::Undo);
        }
        if i.consume_key(Modifiers::COMMAND, Key::C) {
            messages.push(UiMsg::Copy(clip(keys.sheet, keys.selection)));
        }
        if i.consume_key(Modifiers::COMMAND, Key::V) {
            messages.push(UiMsg::Paste);
        }

        // Type to edit. Read rather than consumed: nothing else is focused, so
        // there is nobody to take it from. Every character of the frame goes
        // into the draft, not just the first: a burst that arrives together —
        // a fast typist, a synthetic event queue — would otherwise lose all but
        // one of its characters before the editor exists to receive them.
        let typed: String = i
            .events
            .iter()
            .filter_map(|event| match event {
                egui::Event::Text(text)
                    if !text.is_empty() && !text.chars().any(char::is_control) =>
                {
                    Some(text.as_str())
                }
                _ => None,
            })
            .collect();
        if !typed.is_empty() {
            messages.push(UiMsg::Edit {
                at: keys.cursor,
                draft: typed,
                select_all: false,
            });
        }
    });

    for msg in messages {
        keys.ui.send(msg);
    }
    for msg in history {
        keys.actions.send(msg);
    }
    took
}

/// The line under the toolbar that says what the cursor cell is made of.
fn inspect(sheet: &Sheet, compiled: &Compiled, values: &Values, at: CellRef) -> String {
    let name = at.name();
    let text = sheet.text(at);
    let (shown, ..) = display(values.get(at));
    if text.is_empty() {
        return format!("{name} is empty");
    }
    if !sheet::is_formula(text) {
        return format!("{name} = {text}");
    }
    let reads = compiled.deps.get(&at).map_or(0, Vec::len);
    let arrow = if shown.is_empty() { "—" } else { &shown };
    format!("{name} = {} → {arrow} · reads {reads} cells", &text[1..])
}

/// A value as the grid draws it: the text, whether it is a number (so it is
/// right-aligned) and whether it is an error (so it is red).
pub fn display(value: &Value) -> (String, bool, bool) {
    match value {
        Value::Empty => (String::new(), false, false),
        Value::Num(n) => (number(*n), true, false),
        Value::Text(text) => (text.clone(), false, false),
        Value::Err(error) => (String::from(error.label()), false, true),
    }
}

/// A number as a spreadsheet writes it: whole ones without a point, the rest
/// to four decimals with the trailing zeros taken off.
fn number(n: f64) -> String {
    if n == 0.0 {
        // Which also catches `-0.0`, whose `{:.0}` is `-0`.
        return String::from("0");
    }
    if n == n.trunc() && n.abs() < 1e15 {
        return format!("{n:.0}");
    }
    let mut text = format!("{n:.4}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

// ---------------------------------------------------------------------------
// 3. The toolbar
// ---------------------------------------------------------------------------

/// Undo, redo, the name box, the formula bar and the two status lines.
///
/// It owns nothing but the text in the name box: everything else is handed to
/// it and everything it does goes out as a message.
#[component]
#[allow(clippy::too_many_arguments)]
fn Toolbar(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    cursor: CellRef,
    selection: Range,
    formula: &str,
    status: &str,
    inspector: &str,
    can_undo: bool,
    can_redo: bool,
    #[event] on_undo: (),
    #[event] on_redo: (),
) {
    let ui = use_ui(cx);
    let mut name_box = use_state(cx, String::new);
    // Where the selection is, as its hint: the box is empty until it is typed
    // into (`clear_on_submit` empties it again), so the hint is free to say
    // where the cursor is without fighting the user for the field.
    let shown = if selection.from == selection.to {
        cursor.name()
    } else {
        format!("{}:{}", selection.from.name(), selection.to.name())
    };
    let jump = ui.clone();
    let draft_to = ui.clone();
    let focus_to = ui.clone();

    rsx! {
        <View style={style} direction="column" w="100%" gap={4} p={6} shrink={0.0}>
            <View direction="row" gap={6} align="center" w="100%">
                <Button label="undo" enabled={can_undo} on_click={|| on_undo.emit(())}>"↶"</Button>
                <Button label="redo" enabled={can_redo} on_click={|| on_redo.emit(())}>"↷"</Button>
                <Separator vertical/>
                // The name box jumps the cursor, not the view: see the note on
                // `read_grid_keys` about what `<VirtualList>` cannot do yet.
                <TextEdit
                    w={80.0}
                    bind={name_box.bind()}
                    hint={shown.as_str()}
                    clear_on_submit
                    on_submit={|text: String| {
                        if let Some(range) = parse_range(&text) {
                            send_ui(&jump, UiMsg::Select(range.from));
                            if range.from != range.to {
                                send_ui(&jump, UiMsg::Extend(range.to));
                            }
                        }
                    }}
                />
                <FormulaBar
                    grow={1.0}
                    text={formula}
                    on_draft={|text: String| send_ui(&draft_to, UiMsg::Draft(text))}
                    // Clicking into the bar with nothing being edited opens an
                    // editor on the cursor cell holding what is already there,
                    // so the bar and the cell go on editing one draft.
                    on_focus={|| send_ui(&focus_to, UiMsg::Edit {
                        at: cursor,
                        draft: String::from(formula),
                        select_all: false,
                    })}
                />
            </View>
            <Text size={11.0}>{status}</Text>
            <Text size={11.0}>{inspector}</Text>
        </View>
    }
}

/// `A1` or `A1:C3`, as the name box reads it.
fn parse_range(text: &str) -> Option<Range> {
    let text = text.trim();
    match text.split_once(':') {
        Some((from, to)) => Some(Range::new(CellRef::parse(from)?, CellRef::parse(to)?)),
        None => CellRef::parse(text).map(Range::one),
    }
}

/// The formula bar: the cursor cell's raw text, or the draft while it is being
/// edited.
///
/// Hand-written rather than `<TextEdit>` because this one has to *report* the
/// text: the element's `bind` holds the only `&mut` to the string, so a handler
/// on the same element could not read it as well (ARCHITECTURE 3.7). The buffer
/// is a copy made each frame, which is what an immediate-mode text field is
/// happy with — the same shape as `patch`'s `SourceEdit`.
#[component]
fn FormulaBar(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    text: &str,
    #[event] on_draft: String,
    #[event] on_focus: (),
) {
    let mut buffer = text.to_owned();
    let (changed, gained) = cx.leaf(&style, |ui| {
        let response = ui.add(
            egui::TextEdit::singleline(&mut buffer)
                .id(formula_id())
                .hint_text("formula")
                .desired_width(ui.available_width()),
        );
        (response.changed(), response.gained_focus())
    });
    // Gaining focus opens an editor on the cursor cell if none is open, so the
    // bar and the cell edit one draft between them.
    if gained {
        on_focus.emit(());
    }
    if changed {
        on_draft.emit(buffer);
    }
}

// ---------------------------------------------------------------------------
// 4. The headers
// ---------------------------------------------------------------------------

/// The colours the grid is drawn in. Everything comes from the theme, so the
/// example follows the gallery's light / dark switch without a snapshot.
#[derive(Clone, Copy)]
struct Look {
    paper: egui::Color32,
    grid: egui::Color32,
    header: egui::Color32,
    text: egui::Color32,
    faint: egui::Color32,
    selection: egui::Color32,
    active: egui::Color32,
    flash: egui::Color32,
    error: egui::Color32,
}

fn look(ctx: &egui::Context) -> Look {
    let visuals = &ctx.style_of(ctx.theme()).visuals;
    Look {
        paper: visuals.extreme_bg_color,
        grid: visuals.widgets.noninteractive.bg_stroke.color,
        header: visuals.faint_bg_color,
        text: visuals.text_color(),
        faint: visuals.weak_text_color(),
        selection: visuals.selection.bg_fill,
        active: visuals.selection.stroke.color,
        flash: visuals.hyperlink_color,
        error: visuals.error_fg_color,
    }
}

/// `A`..`Z` across the top, and the six points on each edge that resize a
/// column.
///
/// Not a `<Canvas>`: `Canvas` hands `paint` one rect with one `sense`, and a
/// column edge needs an `interact` of its own. So it is a `cx.leaf_fill` with a
/// hand-written body, the third hatch down in `examples/escape-hatch`.
///
/// Clicking a letter does not select the column. The header is for widths;
/// selection is the grid's, and one job per surface keeps the drag
/// unambiguous.
#[component]
fn ColumnHeader(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    widths: &[f32],
    selection: Range,
    offset_x: f32,
) {
    let ui_actions = use_ui(cx);
    let actions = use_actions(cx);
    let look = look(cx.ctx());

    cx.leaf_fill(&style, move |ui| {
        let (rect, _response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(rect, 0.0, look.header);

        let font = egui::FontId::proportional(11.0);
        let mut x = rect.left() - offset_x;
        for (col, w) in widths.iter().enumerate() {
            let col = col as u16;
            let cell =
                egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(*w, rect.height()));
            x += w;
            if cell.right() < rect.left() || cell.left() > rect.right() {
                continue;
            }
            if (selection.from.col..=selection.to.col).contains(&col) {
                painter.rect_filled(cell, 0.0, look.selection.gamma_multiply(0.35));
            }
            painter.text(
                cell.center(),
                egui::Align2::CENTER_CENTER,
                CellRef::new(col, 0).name().trim_end_matches('1'),
                font.clone(),
                look.text,
            );
            painter.vline(
                cell.right(),
                rect.y_range(),
                egui::Stroke::new(1.0, look.grid),
            );

            // The six points either side of the edge that drag the width.
            let edge = egui::Rect::from_min_max(
                egui::pos2(cell.right() - 3.0, rect.top()),
                egui::pos2(cell.right() + 3.0, rect.bottom()),
            );
            // `Sense::DRAG`, not `Sense::drag()`: the latter is the same thing
            // plus `FOCUSABLE`, and a resize handle that can be tabbed to but
            // carries no name is a node a screen reader can only read out as
            // "unknown". Same reasoning as the cells, one screen down.
            let handle = ui.interact(edge, ui.id().with(("edge", col)), egui::Sense::DRAG);
            if handle.hovered() || handle.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
            }
            if handle.dragged() {
                let next = (*w + handle.drag_delta().x).max(MIN_COL_W);
                send_ui(&ui_actions, UiMsg::DragCol(Some((col, next))));
            }
            if handle.drag_stopped() {
                // One message for the whole drag, so undo walks it back in one
                // step (the same reasoning as `patch`'s `MoveNode`).
                if let Some(actions) = &actions {
                    actions.send(Undoable::Do(Msg::SetColWidth { col, w: *w }));
                }
                send_ui(&ui_actions, UiMsg::DragCol(None));
            }
        }
        painter.hline(
            rect.x_range(),
            rect.bottom(),
            egui::Stroke::new(1.0, look.grid),
        );
    });
}

/// `1`..`10000` down the left. A leaf that paints and senses nothing.
#[component]
fn RowHeader(cx: &mut Cx, #[prop(default)] style: ItemStyle, selection: Range, offset_y: f32) {
    let look = look(cx.ctx());

    cx.leaf_fill(&style, move |ui| {
        let (rect, _response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(rect, 0.0, look.header);

        let font = egui::FontId::proportional(11.0);
        let first = (offset_y / ROW_H).floor().max(0.0) as u32;
        let count = (rect.height() / ROW_H).ceil() as u32 + 1;
        for row in first..(first + count).min(ROWS) {
            let top = rect.top() + row as f32 * ROW_H - offset_y;
            let cell = egui::Rect::from_min_size(
                egui::pos2(rect.left(), top),
                egui::vec2(rect.width(), ROW_H),
            );
            if (selection.from.row..=selection.to.row).contains(&row) {
                painter.rect_filled(cell, 0.0, look.selection.gamma_multiply(0.35));
            }
            painter.text(
                cell.center(),
                egui::Align2::CENTER_CENTER,
                format!("{}", row + 1),
                font.clone(),
                look.text,
            );
            painter.hline(
                rect.x_range(),
                cell.bottom(),
                egui::Stroke::new(1.0, look.grid),
            );
        }
        painter.vline(
            rect.right(),
            rect.y_range(),
            egui::Stroke::new(1.0, look.grid),
        );
    });
}

// ---------------------------------------------------------------------------
// 5. The grid
// ---------------------------------------------------------------------------

/// One cell as the row worked it out, so that the `rsx!` below is a loop over
/// values rather than a loop with a body.
struct Shown {
    at: CellRef,
    w: f32,
    text: String,
    is_number: bool,
    is_error: bool,
}

/// One row of the grid: twenty-six cells and the messages they send.
///
/// The row is what turns "this cell was pressed" into a `UiMsg`, so a cell
/// holds no dispatcher and no state that matters. `use_ui` once per row is
/// twenty hook calls a frame; once per cell would be five hundred.
#[component]
#[allow(clippy::too_many_arguments)]
fn Row(
    cx: &mut Cx,
    row: u32,
    widths: &[f32],
    values: &Values,
    selection: Range,
    cursor: CellRef,
    editing: bool,
    editing_col: u16,
    draft: &str,
    select_all: bool,
) {
    let ui_actions = use_ui(cx);
    let shown: Vec<Shown> = (0..COLS)
        .map(|col| {
            let at = CellRef::new(col, row);
            let (text, is_number, is_error) = display(values.get(at));
            Shown {
                at,
                w: widths.get(col as usize).copied().unwrap_or(DEFAULT_COL_W),
                text,
                is_number,
                is_error,
            }
        })
        .collect();

    rsx! {
        <View direction="row" w="100%" h={ROW_H}>
            for cell in &shown {
                <Cell
                    key={cell.at.col}
                    at={cell.at}
                    width={cell.w}
                    text={cell.text.as_str()}
                    is_number={cell.is_number}
                    is_error={cell.is_error}
                    selected={selection.contains(cell.at)}
                    active={cell.at == cursor}
                    editing={editing && cell.at.col == editing_col}
                    draft={draft}
                    select_all={select_all}
                    on_press={|| send_ui(&ui_actions, UiMsg::Select(cell.at))}
                    on_shift_press={|| send_ui(&ui_actions, UiMsg::Extend(cell.at))}
                    on_drag_over={|| send_ui(&ui_actions, UiMsg::Extend(cell.at))}
                    on_double={|| send_ui(&ui_actions, UiMsg::Edit {
                        at: cell.at,
                        draft: String::new(),
                        select_all: true,
                    })}
                    on_draft={|text: String| send_ui(&ui_actions, UiMsg::Draft(text))}
                    on_finish={|how: Finish| send_ui(&ui_actions, match how {
                        Finish::Down => UiMsg::Commit { then: Some((0, 1)) },
                        Finish::Right => UiMsg::Commit { then: Some((1, 0)) },
                        Finish::Cancel => UiMsg::Cancel,
                        Finish::Blur => UiMsg::Commit { then: None },
                    })}
                />
            }
        </View>
    }
}

/// One cell: a rectangle, a value, and — when it is the one being edited — a
/// text field on top of it.
///
/// A single `cx.leaf`, because a cell is one rectangle however much is going on
/// in it. The two `use_state`s here are the ones that *should* be lost when the
/// row scrolls away: "flash, my value just changed" and "this is the editor's
/// first frame, take the focus".
#[component]
#[allow(clippy::too_many_arguments)]
fn Cell(
    cx: &mut Cx,
    at: CellRef,
    // Not `w`: `rsx!` treats every layout shorthand as an `ItemStyle`
    // attribute, so a prop called `w` would be packed into `style` instead of
    // reaching the component.
    width: f32,
    text: &str,
    is_number: bool,
    is_error: bool,
    selected: bool,
    active: bool,
    editing: bool,
    draft: &str,
    select_all: bool,
    #[event] on_press: (),
    #[event] on_shift_press: (),
    #[event] on_drag_over: (),
    #[event] on_double: (),
    #[event] on_draft: String,
    #[event] on_finish: Finish,
) {
    let look = look(cx.ctx());
    let now = cx.ui().input(|i| i.time);

    // The value this cell showed last, and when it changed. Seeded with what
    // is on screen now, so a cell that scrolls into view arrives calm — which
    // is the whole reason this state is allowed to be cell-local.
    let mut last = use_state(cx, || (text.to_owned(), None::<f64>));
    if last.0 != text {
        *last = (text.to_owned(), Some(now));
    }
    let flash = last.1.map_or(0.0, |when| {
        (1.0 - (now - when) / FLASH_SECS).clamp(0.0, 1.0)
    }) as f32;
    if flash > 0.0 {
        cx.ctx().request_repaint();
    }

    let mut was_editing = use_state(cx, || false);
    let first = editing && !*was_editing;
    if *was_editing != editing {
        *was_editing = editing;
    }

    let mut buffer = draft.to_owned();
    let style = ItemStyle::default().w(width).h(ROW_H).shrink(0.0);
    let out = cx.leaf(&style, |ui| {
        // Not `Sense::click_and_drag()`: that one is focusable, and a cell that
        // took the keyboard focus would silence the grid's key handler.
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(width, ROW_H),
            egui::Sense::CLICK | egui::Sense::DRAG,
        );
        let painter = ui.painter().with_clip_rect(rect);

        painter.rect_filled(rect, 0.0, look.paper);
        if selected {
            painter.rect_filled(rect, 0.0, look.selection.gamma_multiply(0.25));
        }
        if flash > 0.0 {
            painter.rect_filled(rect, 0.0, look.flash.gamma_multiply(0.30 * flash));
        }
        let stroke = egui::Stroke::new(1.0, look.grid);
        painter.vline(rect.right(), rect.y_range(), stroke);
        painter.hline(rect.x_range(), rect.bottom(), stroke);

        if !editing {
            let colour = if is_error {
                look.error
            } else if text.is_empty() {
                look.faint
            } else {
                look.text
            };
            let (anchor, x) = if is_number {
                (egui::Align2::RIGHT_CENTER, rect.right() - 4.0)
            } else {
                (egui::Align2::LEFT_CENTER, rect.left() + 4.0)
            };
            painter.text(
                egui::pos2(x, rect.center().y),
                anchor,
                text,
                egui::FontId::proportional(12.0),
                colour,
            );
        }
        if active {
            painter.rect_stroke(
                rect.shrink(1.0),
                0.0,
                egui::Stroke::new(2.0, look.active),
                egui::StrokeKind::Inside,
            );
        }

        // The name in the accessibility tree, which is also what a test looks
        // the cell up by, and the shown value as the node's value: the text is
        // painted, so without this a screen reader — and a test — would be told
        // there is a cell called `B2` and never what is in it. Only the cells
        // in view are in the tree, a few hundred of them.
        response.widget_info(|| {
            let mut info = egui::WidgetInfo::labeled(egui::WidgetType::Other, true, at.name());
            info.current_text_value = Some(text.to_owned());
            info
        });

        let mut changed = false;
        let mut finish = None;
        if editing {
            // `lock_focus`, so that Tab neither moves the focus nor inserts a
            // tab character: a single-line field ignores it, and egui's
            // `begin_pass` skips it because the field's event filter claims it.
            // That is what lets Tab mean "commit and step right".
            let field = ui.put(
                rect,
                egui::TextEdit::singleline(&mut buffer)
                    .id(editor_id())
                    .lock_focus(true)
                    .margin(egui::Margin::symmetric(3, 2)),
            );
            ui.ctx().accesskit_node_builder(field.id, |node| {
                node.set_label(format!("edit {}", at.name()));
            });
            if first {
                field.request_focus();
                let mut state = egui::TextEdit::load_state(ui.ctx(), field.id).unwrap_or_default();
                let end = egui::text::CCursor::new(buffer.chars().count());
                let range = if select_all {
                    egui::text::CCursorRange::two(egui::text::CCursor::new(0), end)
                } else {
                    egui::text::CCursorRange::one(end)
                };
                state.cursor.set_char_range(Some(range));
                state.store(ui.ctx(), field.id);
            }
            changed = field.changed();
            if field.has_focus()
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab))
            {
                finish = Some(Finish::Right);
            } else if field.lost_focus() {
                let (enter, escape) = ui.input(|i| {
                    (
                        i.key_pressed(egui::Key::Enter),
                        i.key_pressed(egui::Key::Escape),
                    )
                });
                let to_bar = ui.ctx().memory(|m| m.focused()) == Some(formula_id());
                finish = if escape {
                    Some(Finish::Cancel)
                } else if enter {
                    Some(Finish::Down)
                } else if to_bar {
                    // The bar and the cell edit one draft; moving between them
                    // is not the end of the edit.
                    None
                } else {
                    Some(Finish::Blur)
                };
            }
        }

        let dragging_over = ui.rect_contains_pointer(rect)
            && ui.input(|i| i.pointer.is_decidedly_dragging() && i.pointer.primary_down());
        let shift = ui.input(|i| i.modifiers.shift);
        Painted {
            pressed: response.clicked() || response.drag_started(),
            shift,
            dragging_over,
            double: response.double_clicked(),
            changed,
            finish,
        }
    });

    // Nothing is picked up and nothing is dropped, so `board`'s `use_dnd` is
    // the wrong tool for a drag-select: a cell only reports "the pointer is
    // over me with the button down" and the row extends the selection.
    if out.dragging_over && !out.pressed {
        on_drag_over.emit(());
    }
    if out.pressed {
        if out.shift {
            on_shift_press.emit(());
        } else {
            on_press.emit(());
        }
    }
    if out.double {
        on_double.emit(());
    }
    if out.changed {
        on_draft.emit(buffer);
    }
    if let Some(how) = out.finish {
        on_finish.emit(how);
    }
}

/// What one cell's leaf reports back out of the `Ui` closure.
struct Painted {
    pressed: bool,
    shift: bool,
    dragging_over: bool,
    double: bool,
    changed: bool,
    finish: Option<Finish>,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::formula::Error;

    fn cell(name: &str) -> CellRef {
        CellRef::parse(name).expect("a reference the test wrote")
    }

    fn state_at(name: &str) -> UiState {
        UiState {
            anchor: cell(name),
            cursor: cell(name),
            ..UiState::default()
        }
    }

    fn editing(at: &str, draft: &str) -> UiState {
        UiState {
            anchor: cell(at),
            cursor: cell(at),
            editor: Some(Editor {
                at: cell(at),
                draft: String::from(draft),
                select_all: false,
            }),
            ..UiState::default()
        }
    }

    #[test]
    fn a_click_moves_both_ends_of_the_selection() {
        let mut ui = state_at("A1");
        assert_eq!(ui_reduce(&mut ui, UiMsg::Select(cell("C3"))), None);
        assert_eq!(ui.cursor, cell("C3"));
        assert_eq!(ui.selection(), Range::one(cell("C3")));
        // Shift-click leaves the anchor where it was.
        assert_eq!(ui_reduce(&mut ui, UiMsg::Extend(cell("E5"))), None);
        assert_eq!(ui.anchor, cell("C3"));
        assert_eq!(ui.selection(), Range::new(cell("C3"), cell("E5")));
    }

    #[test]
    fn the_cursor_stops_at_the_edges_of_the_sheet() {
        let mut ui = state_at("A1");
        ui_reduce(
            &mut ui,
            UiMsg::Move {
                dcol: -1,
                drow: -1,
                extend: false,
            },
        );
        assert_eq!(ui.cursor, cell("A1"), "there is nothing above or left");

        ui.cursor = cell("Z10000");
        ui.anchor = ui.cursor;
        ui_reduce(
            &mut ui,
            UiMsg::Move {
                dcol: 1,
                drow: 1,
                extend: false,
            },
        );
        assert_eq!(ui.cursor, cell("Z10000"), "nor below or right");
    }

    #[test]
    fn shift_and_an_arrow_grows_the_selection() {
        let mut ui = state_at("B2");
        ui_reduce(
            &mut ui,
            UiMsg::Move {
                dcol: 1,
                drow: 0,
                extend: true,
            },
        );
        assert_eq!(ui.anchor, cell("B2"));
        assert_eq!(ui.cursor, cell("C2"));
        assert_eq!(ui.selection(), Range::new(cell("B2"), cell("C2")));
    }

    #[test]
    fn moving_away_from_an_open_editor_writes_what_was_typed() {
        let mut ui = editing("B2", "=1+1");
        let msg = ui_reduce(
            &mut ui,
            UiMsg::Move {
                dcol: 0,
                drow: 1,
                extend: false,
            },
        );
        assert_eq!(
            msg,
            Some(Msg::Set {
                at: cell("B2"),
                text: String::from("=1+1"),
            })
        );
        assert!(ui.editor.is_none());
        assert_eq!(ui.cursor, cell("B3"));

        // And so does clicking somewhere else.
        let mut ui = editing("B2", "7");
        let msg = ui_reduce(&mut ui, UiMsg::Select(cell("D4")));
        assert!(matches!(msg, Some(Msg::Set { .. })));
        assert_eq!(ui.cursor, cell("D4"));
    }

    #[test]
    fn enter_commits_and_steps_down_tab_commits_and_steps_right() {
        let mut ui = editing("B2", "12");
        let msg = ui_reduce(&mut ui, UiMsg::Commit { then: Some((0, 1)) });
        assert_eq!(
            msg,
            Some(Msg::Set {
                at: cell("B2"),
                text: String::from("12"),
            })
        );
        assert_eq!(ui.cursor, cell("B3"));

        let mut ui = editing("B2", "12");
        ui_reduce(&mut ui, UiMsg::Commit { then: Some((1, 0)) });
        assert_eq!(ui.cursor, cell("C2"));
    }

    #[test]
    fn escape_throws_the_draft_away() {
        let mut ui = editing("B2", "nonsense");
        assert_eq!(ui_reduce(&mut ui, UiMsg::Cancel), None);
        assert!(ui.editor.is_none(), "and writes nothing");
        assert_eq!(ui.cursor, cell("B2"), "and does not move");
    }

    #[test]
    fn a_draft_reaches_the_editor_whichever_field_typed_it() {
        let mut ui = editing("B2", "1");
        ui_reduce(&mut ui, UiMsg::Draft(String::from("12")));
        assert_eq!(ui.editor.as_ref().expect("still open").draft, "12");
        assert!(
            !ui.editor.as_ref().expect("still open").select_all,
            "the first frame is over"
        );
        // With nothing open there is nothing to type into.
        let mut ui = state_at("B2");
        ui_reduce(&mut ui, UiMsg::Draft(String::from("12")));
        assert!(ui.editor.is_none());
    }

    #[test]
    fn opening_an_editor_elsewhere_commits_the_one_that_was_open() {
        let mut ui = editing("B2", "7");
        let msg = ui_reduce(
            &mut ui,
            UiMsg::Edit {
                at: cell("C3"),
                draft: String::from("x"),
                select_all: false,
            },
        );
        assert_eq!(
            msg,
            Some(Msg::Set {
                at: cell("B2"),
                text: String::from("7"),
            })
        );
        assert_eq!(ui.editor.as_ref().expect("the new one").at, cell("C3"));
        assert_eq!(ui.cursor, cell("C3"));
    }

    #[test]
    fn delete_clears_the_whole_selection() {
        let mut ui = state_at("B2");
        ui.cursor = cell("D4");
        let msg = ui_reduce(&mut ui, UiMsg::Clear);
        assert_eq!(msg, Some(Msg::Clear(Range::new(cell("B2"), cell("D4")))));
    }

    #[test]
    fn a_paste_shifts_every_reference_by_where_it_lands() {
        let sheet =
            Sheet::from_cells([(cell("A1"), "1"), (cell("B1"), "2"), (cell("C1"), "=A1+B1")]);
        let mut ui = state_at("A1");
        ui.cursor = cell("C1");
        ui.anchor = cell("C1");
        let copied = clip(&sheet, ui.selection());
        ui_reduce(&mut ui, UiMsg::Copy(copied));

        ui_reduce(&mut ui, UiMsg::Select(cell("C2")));
        let msg = ui_reduce(&mut ui, UiMsg::Paste);
        assert_eq!(
            msg,
            Some(Msg::SetMany(vec![(cell("C2"), String::from("=A2+B2"))]))
        );
    }

    #[test]
    fn a_paste_that_would_leave_the_sheet_writes_the_error_into_the_text() {
        let sheet = Sheet::from_cells([(cell("B2"), "=A2")]);
        let mut ui = state_at("B2");
        let copied = clip(&sheet, ui.selection());
        ui_reduce(&mut ui, UiMsg::Copy(copied));
        ui_reduce(&mut ui, UiMsg::Select(cell("A2")));
        let msg = ui_reduce(&mut ui, UiMsg::Paste);
        assert_eq!(
            msg,
            Some(Msg::SetMany(vec![(cell("A2"), String::from("=#REF!"))]))
        );
    }

    #[test]
    fn pasting_nothing_says_nothing() {
        let mut ui = state_at("A1");
        assert_eq!(ui_reduce(&mut ui, UiMsg::Paste), None);
    }

    #[test]
    fn a_column_drag_is_live_in_the_ui_and_lands_in_the_sheet_once() {
        let mut ui = state_at("A1");
        assert_eq!(ui_reduce(&mut ui, UiMsg::DragCol(Some((2, 130.0)))), None);
        assert_eq!(ui.dragging_col, Some((2, 130.0)));
        assert_eq!(ui_reduce(&mut ui, UiMsg::DragCol(None)), None);
        assert_eq!(ui.dragging_col, None);
    }

    #[test]
    fn the_name_box_reads_a_cell_or_a_rectangle() {
        assert_eq!(parse_range("b2"), Some(Range::one(cell("B2"))));
        assert_eq!(
            parse_range(" A1:C3 "),
            Some(Range::new(cell("A1"), cell("C3")))
        );
        assert_eq!(
            parse_range("C3:A1"),
            Some(Range::new(cell("A1"), cell("C3")))
        );
        assert_eq!(parse_range("AA1"), None);
        assert_eq!(parse_range(""), None);
        assert_eq!(parse_range("A1:"), None);
    }

    #[test]
    fn a_number_is_shown_the_way_a_spreadsheet_shows_it() {
        assert_eq!(
            display(&Value::Num(12.0)),
            (String::from("12"), true, false)
        );
        assert_eq!(display(&Value::Num(-0.0)).0, "0");
        assert_eq!(display(&Value::Num(2.5)).0, "2.5");
        assert_eq!(display(&Value::Num(1.0 / 3.0)).0, "0.3333");
        assert_eq!(display(&Value::Num(2.10)).0, "2.1");
        assert_eq!(display(&Value::Empty), (String::new(), false, false));
        assert_eq!(
            display(&Value::Text(String::from("hi"))),
            (String::from("hi"), false, false)
        );
        assert_eq!(
            display(&Value::Err(Error::Cycle)),
            (String::from("#CYCLE!"), false, true)
        );
    }

    #[test]
    fn the_inspector_says_what_the_cursor_cell_is_made_of() {
        let sheet = Sheet::from_cells([
            (cell("A1"), "2"),
            (cell("A2"), "3"),
            (cell("B1"), "=SUM(A1:A2)"),
        ]);
        let compiled = eval::compile(&sheet);
        let values = eval::evaluate(&sheet, &compiled);
        assert_eq!(
            inspect(&sheet, &compiled, &values, cell("B1")),
            "B1 = SUM(A1:A2) → 5 · reads 2 cells"
        );
        assert_eq!(inspect(&sheet, &compiled, &values, cell("A1")), "A1 = 2");
        assert_eq!(
            inspect(&sheet, &compiled, &values, cell("Z9")),
            "Z9 is empty"
        );
    }
}
