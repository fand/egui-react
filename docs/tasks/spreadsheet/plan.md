# Plan: spreadsheet

The task definition is in [task.md](task.md). The design rationale is in [docs/ARCHITECTURE.md](../../ARCHITECTURE.md). If you decide to depart from this plan during implementation, update this document, and update ARCHITECTURE.md too if the change has design meaning. Section 8 is written at the end: what could not be written, how it was worked around, what the library should grow.

## 0. Overview

```
examples/spreadsheet/src/
  lib.rs       App, the grid, the headers, the editor, the toolbar (this is what the gallery shows)
  sheet.rs     Sheet / CellRef / Msg / reduce. Pure, no egui, with unit tests
  formula.rs   Tokenizer, parser, Expr, Value, reference shifting. Pure, with unit tests
  eval.rs      compile (parse every formula, dependency graph, order) and evaluate. Pure, with unit tests
  preset.rs    The starter sheet, built in code
  main.rs      run(Options { title, .. }, |_cx| rsx! { <App/> })
```

Same split as board and patch: everything the gallery's code pane should tell the reader is in `lib.rs`; everything that can be tested as a string or a number is in a file that never mentions egui.

Constants, in `lib.rs`: `COLS = 26`, `ROWS = 10_000`, `ROW_H = 22.0`, `DEFAULT_COL_W = 80.0`, `ROW_HDR_W = 48.0`, `MIN_COL_W = 24.0`, `FLASH_SECS = 0.6`. (Implemented as: `COLS` / `ROWS` / `DEFAULT_COL_W` live in `sheet.rs`, because `CellRef::parse` and `Sheet::empty` need them without a screen, and `lib.rs` re-exports them; `ROW_HDR_W` is 52.0, which is what four digits and the gutter came to. Section 8.12.)

## 1. Data model (`sheet.rs`)

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, PartialOrd, Ord)]
pub struct CellRef { pub col: u16, pub row: u32 }        // 0-based; `A1` is (0, 0)

impl CellRef {
    pub fn name(self) -> String;                          // "B2"
    pub fn parse(s: &str) -> Option<CellRef>;             // "B2", case-insensitive; None outside COLS x ROWS
}

/// A rectangle of cells, normalised so `from <= to` on both axes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Range { pub from: CellRef, pub to: CellRef }

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    /// Raw text as typed. Absent means empty. A cell is never stored as "".
    pub cells: HashMap<CellRef, String>,
    /// Per column, in points. `Vec` of `COLS`.
    pub col_w: Vec<f32>,
    /// +1 when a formula changed: text starting with `=` was set, removed or replaced,
    /// or a cell switched between literal and formula. deps of the compile memo.
    pub structure_rev: u64,
    /// +1 on any change to a cell's text. deps of the evaluate memo.
    pub value_rev: u64,
}

pub enum Msg {
    /// One cell. Empty text removes it.
    Set { at: CellRef, text: String },
    /// Several cells in one undo step (paste).
    SetMany(Vec<(CellRef, String)>),
    /// Every cell in the range (Delete key).
    Clear(Range),
    SetColWidth { col: u16, w: f32 },
    /// Replace the sheet with the preset. Bumps both revs past the current values.
    Load(Box<Sheet>),
}

pub fn reduce(sheet: &mut Sheet, msg: Msg);
pub fn is_formula(text: &str) -> bool;   // starts with '='
```

Which rev `reduce` bumps is the core of this example, and a unit test names it per message:

| Msg | `structure_rev` | `value_rev` |
|---|---|---|
| `Set` where old and new are both literals (or one is empty and the other a literal) | – | +1 |
| `Set` where old or new is a formula | +1 | +1 |
| `SetMany` / `Clear` | +1 if any cell in it is a formula before or after, else – | +1 if anything changed |
| `SetColWidth` | – | – |
| `Load` | +1 | +1 |

A message that changes nothing (same text, same width) bumps nothing, so `use_undoable`'s "no step for a no-op" rule holds and the memos do not rerun. `col_w` is in the sheet so that it is persisted and undone with everything else; the header sends `SetColWidth` once on release, not per pixel (same reasoning as patch's `MoveNode`).

`Sheet` is `Clone + PartialEq` for `use_undoable`. A sheet of a few thousand cells clones in microseconds; the history depth is board's 64.

## 2. Formulas (`formula.rs`)

```rust
pub enum Expr {
    Num(f64), Str(String), Ref(CellRef), Range(CellRef, CellRef),
    Neg(Box<Expr>), Bin(Op, Box<Expr>, Box<Expr>), Call(Func, Vec<Expr>),
}
pub enum Op { Add, Sub, Mul, Div, Pow, Eq, Ne, Lt, Le, Gt, Ge }
pub enum Func { Sum, Avg, Min, Max, Count, If }

#[derive(Clone, PartialEq, Debug)]
pub enum Value { Empty, Num(f64), Text(String), Err(Error) }
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error { Parse, Ref, Cycle, Div0, Name, Value }
impl Error { pub fn label(self) -> &'static str }   // "#PARSE!" "#REF!" "#CYCLE!" "#DIV/0!" "#NAME?" "#VALUE!"

/// `text` without the leading '='.
pub fn parse(text: &str) -> Result<Expr, Error>;
/// Every cell the expression reads, with ranges expanded. Sorted, deduplicated.
pub fn refs(expr: &Expr) -> Vec<CellRef>;
/// The same text with every reference moved by (dcol, drow). References that would
/// leave the sheet become `#REF!` in the text. Works on tokens, so spacing is kept.
pub fn shift(text: &str, dcol: i32, drow: i32) -> String;
/// A literal cell's value: a number if it parses as one, else text, else Empty.
pub fn literal(text: &str) -> Value;
```

- Recursive descent with precedence: comparison < `+ -` < `* /` < unary `-` < `^` (right-associative) < atoms. `Range` is only valid as a function argument; elsewhere it is `Error::Value` at evaluation, not a parse error, so `=A1:A3` shows `#VALUE!`.
- An unknown function name is `Error::Name` at parse time. Wrong arity for `IF` is `Error::Value` at evaluation.
- Evaluation of one expression lives in `eval.rs` (it needs the values of other cells); this file is syntax only.
- `Value` arithmetic: `Empty` counts as 0 in arithmetic and is skipped by `SUM` / `AVG` / `MIN` / `MAX` / `COUNT`. `Text` in arithmetic is `Error::Value`. Comparison of two texts compares strings; text against number is `Error::Value`. Any `Err` operand propagates that error. `IF(cond, a, b)` evaluates both branches (no laziness needed; the dependency graph already contains both).
- `shift` is what copy / paste uses. It re-emits the token stream with `Ref` and `Range` tokens moved, so `= A1 + 1` pastes as `= B1 + 1`.

## 3. Compilation and evaluation (`eval.rs`)

```rust
pub struct Compiled {
    /// Every formula cell, parsed. Literals are not here.
    pub formulas: HashMap<CellRef, Result<Expr, Error>>,
    /// Formula cells in an order where dependencies come first. Cells on a cycle are absent.
    pub order: Vec<CellRef>,
    /// Formula cells that are on, or downstream of, a cycle.
    pub cyclic: HashSet<CellRef>,
    /// Per formula cell, the cells it reads (for the inspector line in the status bar, and tests).
    pub deps: HashMap<CellRef, Vec<CellRef>>,
}
pub fn compile(sheet: &Sheet) -> Compiled;

pub struct Values {
    pub cells: HashMap<CellRef, Value>,   // formula cells and literal cells; absent means Empty
}
pub fn evaluate(sheet: &Sheet, compiled: &Compiled) -> Values;
```

- `compile` parses each formula cell, collects `refs`, and runs a DFS with three colours to find cycles and produce a topological order. A reference to a cell that is not a formula (a literal or empty) is a leaf and needs no ordering. A cell downstream of a cycle is not on the cycle but cannot be evaluated; it goes into `cyclic` too, and shows `#CYCLE!` (a spreadsheet would show the error propagating; this is the same result by a shorter route).
- `evaluate` writes every literal first (`formula::literal`), then walks `order`, evaluating each expression against the values so far. `cyclic` cells get `Value::Err(Cycle)`. A parse failure stored in `formulas` gives `Value::Err(Parse)` (or `Name`).
- Deliberately not incremental. A `Values` for the preset (about 400 cells) is a few microseconds; a sheet with 10,000 formulas is still under a millisecond. Section 8 gets a note if that turns out wrong.
- Unit tests: precedence and associativity (`2^3^2`, `-2^2`, `1+2*3`), every function on a range with empties and text mixed in, every `Error`, a diamond (`D1 = B1 + C1`, both from `A1`) evaluates `A1` before both, a two-cell cycle and a self-reference give `Cycle` and leave unrelated cells correct, a cell downstream of a cycle, `shift` in every direction and off the edge, `CellRef::parse` on `A1` / `z9999` / `AA1` (None: only 26 columns) / `A0` (None).

## 4. Screen (`lib.rs`)

```
App
└ SheetProvider          provide_context: Actions (Dispatch<Undoable<Msg>>), Ui (Dispatch<UiMsg>), Counters
  └ View column grow gap=0
    ├ Toolbar             undo / redo · name box · formula bar · status
    ├ View row            ★ 4.2: corner + column header (a leaf, painted from the body's x offset)
    └ View row grow h=0
      ├ RowHeader (w=ROW_HDR_W)   a leaf, painted from the body's y offset
      └ VirtualList grow h=0 horizontal rows=ROWS row_h=ROW_H on_scroll=..
          └ Row i          <View direction="row" w={total_w} shrink={0} h={ROW_H}>
              └ Cell × 26  <Cell at value selected editing .. on_click on_shift_click on_drag_over on_double_click/>
```

### 4.1 State: what lives where

Three pieces of state, three homes. Write this table into the header comment of `lib.rs`; it is the example's thesis.

| State | Home | Why |
|---|---|---|
| The sheet (cells, widths) | `use_undoable` + `use_persisted` at `App` | The document. Undone, saved. |
| Selection, the editor (which cell, its draft), the clipboard, a column edge being dragged | `use_reducer(ui_reduce)` at `App` — `UiState` | Must survive the cell leaving the viewport. A `<VirtualList>` row that scrolls away is unmounted and its hooks are swept, so a draft kept in the cell would vanish on scroll. |
| "Focus and select-all on my first frame" (`CellEditor`), "the value I showed last frame, to flash on change" (`Cell`) | `use_state` in the cell | State that is *supposed* to reset when the cell is remounted. Losing it on scroll is the right behaviour. |

```rust
pub struct UiState {
    pub anchor: CellRef,           // where a Shift-extend or a drag started
    pub cursor: CellRef,           // the active cell; selection = Range::new(anchor, cursor)
    pub editor: Option<Editor>,
    pub clipboard: Option<Clip>,
    pub dragging_col: Option<(u16, f32)>,   // column and its live width during a header drag
}
pub struct Editor { pub at: CellRef, pub draft: String, pub select_all: bool }
pub struct Clip { pub range: Range, pub cells: Vec<(CellRef, String)> }   // raw text, at their original refs

pub enum UiMsg {
    Select(CellRef),                 // click: anchor = cursor = at. Commits an open editor first
    Extend(CellRef),                 // shift-click / drag-over: cursor = at, anchor stays
    Move { dcol: i32, drow: i32, extend: bool },   // arrows. Clamped to the sheet. Commits first
    Edit { at: CellRef, draft: String, select_all: bool },
    Draft(String),                   // every keystroke in the cell editor or the formula bar
    Commit { then: Option<(i32, i32)> },           // Enter / Tab: Set + move. Escape: Cancel
    Cancel,
    Copy, Paste, Clear,              // resolved against the selection
    DragCol(Option<(u16, f32)>),     // live width while dragging; None on release
}
```

`ui_reduce` needs to send sheet messages (`Commit` -> `Msg::Set`, `Paste` -> `Msg::SetMany`, `Clear` -> `Msg::Clear`), so the reducer closure captures the sheet's `Actions` (a `Dispatch`, `Clone + Send + 'static`). (Implemented as `ui_reduce` *returning* `Option<Msg>` and `App`'s closure sending it, so the reducer is a plain function and its table is a unit test; `Copy` carries the snapshot the reducer cannot take. Section 8.10.) A reducer that sends to another reducer is fine: `Dispatch::send` only queues and asks for a repaint, and the sheet reducer applies it at its next visit, which is the next frame — the same one-frame delay every handler write has (ARCHITECTURE 5.7). `Paste` needs the sheet's text to build `SetMany`? No: `Copy` snapshots the raw text into `Clip` at copy time, so `Paste` is `shift` over the clip and needs nothing from the sheet.

Both `Dispatch`es go down by context (`Actions` for the sheet, `Dispatch<UiMsg>` for the UI). Rows and cells only `send`; they never see a `State` guard. That is what lets the row `render` closure be written without borrowing anything mutable from `App`, and it is the same shape as board: what a component did goes up as an event, the row turns it into a message.

### 4.2 The grid

**The body** is one `<VirtualList rows={ROWS} row_h={ROW_H} row_w={total_w} horizontal on_scroll={..} grow={1.0} h={0.0}>`. `h={0.0}` with `grow` is the "take the rest, not everything" idiom from board 8.3 / patch 8.3 — but that is a *column* idiom, and the row this list sits in gives it `h="100%"` instead (section 8.11). `total_w` is the sum of `col_w`, and it goes to the list rather than to the row: the rows are drawn into the rects the list reserved, so the list is the only thing that can tell the scroll area how far sideways the sheet reaches (section 8). Each row is then `<View direction="row" w="100%" h={ROW_H}>` — `100%` of `row_w`, so the row reaches past the edge without a `w` or a `shrink={0}` of its own.

**`on_scroll` on `<VirtualList>`** is the one addition to `egui-reactor-elements`. `ScrollAreaOutput::state.offset` is the offset egui used this frame; the element emits it after `show_rows` returns:

```rust
#[event] on_scroll: egui::Vec2,
..
let out = egui::ScrollArea::new([horizontal, true]).show_rows(ui, row_h, rows, ..);
on_scroll.emit(out.state.offset);
```

`App` keeps the offset in `use_state<egui::Vec2>` and writes it **only when it differs** (a write every frame would ask for a repaint every frame, ARCHITECTURE 5.6). The headers, drawn earlier in the same frame, use the offset from the previous frame; a scroll is one frame behind on the headers, which is the standard one-frame delay and invisible at 60 fps. Add the event to the elements table in ARCHITECTURE section 6.

**The column header** is not a `<Canvas>`: `Canvas` hands `paint` one `Ui` and one rect with one `sense`, and a drag on a column edge needs an `interact` per edge. So it is a `cx.leaf_fill` with a hand-written body (the escape-hatch example's third way down). It paints `A`..`Z` at `x = rect.left() - offset.x + column start`, clipped to its rect, the selected columns tinted, and for each column edge allocates a 6pt wide `ui.interact(.., Sense::drag())` zone. On `drag_started` / `dragged`, `UiMsg::DragCol(Some((col, w)))` with `w = max(MIN_COL_W, start_w + drag_delta.x total)`; on `drag_stopped`, `Msg::SetColWidth` then `DragCol(None)`. While `dragging_col` is `Some`, the width vector the rows are given has that column overridden, so the body follows the drag live and the sheet is written once.

Clicking a column header selects the column? Out of scope: the header is only for width. Say so in a comment.

**The row header** is the same shape: a leaf that paints `1`..`10000` from `offset.y`, the selected rows tinted. `ROW_HDR_W` wide, no interaction.

**A cell** is a `#[component]`:

```rust
#[component]
fn Cell(
    cx: &mut Cx,
    at: CellRef,
    w: f32,
    text: &str,               // the shown text: formatted value, or "" — computed by the row
    is_error: bool,
    is_number: bool,          // right-aligned
    selected: bool,           // inside the selection range
    active: bool,             // the cursor cell (thicker border)
    editor: Option<&str>,     // Some(draft) when this cell is being edited
    select_all: bool,         // (implemented as `editing: bool` + `draft: &str`, and `w` as `width`: section 8.6)
    #[event] on_press: (),          // primary press without Shift
    #[event] on_shift_press: (),
    #[event] on_drag_over: (),      // the pointer is over this cell with the primary button held
    #[event] on_double: (),
    #[event] on_draft: String,      // from the editor inside
    #[event] on_lost_focus: (),     // (implemented as `on_finish: Finish`, an enum of Down / Right / Cancel / Blur: section 8.7)
)
```

The cell is one `cx.leaf` of exactly `w` by `ROW_H`: `allocate_exact_size(.., Sense::click_and_drag())` (implemented as `Sense::CLICK | Sense::DRAG`: the named constructor is also `FOCUSABLE`, and a focusable cell takes the keyboard focus and silences the grid — section 8.7), paint the fill (selection tint, flash tint), the one-pixel right and bottom border, the text (`Monospace`? no, proportional; numbers right, text left, errors in the theme's error colour), then if `editor.is_some()` draw a `CellEditor` on top instead of the text. Name it for the accessibility tree: `response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, at.name()))` (implemented with `current_text_value` set to the shown value as well — a painted cell that says only its name tells a screen reader, and a test, nothing about what is in it; section 8.9), so tests can `get_by_label("B2")` and the gallery's a11y test finds nothing unnamed. Only the cells in view exist in the tree (a few hundred), which is fine.

`on_drag_over` is how a drag-select works without `use_dnd`: the cell reports "pointer over me while the primary button is down" (`response.hovered() && ui.input(|i| i.pointer.primary_down())` and the drag did not start on a column edge), the row sends `Extend(at)`. Nothing is picked up and nothing is dropped, so board's hook is the wrong tool; note that in the comment where the difference from patch's wiring is one line.

**The flash**: `let mut last = use_state(cx, || (text.to_owned(), 0.0f64));` — when `text` differs from `last.0`, set `last = (text, now)`. While `now - last.1 < FLASH_SECS`, tint the cell and `request_repaint`. Edit `B2` and every cell downstream of it lights up for half a second: that is the derivation chain, seen. Because the state is cell-local, a cell that scrolls into view starts from "no flash", which is exactly right.

**`CellEditor`** (implemented inline inside `Cell`'s leaf rather than as its own component — a `#[component]` cannot be drawn inside a leaf's `Ui`; section 8.5) is board's `TitleEdit` with the payload pattern of patch's `SourceEdit`: draw an `egui::TextEdit::singleline` on a local copy of the draft, `on_draft.emit(text)` when `changed()`, focus and select-all (or caret at the end when `select_all` is false, which is the type-to-edit case) on its first frame, name the AccessKit node `"edit B2"`. Enter / Tab / Escape are **not** handled here — the `App`-level key handler sees them (4.3) and egui's `TextEdit` does not consume Enter on a single-line field. `lost_focus` is reported up, and `App` turns it into `Commit` unless the focus went to the formula bar (the two share the draft, so moving between them is not a commit).

Number formatting: integers as integers, others with up to 4 decimals and trailing zeros trimmed, `#…` labels for errors, text as-is. One free function `display(&Value) -> (String, is_number, is_error)`.

### 4.3 Keys

One place, at the top of `App`, every frame (implemented as `focused.is_none()` alone — the three keys the editor takes are read where the editor is drawn, section 8.7):

```rust
let focused = cx.ctx().memory(|m| m.focused());
let grid_keys = focused.is_none() || focused == editor_field_id;
```

- No widget focused: arrows (`Move`, with `extend = shift`), Enter / F2 (`Edit` keeping the text, `select_all` true), Delete / Backspace (`Clear`), Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z (`Undoable::Undo` / `Redo`), Ctrl+C / Ctrl+V (`Copy` / `Paste`), Tab (`Move` right), and any `Event::Text(s)` that is not a control character starts an edit with `draft = s`, `select_all = false`.
- The cell editor focused: Enter → `Commit { then: Some((0, 1)) }`, Tab → `Commit { then: Some((1, 0)) }`, Escape → `Cancel`. Arrows are the caret's (egui's), the way every spreadsheet does it while typing. (Implemented in the editor's own `Response` rather than here: `Memory::begin_pass` reads Tab and Escape out of the raw events before any application code runs, so only the focused widget's event filter can reach them. Section 8.7.)
- The formula bar or the name box focused: nothing; they are ordinary widgets. The name box's `on_submit` parses a reference and sends `Select` (or a range `A1:C3` → `Select` + `Extend`); the formula bar's changes are `Draft`, and if no editor is open when it gains focus, `Edit { at: cursor, draft: current text, select_all: false }`.

Use `ui.input_mut(|i| i.consume_key(..))` for the keys the grid takes, so a scroll area below does not also react to arrows. Read `Event::Text` without consuming (nothing else is focused). The gallery's own search box has focus when the user types in it, so `grid_keys` is false then and nothing leaks.

Scrolling the cursor into view: `<VirtualList>` has no `scroll_to`, and this plan does not add one. Arrow keys move the cursor; the viewport does not follow it. The name box jumps the cursor, not the view. Say so in a comment at the key handler and list it in section 8 as the second thing `<VirtualList>` would need for an application (the first is `on_scroll`). Do not add it in this PR: one addition to elements is the budget.

### 4.4 Derived values

```rust
// `epoch` is patch's: a `use_state<u64>` in App, `+= 1` in the undo and redo handlers,
// read once into a local before `rsx!` (patch lib.rs, "A revision counter does: ..").
let compiled = use_memo(cx, (sheet.structure_rev, epoch), || { counters.with(|c| c.compiled.set(c.compiled.get() + 1)); eval::compile(&sheet) });
let values   = use_memo(cx, (sheet.value_rev, sheet.structure_rev, epoch), || { counters.with(..evaluated + 1..); eval::evaluate(&sheet, compiled) });
```

`epoch` is patch's argument verbatim: revs roll back on undo, so `(rev, epoch)` is what makes deps monotonic. Copy patch's comment above `let mut epoch = use_state(cx, || 0u64);` rather than paraphrasing it; it is the explanation.

`Counters` is a `use_handle` of `Cell<u32>`s written through `Handle::with` inside the memo closures (a dirtying write would ask for a repaint from inside a memo, and the count is only for the status bar). The status bar shows `26 × 10000 · 412 cells · compiled 3 · evaluated 7`. **Those two numbers are how S-4 is tested** and how the reader sees the two stages come apart. Also show, on the cursor cell's line: `B7 = SUM(B2:B6) → 1234` and its deps, from `compiled.deps`; on an error, the label.

The row `render` closure captures `values`, `compiled` (both `&'s`, so they coexist with anything), the `UiState` snapshot (a `Copy` of `cursor` / `anchor` / editor cell / draft clone), the width vector, and the two `Dispatch`es. Nothing mutable. This is the sentence for the header comment: *the row closure borrows nothing it could not borrow twice*.

## 5. Reuse

- `board::hooks::{Undoable, use_undoable}` via `board = { path = "../board" }`, exactly as patch does. Zero changes on the board side.
- Not used: `use_dnd` (nothing is picked up and dropped; drag-select is hover-with-button-down, column resize is `drag_delta`), `use_identity` (the only per-cell state is the kind that should reset on remount), `use_future` / `<Suspense>` (nothing is loaded).
- Patch's `SourceEdit` shape for the editor, board's `TitleEdit` shape for first-frame focus, both copied rather than shared: a 20-line leaf is not worth a crate.

## 6. Tests (`tests/spreadsheet.rs`)

Same harness as `patch/tests/patch.rs` (`build_ui_state(run_app, Store::new())`, `settle`, `SIZE` wide enough for `ROW_HDR_W + 8 columns` and about 20 rows: `egui::vec2(760.0, 520.0)`). The starter sheet is loaded on the first frame (`Load` is one message; it arrives on frame two). A helper `type_into(harness, "B2", "=A1+1")` clicks the cell, sends `Event::Text` per character, presses Enter.

- **S-1** Typing `12` into `A1` shows `12` in `A1`; typing `hello` shows `hello`; `A1` is empty after Delete.
- **S-2** `A1 = 2`, `B1 = 3`, `C1 = =A1*B1` shows `6`; changing `A1` to `4` shows `12` in `C1`.
- **S-3** `A1 = =B1`, `B1 = =A1` show `#CYCLE!` in both; `C1 = =1+1` still shows `2`; fixing `B1` clears both.
- **S-4 (the highlight)** Read `compiled N · evaluated M` from the status bar. Type a number into a literal cell: `compiled` unchanged, `evaluated` +1. Type a formula: both +1. Change the formula's text: both +1. Type a different number into the same literal cell: `compiled` unchanged again.
- **S-5** Keys: Down from `A1` puts the cursor on `A2` (name box reads `A2`); Shift+Right makes `A2:B2`; Enter opens the editor on `A2`; typing then Enter commits and the cursor is on `A3`; Tab from the editor commits and moves right; Escape leaves the cell unchanged; F2 keeps the text; typing a character replaces it.
- **S-6** Copy `C1` (`=A1*B1`) and paste on `C2`: `C2` reads `=A2*B2` (formula bar) and shows the product of row 2. Paste a 2×2 range; one undo removes all four.
- **S-7 (the other highlight)** Scroll the body down by 40 rows with wheel events (copy `scroll` from `crates/egui-reactor-elements/tests/virtual_list.rs`: a `TouchPhase::Start` wheel event turns off smoothing so one call moves exactly `points`; point the pointer inside the body first). Click `A41`, type `abc` (no Enter): `get_by_label("edit A41")` exists. Scroll back to the top: `query_by_label("A41")` and `query_by_label("edit A41")` are both `None` (the row is unmounted). Scroll down again: the editor is open on `A41` with `abc` in it (read the formula bar, or the editor node's value). Enter commits, and `A41` shows `abc`. Row 41 rather than 5000 so the wheel distance stays small; the property is the same.
- **S-8** Undo after two edits restores one, then the other; redo replays; a new edit after undo drops the redo branch (`can_redo` false, via the redo button's `enabled`).
- **S-9** After S-7's scrolling, `harness.state().collisions()` is empty (the row scope is the row index and the cell scope is the column; `cx.scope` is written by the list, but this is the first example with a thousand hooks in view).
- **S-10** Dragging column `A`'s edge 40pt right makes `A` wider by 40 (compare `get_by_label("A2")` rect widths before and after) and `B2` moves right by 40; one undo puts it back.
- **S-11** `on_scroll` is tested where it lives: one more test in `crates/egui-reactor-elements/tests/virtual_list.rs` builds a `<VirtualList on_scroll={..}>` that writes the offset into a `use_state`, scrolls with the file's own `scroll` helper, and asserts the reported `y` equals the points scrolled (and `x` after a horizontal wheel event on a `horizontal` list). The painted header labels are not in the accessibility tree and are not asserted from the app test.
- Unit tests in `sheet.rs`: the rev table in section 1, one line per row; `Set` with the same text bumps nothing; `Set("")` removes the key.
- Unit tests in `formula.rs` / `eval.rs`: section 2 and 3 lists.

The a11y test in the gallery: cells and the editor are named, the formula bar and the name box are `TextInput`s without a name (`<TextEdit>` has no name prop) — add `"spreadsheet: TextInput"` twice to `KNOWN_UNNAMED`, in tree order (name box first, then formula bar). The header and row-header leaves have no sense and no nodes.

## 7. gallery / README / ARCHITECTURE

- `examples/gallery/Cargo.toml`: `spreadsheet = { path = "../spreadsheet" }`. `EXAMPLES`: after `patch`. `Running`: `"spreadsheet" => { <SpreadsheetApp/> }` (no plain). `tests/bench.rs` name list: add it. `tests/a11y.rs`: the two `TextInput`s.
- `Meta`: hooks `use_state`, `use_reducer`, `use_persisted`, `use_memo`, `use_handle`, `provide_context`, `use_context`, `#[hook]`; elements `View`, `Text`, `Button`, `TextEdit`, `VirtualList`, `Frame`, `Separator`.
- README table, after `patch`: `spreadsheet` · "Formulas over 26 × 10,000 cells: two memo stages, and a draft that survives scrolling out of view." · live · lib.rs · –.
- ARCHITECTURE section 6, elements table, `VirtualList`: add `on_scroll` after `render`. One sentence in the prose under it: the offset egui used this frame, after the rows are drawn.
- `examples/spreadsheet/Trunk.toml` and `index.html`: copy board's.
- No snapshot.

## 8. Differences found during implementation

Neither core (`egui-reactor` / `egui-reactor-macros`) nor the gallery's own code was touched. `egui-reactor-elements` gained exactly what section 4.2 budgeted for and one repair next to it (8.1). Below is "what could not be written / how we worked around it / what we would add". Where this overlaps with board or patch section 8, the overlap itself is the information, so it is stated explicitly.

### 8.1 `<VirtualList horizontal>` never scrolled sideways, and could not be made to from outside

The element took the `horizontal` flag and passed it to `ScrollArea::new([horizontal, true])`, and its doc said a row meant to reach past the edge should carry its own `w` and `shrink={0}` — but no row ever gave the list a scroll range. Measured from inside the element, a list 284pt wide whose rows were laid out 900pt wide reported `content_size = [284, ..]`: the viewport, every time. The cause is one line at each end of the layout engine. A `<VirtualList>` row is drawn with `Reserve::Fixed(row_size)`, and both paths finish with `ui.allocate_space(size)` — `engine/lite.rs` and `engine/mod.rs` — so a row occupies exactly the rect the list reserved for it, and nothing a row draws past that rect is ever allocated in the scroll area's `Ui`. The rows are laid out correctly and painted in the right places; they are simply not *measurable* from outside. The prop had no test and no user in the repo, which is how it stayed broken.

The fix is on the elements side, because core is out of scope here: `<VirtualList>` gained `row_w: Option<f32>`, "every row is this wide". It replaces the viewport width as the root the row's tree is laid out into — never narrower than the viewport, so a list given less than its window still fills it — and it is handed to the scroll area as `ui.set_min_width(row_w)`, the smallest thing that widens `min_rect`; with it, `content_size` follows `row_w` and the offset moves. The width the sheet reaches is *told* to the list rather than measured from the rows, which is a fair contract for a virtual list: it is already told `rows` and `row_h` for the same reason.

The homework core owes is the other half — allocate `size.x.max(content_width)` at those two `allocate_space` sites, keeping `size.y` fixed, because the pitch guarantee is entirely about the vertical cursor, with a `lite_parity` corpus case for a row that overflows sideways. After that `row_w` would go back to being guidance, one fewer measure pass rather than the only way to scroll sideways.

### 8.2 `<VirtualList>` still cannot be told where to scroll

`on_scroll` reports where the list *is*; nothing says where it should go. So the cursor can leave the viewport: arrow keys move it, the name box jumps it, and neither brings the view along. The example says so in a comment at the key handler and does not work around it, because every workaround is worse — a hidden `scroll_to_me` widget inside the row would fight `show_rows` for the offset it just decided.

What we would add is `scroll_to: Option<usize>` (and, with 8.1's `row_w`, an `x` alongside it), passed to `ScrollArea::vertical_scroll_offset` when it changes. That is the second thing `<VirtualList>` needs before it is an application element rather than a benchmark; `on_scroll` was the first. One addition to elements was this PR's budget, and `on_scroll` is what the frozen headers could not be written without, so it won.

### 8.3 The non-dirtying write, for the third time: `Handle::with_mut` is overdue

The status bar counts how often each memo stage has run, and the counting happens *inside* the memo closures. Every write on `Handle` (`set`, `update`) asks for a repaint, and asking for a repaint from inside a memo is an app that never idles (ARCHITECTURE 5.6). So `Counters` is a `use_handle` of two `std::cell::Cell<u32>`s written through `Handle::with`, which hands out a `&T` and dirties nothing — exactly board 8.2's workaround and exactly patch 8.2's.

Three examples, three interior-mutability wrappers for the same missing method. `Handle::with_mut(|&mut T|)` — "give me the value, do not mark it dirty" — would delete the `Cell`s here, the `RefCell<HashMap>` in patch's `Ports` and the one in board's `Dnd`. The counterpart already exists for state (`State::bind()`, 5.6); this is the same idea one type over.

### 8.4 An element with `bind` still cannot report its value (patch 8.4), and this example needed two

`<TextEdit>`'s `bind` holds the only `&mut String`, so a handler on the same element cannot read that string as well (ARCHITECTURE 3.7's second constraint), and `on_change` carries no payload. Both text fields here have to report every keystroke, because both are editing a draft that lives in a reducer above them:

- the **formula bar** (`FormulaBar`), 34 lines, one `cx.leaf` around `egui::TextEdit::singleline` on a per-frame copy, emitting on `changed()`;
- the **cell editor**, inside `Cell`'s leaf, for the same reason plus 8.5.

That is two more hand-written leaves on top of patch's `SourceEdit`, so the count of "elements that had to be unwrapped because `on_change` has no payload" is now three across two examples. A `String` payload on `<TextEdit on_change>` — one clone on the element side, which already owns the `&mut` — would remove all three.

The name box is the counter-example and shows where the line is: it only has to report on Enter, `on_submit` already carries the text, and it is an ordinary `<TextEdit>`.

### 8.5 A `#[component]` cannot be drawn inside a leaf, so the cell editor is not one

The plan has `CellEditor` as its own component. It cannot be: the field is drawn *on top of the cell's rectangle*, inside the `cx.leaf` closure that owns that rect, and a leaf's closure gets an `&mut egui::Ui`, not a `Cx`. Building a nested `Cx` there is possible (`escape-hatch`'s fourth hatch, which is what `patch`'s canvas does for its nodes) but it would cost a `Cx`, a scope and a tree per *edited cell* to save nothing: there is only ever one.

So the field is written inline in `Cell`, with board's `TitleEdit` first-frame focus and select-all and patch's `SourceEdit` payload emit. The one thing that had to change with it is "is this the editor's first frame": `TitleEdit` gets it from being freshly mounted, and the cell is not — it was already on screen before the edit began. It is a `use_state<bool>` that flips when `editing` starts, written only when it differs so it does not dirty anything per frame.

The plan's `#[event] on_lost_focus: ()` became `#[event] on_finish: Finish`, an enum of `Down` / `Right` / `Cancel` / `Blur`, because the cell is now the only thing that can tell those apart (8.7).

### 8.6 Two props that `rsx!` will not deliver: `Option`, and anything named like a layout attribute

`Cell` was planned with `editor: Option<&str>`, `Some(draft)` while the cell is being edited. `#[component]` gives every `Option<T>` prop `#[builder(setter(strip_option))]`, so it is written `editor="x"` and not `editor={Some("x")}` — which is pleasant everywhere else and makes it impossible to pass *conditionally* from `rsx!`. It became two props, `editing: bool` and `draft: &str`. Every `Option` prop in elements (`hint`, `desired_width`, `rows`) has the same shape; none of them has ever needed to be conditional, which is why this had not come up.

The second is sharper and cost a confusing compile error. `rsx!` packs every layout shorthand into the `style` prop, and `w` is one of them, so `Cell`'s planned `w: f32` never reached the component — the macro reported `no method named 'style' found`. Renamed to `width`, with a comment. What we would add is a better error: the macro knows the component's prop names, so a prop whose name is a layout attribute could be a compile error at the definition ("`w` is a layout attribute; a prop cannot be called that") rather than a type error at every call site.

### 8.7 egui decides some keys before the application sees them

Four surprises, all in `Memory::begin_pass` and `Sense`, and together they are most of the reason the key handling looks the way it does.

**Focusable senses silence the grid.** `Sense::click_and_drag()` is `CLICK | DRAG | FOCUSABLE`, so a cell would take the keyboard focus the moment it was clicked, and the app-level handler — which only runs when nothing is focused — would never run again. The cells are `Sense::CLICK | Sense::DRAG`; egui's own docs point at the bare constants for exactly this. The column-edge drag handles are `Sense::DRAG` for the same reason plus a11y (8.9).

**Tab and Escape are read before any application code runs.** `Memory::begin_pass` walks the raw events, turns Tab into a focus direction and Escape into "clear the focus", and only the *event filter of the widget that currently has focus* can stop it. So `consume_key` at the top of `App` is too late for both, and the plan's "Enter / Tab / Escape are handled by the App-level key handler" could not be written. Where they land instead:

- **Escape and Enter** are read from the cell editor's own `Response`: both surrender the focus (Escape in `begin_pass`, Enter through `TextEdit`'s `return_key`), so both arrive as `lost_focus()` plus `key_pressed`, which is board's `TitleEdit` pattern with one extra key.
- **Tab inside the editor** works because the field is `lock_focus(true)`: that puts Tab in the field's event filter, `begin_pass` skips it, and a *single-line* `TextEdit` has no Tab handler of its own (egui's tab arm is gated on `multiline`), so it neither moves the focus nor inserts a character and the app can take it.
- **Tab in the grid** has no widget to set a filter, so egui hands the focus to the first widget that wants it. There is no API to undo that, so `read_grid_keys` reports that it took a Tab and `App` surrenders the focus at the top of the next frame. Six lines, and it is the one piece of this example that reads like a workaround.

**`consume_key` matches "at least these modifiers", not "exactly these".** `Modifiers::matches_logically` only rejects a modifier the pattern *requires* and the event lacks, so `consume_key(NONE, ArrowRight)` swallows Shift+ArrowRight, `consume_key(NONE, Tab)` swallows Shift+Tab, and `consume_key(COMMAND, Z)` swallows Ctrl+Shift+Z. Written the obvious way, Shift-extend never worked and redo did an undo. Every pair is now tried most-specific-first with a comment naming the rule. This is egui's documented behaviour and not a bug, but it is a sharp edge that the S-5 and S-6 tests found on their first run and nothing else would have.

What we would add: nothing to the library. This is the price of driving a grid from the keyboard next to a widget toolkit that owns the focus, and it belongs in the example where a reader can see it.

### 8.8 `Dispatch::send` cannot be called from inside `ui.input_mut`

`send` ends with `ctx.request_repaint()`, and `input_mut` holds the context's lock for the length of its closure, so sending a message while reading the keyboard deadlocks — a real ten-second `RwLock` panic, not a theoretical one. `read_grid_keys` collects into two `Vec`s and sends after the closure returns.

Worth a line in ARCHITECTURE if it comes up again: anything that touches `egui::Context` is off limits inside `input`, `input_mut`, `memory` and `memory_mut`, and `Dispatch::send`, `State`'s guard drop and `Handle::set` all touch it.

### 8.9 Two things a painted grid has to say out loud

A cell is a painted rectangle: the value is drawn with `Painter::text`, so there is no widget and nothing in the accessibility tree unless the cell puts it there. `WidgetInfo::labeled` gives it its name (`B2`), and `current_text_value` gives it the value it is showing. Without the second, a screen reader is told there is a cell called `B2` and never what is in it — and `tests/spreadsheet.rs` could not read a single result. Both are one `widget_info` call; this is patch 8.5's lesson (painted things do not exist unless you name them) with a value added to it.

The other half is the column-edge drag handles. `ui.interact(.., Sense::drag())` is focusable, so six unnamed `Unknown` nodes appeared in the gallery's a11y test, and how many depends on how wide the header happens to be — an exact-match test that changes with the window is worse than no test. They are `Sense::DRAG` now: nothing drawn changed, they are still draggable, and they are simply not keyboard-reachable. The honest fix is `Role::Splitter` with a name, which needs a `WidgetType::ResizeHandle` path through `widget_info` on a bare `interact` response; that is a story for `docs/tasks/a11y/`, not for this PR.

### 8.10 Two reducers, and what has to cross between them

The screen has a reducer of its own (`ui_reduce` over `UiState`) next to the document's (`sheet::reduce` over `Sheet`), because the selection and the draft must survive a row being unmounted and the sheet must not carry them into the undo history. Two things fell out of putting them side by side.

**The UI reducer returns the sheet message rather than sending it.** The plan has the closure capture the sheet's `Dispatch` and send from inside. Returning `Option<Msg>` and letting `App` send it, one line up, makes the whole reducer a plain function of its arguments — so "what does Enter in a cell actually do" is a unit test at the bottom of `lib.rs` instead of a kittest scenario. Seventeen of the file's tests exist because of that shape. No `UiMsg` ever implies two sheet messages, so `Option` is enough.

**A snapshot has to be taken where the data is.** `UiMsg::Copy` carries its `Clip`: the UI reducer has no sheet to read, and the key handler does, so the handler builds it with the free function `clip(sheet, range)`. `Paste` then needs nothing from the sheet at all — it is `formula::shift` over the clip — which is what the plan predicted, from the other end.

### 8.11 `h="100%"` and `h={0.0}` are not interchangeable, and which one depends on the parent

board 8.3 and patch 8.3 both end with `grow={1.0} h={0.0}` as "take the rest, not everything". That is right for a filling leaf inside a **column**, where `grow` is the vertical axis. Inside a **row** it is wrong twice over: `grow` distributes the horizontal space and `h={0.0}` really does mean a height of zero. Written the way the plan quoted it, the body drew four rows in a 640pt window. The outer column keeps `grow={1.0} h={0.0}`; the row header and the list inside it are `h="100%"`.

Nothing to add to the library — the rule is already in ARCHITECTURE 6 — but the idiom is quoted often enough that it is worth saying that it is a *column* idiom.

### 8.12 The pure modules: three decisions the plan left open

- **`HashMap<CellRef, String>` has no JSON spelling.** A struct key is not a string, and `use_persisted` saves JSON, so `Sheet::cells` has a small `serde(with = ..)` adapter that writes the map as `"B2" -> text`. A saved sheet is then readable by a human, and a key that cannot be parsed back is dropped with a `log::warn!` rather than failing the whole load: one broken entry should not cost the user the rest of the sheet.
- **Kahn, not a three-colour DFS.** The plan asks for a DFS to find cycles and produce an order. Kahn's algorithm gives the same order, is iterative (a chain ten thousand cells long cannot overflow the stack), and what it leaves undrained when it runs out of work is *exactly* "on or downstream of a cycle" — which is the set `evaluate` has to fill with `#CYCLE!`. The ready set is a `BTreeSet`, so the order does not depend on `HashMap` iteration order and the tests can assert it.
- **`COLS` / `ROWS` / `DEFAULT_COL_W` live in `sheet.rs`, not `lib.rs`.** Every reference is bounded by the first two (`CellRef::parse` refuses `AA1`) and `Sheet::empty` builds the width vector from the third, so a sheet has to be buildable without a screen. `lib.rs` re-exports all three next to `ROW_H`, `ROW_HDR_W`, `MIN_COL_W` and `FLASH_SECS`, which are its own.

### 8.13 What virtualization costs at this size

About six hundred cells are on screen at once in the gallery (26 columns by roughly 23 rows), each with two `use_state` hooks, plus one `use_context` per row: on the order of 1,300 hook slots visited per frame, against patch's dozens. Two things worth recording:

- `Store::collisions()` is empty after a long scroll (test S-9). The row scope is written by `<VirtualList>` and the cell scope by hand as the column index, and this is the first example where a mistake there would be invisible rather than obvious.
- The app idles. `Harness::run` settles, which means nothing in six hundred cells asks for a repaint on a quiet frame — the flash is the only per-frame repaint request and it is bounded by `FLASH_SECS`. That is the property 8.3 is protecting, at six hundred times the usual scale.

The per-pane frame time is in `examples/gallery/tests/bench.rs`, which now lists `spreadsheet` alongside `patch`; run it with `cargo test --release -p gallery --test bench -- --ignored --nocapture`. It was not run for this PR — the number is a measurement, not a claim the tests make.
