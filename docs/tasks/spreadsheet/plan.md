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

Constants, in `lib.rs`: `COLS = 26`, `ROWS = 10_000`, `ROW_H = 22.0`, `DEFAULT_COL_W = 80.0`, `ROW_HDR_W = 48.0`, `MIN_COL_W = 24.0`, `FLASH_SECS = 0.6`.

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

`ui_reduce` needs to send sheet messages (`Commit` -> `Msg::Set`, `Paste` -> `Msg::SetMany`, `Clear` -> `Msg::Clear`), so the reducer closure captures the sheet's `Actions` (a `Dispatch`, `Clone + Send + 'static`). A reducer that sends to another reducer is fine: `Dispatch::send` only queues and asks for a repaint, and the sheet reducer applies it at its next visit, which is the next frame — the same one-frame delay every handler write has (ARCHITECTURE 5.7). `Paste` needs the sheet's text to build `SetMany`? No: `Copy` snapshots the raw text into `Clip` at copy time, so `Paste` is `shift` over the clip and needs nothing from the sheet.

Both `Dispatch`es go down by context (`Actions` for the sheet, `Dispatch<UiMsg>` for the UI). Rows and cells only `send`; they never see a `State` guard. That is what lets the row `render` closure be written without borrowing anything mutable from `App`, and it is the same shape as board: what a component did goes up as an event, the row turns it into a message.

### 4.2 The grid

**The body** is one `<VirtualList rows={ROWS} row_h={ROW_H} horizontal on_scroll={..} grow={1.0} h={0.0}>`. `h={0.0}` with `grow` is the "take the rest, not everything" idiom from board 8.3 / patch 8.3. Each row is `<View direction="row" w={total_w} shrink={0} h={ROW_H}>` — the width is the sum of `col_w`, which is what makes the list scroll sideways (the doc on `VirtualList` says exactly this: a row meant to reach past the edge gets its own `w` and `shrink={0}`).

**`on_scroll` on `<VirtualList>`** is the one addition to `egui-react-elements`. `ScrollAreaOutput::state.offset` is the offset egui used this frame; the element emits it after `show_rows` returns:

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
    select_all: bool,
    #[event] on_press: (),          // primary press without Shift
    #[event] on_shift_press: (),
    #[event] on_drag_over: (),      // the pointer is over this cell with the primary button held
    #[event] on_double: (),
    #[event] on_draft: String,      // from the editor inside
    #[event] on_lost_focus: (),
)
```

The cell is one `cx.leaf` of exactly `w` by `ROW_H`: `allocate_exact_size(.., Sense::click_and_drag())`, paint the fill (selection tint, flash tint), the one-pixel right and bottom border, the text (`Monospace`? no, proportional; numbers right, text left, errors in the theme's error colour), then if `editor.is_some()` draw a `CellEditor` on top instead of the text. Name it for the accessibility tree: `response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, at.name()))`, so tests can `get_by_label("B2")` and the gallery's a11y test finds nothing unnamed. Only the cells in view exist in the tree (a few hundred), which is fine.

`on_drag_over` is how a drag-select works without `use_dnd`: the cell reports "pointer over me while the primary button is down" (`response.hovered() && ui.input(|i| i.pointer.primary_down())` and the drag did not start on a column edge), the row sends `Extend(at)`. Nothing is picked up and nothing is dropped, so board's hook is the wrong tool; note that in the comment where the difference from patch's wiring is one line.

**The flash**: `let mut last = use_state(cx, || (text.to_owned(), 0.0f64));` — when `text` differs from `last.0`, set `last = (text, now)`. While `now - last.1 < FLASH_SECS`, tint the cell and `request_repaint`. Edit `B2` and every cell downstream of it lights up for half a second: that is the derivation chain, seen. Because the state is cell-local, a cell that scrolls into view starts from "no flash", which is exactly right.

**`CellEditor`** is board's `TitleEdit` with the payload pattern of patch's `SourceEdit`: draw an `egui::TextEdit::singleline` on a local copy of the draft, `on_draft.emit(text)` when `changed()`, focus and select-all (or caret at the end when `select_all` is false, which is the type-to-edit case) on its first frame, name the AccessKit node `"edit B2"`. Enter / Tab / Escape are **not** handled here — the `App`-level key handler sees them (4.3) and egui's `TextEdit` does not consume Enter on a single-line field. `lost_focus` is reported up, and `App` turns it into `Commit` unless the focus went to the formula bar (the two share the draft, so moving between them is not a commit).

Number formatting: integers as integers, others with up to 4 decimals and trailing zeros trimmed, `#…` labels for errors, text as-is. One free function `display(&Value) -> (String, is_number, is_error)`.

### 4.3 Keys

One place, at the top of `App`, every frame:

```rust
let focused = cx.ctx().memory(|m| m.focused());
let grid_keys = focused.is_none() || focused == editor_field_id;
```

- No widget focused: arrows (`Move`, with `extend = shift`), Enter / F2 (`Edit` keeping the text, `select_all` true), Delete / Backspace (`Clear`), Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z (`Undoable::Undo` / `Redo`), Ctrl+C / Ctrl+V (`Copy` / `Paste`), Tab (`Move` right), and any `Event::Text(s)` that is not a control character starts an edit with `draft = s`, `select_all = false`.
- The cell editor focused: Enter → `Commit { then: Some((0, 1)) }`, Tab → `Commit { then: Some((1, 0)) }`, Escape → `Cancel`. Arrows are the caret's (egui's), the way every spreadsheet does it while typing.
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
- **S-7 (the other highlight)** Scroll the body down by 40 rows with wheel events (copy `scroll` from `crates/egui-react-elements/tests/virtual_list.rs`: a `TouchPhase::Start` wheel event turns off smoothing so one call moves exactly `points`; point the pointer inside the body first). Click `A41`, type `abc` (no Enter): `get_by_label("edit A41")` exists. Scroll back to the top: `query_by_label("A41")` and `query_by_label("edit A41")` are both `None` (the row is unmounted). Scroll down again: the editor is open on `A41` with `abc` in it (read the formula bar, or the editor node's value). Enter commits, and `A41` shows `abc`. Row 41 rather than 5000 so the wheel distance stays small; the property is the same.
- **S-8** Undo after two edits restores one, then the other; redo replays; a new edit after undo drops the redo branch (`can_redo` false, via the redo button's `enabled`).
- **S-9** After S-7's scrolling, `harness.state().collisions()` is empty (the row scope is the row index and the cell scope is the column; `cx.scope` is written by the list, but this is the first example with a thousand hooks in view).
- **S-10** Dragging column `A`'s edge 40pt right makes `A` wider by 40 (compare `get_by_label("A2")` rect widths before and after) and `B2` moves right by 40; one undo puts it back.
- **S-11** `on_scroll` is tested where it lives: one more test in `crates/egui-react-elements/tests/virtual_list.rs` builds a `<VirtualList on_scroll={..}>` that writes the offset into a `use_state`, scrolls with the file's own `scroll` helper, and asserts the reported `y` equals the points scrolled (and `x` after a horizontal wheel event on a `horizontal` list). The painted header labels are not in the accessibility tree and are not asserted from the app test.
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

To be written at the end, in the same shape as patch section 8: what could not be written as planned, the workaround, and what the library would grow. Things this plan already expects to land here:

- `<VirtualList>` had no way to tell the outside where it scrolled (4.2), so frozen headers were impossible without `on_scroll`. `scroll_to` is the second thing it lacks (4.3); the cursor can leave the viewport.
- Whether `Handle::with` + `Cell` for the counters is the third time the "non-dirtying write" hole shows up (board 8.2, patch 8.2), which would make `Handle::with_mut` overdue.
- `<TextEdit>`'s `on_change` still carries no payload (patch 8.4); the editor and the formula bar are hand-written leaves for that reason. Count them.
- Anything about `Event::Text` and `consume_key` that egui made awkward (the gallery's search box, the code pane's own key handling).
- How many hooks were in view at once and what the store's per-frame cost looked like at 26 × 24 cells (the bench in `gallery/tests/bench.rs` gives the number).
