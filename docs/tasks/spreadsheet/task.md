# Task: spreadsheet

## Goal

Add a demo with the feel of a real application: a spreadsheet with formulas. Cells reference other cells, a change recalculates everything downstream, and the grid is 26 columns by 10,000 rows drawn through `<VirtualList>`.

[board](../board/task.md) proved each property one at a time and [patch](../patch/task.md) showed them on a large screen. What neither shows, and this example is for:

- **A derivation chain over a 2D grid.** Cell text -> parsed formulas + dependency graph + evaluation order -> values. Two `use_memo` stages with two revision counters, in the same shape as patch's `topology_rev` / `param_rev`: typing a number into a literal cell re-runs evaluation only; changing a formula re-runs the parse and the ordering as well.
- **Which state belongs to the item and which to the whole, under virtualization.** A `<VirtualList>` row that scrolls out of view is unmounted and its hooks are swept. So the draft being typed into a cell is *not* cell-local state (the way board's card draft is): a draft that vanished because the user scrolled would be a bug. The editor lives above the list. What stays in the cell is the state that is fine to lose: the "focus me on first frame" flag and the recalculation flash. The example draws that line explicitly and a test pins it.
- **`<VirtualList>` in an application**, not a benchmark: frozen column and row headers that follow the body's scroll, per-column widths, a row that is wider than the viewport.
- **Keyboard-driven UI.** Arrow keys, Shift-extend, Enter / Tab / Escape / F2 / Delete, type-to-edit, Ctrl+Z / Y, Ctrl+C / V. No example has any of this today.

The formula language is deliberately small. The point is the UI over it, not the language.

Do not touch core (`egui-reactor`, `egui-reactor-macros`). One addition to `egui-reactor-elements` is planned ([plan.md](plan.md) section 4.2): `<VirtualList>` reports its scroll offset. Anything else that turns out to be needed goes into plan.md section 8 and a separate PR.

## Scope

### In scope

- `examples/spreadsheet` (lib + bin, no plain egui version).
- Grid: columns `A`..`Z`, rows `1`..`10000`, sparse storage. Column header and row header stay put while the body scrolls both ways. Column widths can be changed by dragging the header edge, and are persisted.
- Cell text: `=` starts a formula; text that parses as a number is a number; anything else is text.
- Formulas: numbers, strings in double quotes, cell references (`B2`), ranges inside function calls (`A1:A10`), `+ - * / ^`, unary minus, parentheses, comparison operators (`= <> < <= > >=`, giving 1 or 0), functions `SUM` `AVG` `MIN` `MAX` `COUNT` `IF`. Errors: `#PARSE!` `#REF!` `#CYCLE!` `#DIV/0!` `#NAME?` `#VALUE!`.
- Selection: one cell or a rectangular range (anchor + cursor). Click, Shift+click, drag, arrows, Shift+arrows.
- Editing: type to start (replacing the content), Enter or F2 to start (keeping it), Enter commits and moves down, Tab commits and moves right, Escape cancels, clicking another cell commits. A formula bar edits the same draft; a name box shows the selection and jumps to a typed reference.
- Delete / Backspace clears the selection. Ctrl+C / Ctrl+V copy and paste the selection with relative references shifted. Ctrl+Z / Ctrl+Y undo and redo (`use_undoable`, one step per user action).
- The sheet is held by `use_reducer` (inside `use_undoable`) and `use_persisted`. A starter sheet (a small budget with `SUM` / `AVG` / `IF`) is loaded when nothing is saved.
- A status bar shows the cell count and how many times each memo stage has run, which is how the tests (and the reader) see that a literal edit does not re-parse.
- Registration in the gallery, kittest, README table, ARCHITECTURE section 6 (the `<VirtualList>` addition).

### Out of scope

- Plain egui version. At this size a twin is not worth writing; board carries that comparison.
- Incremental recalculation (only the dirty cells). The whole sheet is recalculated when a revision changes; at the sizes here that is well under a frame.
- Multiple sheets, named ranges, whole-column references (`A:A`), absolute references (`$A$1`), string functions, date / time, number formats beyond the default.
- Column / row insertion and deletion, hiding, freezing beyond the two headers, merged cells, cell styling, charts.
- OS clipboard. Copy / paste is internal to the app (egui's clipboard is not available everywhere on the web).
- File import / export, server sync, async of any kind (`patch` and `fetch` cover `use_future` / `<Suspense>`).
- Changes to core.

## Deliverables

- `examples/spreadsheet/` (`src/lib.rs` / `sheet.rs` / `formula.rs` / `eval.rs` / `preset.rs` / `main.rs` / `Cargo.toml` / `Trunk.toml` / `index.html` / `tests/spreadsheet.rs`).
- `<VirtualList on_scroll>` and `<VirtualList row_w>` in `crates/egui-reactor-elements`, and their line in ARCHITECTURE section 6. `row_w` is the repair `on_scroll` uncovered: `horizontal` never produced a scroll range without it (plan.md 8.1).
- Registration in `examples/gallery` (`Cargo.toml`, `EXAMPLES`, the `Running` `match`, the a11y / bench lists).
- One row in the README table.
- plan.md section 8 (differences found during implementation).

## Done when

- kittest is green. The highlights (plan.md section 6):
  - S-4: editing a literal cell re-runs evaluation but not the parse; editing a formula re-runs both.
  - S-7: an edit in progress far down the sheet survives scrolling away and back.
- Formula / evaluation unit tests are green (precedence, ranges, every error, diamond dependencies, cycles, reference shifting).
- `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` / `cargo check --workspace --target wasm32-unknown-unknown` are green. The gallery a11y test is green with no new unnamed nodes beyond the ones the plan lists.
- `cargo run -p spreadsheet` works, and `trunk serve` works in the browser (checked by eye).
- `spreadsheet` appears in the gallery and `#spreadsheet` links to it.
- README, ARCHITECTURE and the header comment of `lib.rs` match the implementation.

## Decisions (assumptions at the start)

- No `Panel` / `CentralPanel`; the app fills the area it is given, so the gallery can run it in a column.
- The `use_persisted` key is `"spreadsheet/sheet"`. Time comes from `ctx.input(|i| i.time)`, never `std::time::Instant`.
- `sheet.rs` / `formula.rs` / `eval.rs` know nothing about egui, like board's `board.rs` and patch's `graph.rs`.
- Undo is board's `use_undoable`, depended on the same way patch does (`board = { path = "../board" }`). `use_dnd` and `use_identity` are not needed here; if either turns out to be, say why in plan.md section 8.
- Keyboard input reaches the grid only while no egui widget has focus. The formula bar, the name box and the cell editor are ordinary `TextEdit`s and keep egui's own key handling while focused.
- The look stays plain: a one-pixel grid, a tinted selection, bold headers, an error in red. Nothing that would need a snapshot.
