# spreadsheet: progress and handoff

PR: https://github.com/fand/egui-react/pull/21, branch
`claude/egui-react-demo-ideas-10bnna`, base `main`. Task in `task.md`, plan
in `plan.md`; what could not be written as planned, and what the library
should grow, in `plan.md` section 8 (8.1 to 8.13). The plan was carried out
in five steps by a subagent, one step per commit, each reviewed and
committed before the next started.

## What was done, in order

| Step | Commit | Change | Result |
|---|---|---|---|
| Docs | `d2d4a07` | `task.md` / `plan.md`: goal, scope, data model, formula language, the two-memo derivation, where each piece of state lives, tests S-1 to S-11 | The plan the rest was built against |
| 1 | `9b34615` | `sheet.rs` (sparse `Sheet`, `CellRef`, `Msg`, `reduce`, two revision counters), `formula.rs` (tokenizer, parser, `Expr` / `Value` / `Error`, `shift`), `eval.rs` (`compile` with Kahn's ordering and cycle detection, `evaluate`), `preset.rs` (an 88-cell budget) | 52 unit tests. No egui in any of the four |
| 2 | `9b90572` | `<VirtualList>` gains `on_scroll` (the offset egui used this frame) and `row_w` (the width every row is laid out into and the width the list scrolls over); ARCHITECTURE section 6 | Found and fixed: `horizontal` never scrolled sideways with the documented row shape (8.1). 9 element tests, 2 new |
| 3 | `e017d9e` | `lib.rs`: `App`, a second reducer for the UI state, two memos with counters, painted column and row headers driven by the offset, `Cell` as one leaf with the editor inside it, the key handler | 68 unit tests, headless smoke of S-4 and S-7 |
| 4 | `2c633b6` | `tests/spreadsheet.rs`: S-1 to S-10 | 10 scenarios green. Found and fixed: `consume_key` matches at-least modifiers, so Shift+Arrow and Ctrl+Shift+Z were swallowed; type-to-edit keeps a burst of characters |
| 5 | `c61ec14` | Gallery, README, a11y list, bench list; `plan.md` section 8 | Column-edge handles made non-focusable (they were nameless tabbable nodes, and how many depended on the window width). Full workspace CI green |

## Results

`task.md` "Done when", item by item:

| Criterion | Status |
|---|---|
| S-4: a literal edit re-runs evaluation, not the parse | Met (`a_literal_edit_evaluates_without_recompiling`, read from the status bar's `compiled K · evaluated M`) |
| S-7: an edit in progress survives scrolling out of view and back | Met (`an_edit_survives_scrolling_out_of_view_and_back`, row 41, the row is unmounted in between) |
| Formula / evaluation unit tests | Met: precedence, ranges, every error, diamond, cycles, downstream of a cycle, `shift` in every direction and off the edge |
| fmt / clippy / test / wasm check | Met on the whole workspace, 342 tests, 0 warnings |
| Gallery a11y test, no new unnamed nodes beyond the listed ones | Met: two `TextInput`s (name box, formula bar), same gap as every other `<TextEdit>` |
| `cargo run -p spreadsheet` | Builds. Not launched: no window in the environment |
| `trunk serve` checked by eye | Not done: no browser. `cargo check --target wasm32-unknown-unknown` passes for the gallery with the example in it |
| `#spreadsheet` in the gallery | Registered; the gallery tests that enumerate examples by name pass |
| README, ARCHITECTURE, `lib.rs` header match the implementation | Met |

Sizes: `lib.rs` 1,763 lines, the three pure modules 2,174, tests 586. Test
counts: 68 unit (51 pure, 17 for `ui_reduce` and formatting), 10 scenarios,
9 element tests for `<VirtualList>`.

## How it is built now

- **Two reducers.** The sheet (`use_undoable` over `sheet::reduce`,
  `use_persisted("spreadsheet/sheet")`) and the UI (`use_reducer(ui_reduce)`:
  selection, the editor and its draft, the clipboard, a column drag).
  `ui_reduce` is a plain function returning `Option<Msg>`; `App` sends what
  comes back to the sheet. `Select` and `Move` commit an open editor first.
- **Two memos.** `compile` on `(structure_rev, epoch)`, `evaluate` on
  `(value_rev, structure_rev, epoch)`; `epoch` is patch's undo counter, with
  patch's comment. Counters on a `use_handle` written through `Handle::with`
  feed the status bar.
- **The grid** is `<VirtualList rows=10000 row_h=22 row_w=total horizontal
  on_scroll>`; a row is `<View direction="row" w="100%">` of 26 `<Cell>`s. A
  cell is one leaf: `Sense::CLICK | Sense::DRAG` (not focusable, or it would
  take the grid's keys), named `B2` with its shown value on the AccessKit
  node, and the editor drawn inside the same leaf when it is the editing
  cell. The row closure captures the two memo results, `Copy` snapshots, and
  the two `Dispatch`es from context; no `State` guard.
- **Headers** are `leaf_fill` painters that draw from the offset `on_scroll`
  reported last frame; the column header owns a 6pt `Sense::DRAG` zone per
  edge, sends `DragCol` live and `SetColWidth` once on release.
- **Keys** are read at the top of `App` when no widget has focus (arrows,
  Enter / F2, Delete, Ctrl+Z / Y / C / V, Tab, `Event::Text` to start an
  edit), most-specific modifier set first. Enter / Tab / Escape inside the
  editor are read from the editor's own `Response` with `lock_focus(true)`,
  because `Memory::begin_pass` takes Tab and Escape before the app runs.
  Grid Tab needs a one-frame focus repair (8.7).
- **Reuse.** `board::hooks::{Undoable, use_undoable}`; nothing else. No
  `use_dnd` (drag-select is hover-with-button-down), no `use_identity` (the
  only cell-local state is the kind that should reset on remount), no async.

## Behaviour changes to know about

- `<VirtualList horizontal>` without `row_w` still does not scroll sideways;
  it never did. With `row_w` it does, and a row's `<View w="100%">` is then
  already the full width (no `w` / `shrink={0}` on the row).
- `<VirtualList on_scroll>` fires every frame. A caller that keeps it in
  state must write only when it differs, or the app never idles.
- The `VirtualList` event enum is exported from the elements prelude.
- Cells and the column-edge handles are not keyboard-focusable; the name box
  and the formula bar are. Nothing else in the gallery changed.
- The starter budget is loaded once, when the persisted sheet has no cells.
  A saved sheet is JSON under `"spreadsheet/sheet"`, cells keyed by name
  (`"B2"`); an unreadable key is dropped with a warning, not a failed load.

## What remains

- Look at it in a window and in a browser: header alignment during scroll,
  the flash, the selection tint, the resize cursor. None of these have a
  headless assertion, by design.
- The cursor can leave the viewport (8.2): arrows and the name box move it,
  the view does not follow. Needs `scroll_to` on `<VirtualList>`.
- Core homework from 8.1: allocate `size.x.max(content_width)` at the two
  `allocate_space` sites in `engine/lite.rs` and `engine/mod.rs`, with a
  `lite_parity` case; then `row_w` becomes guidance rather than the only way.
- `Handle::with_mut`, third time asked (8.3). `<TextEdit on_change>` with a
  payload (8.4), which would remove both hand-written text leaves. A
  `Role::Splitter` path for bare `interact` handles (8.9).
- The gallery bench number for `spreadsheet` was not measured (8.13):
  `cargo test --release -p gallery --test bench -- --ignored --nocapture`.
- Incremental recalculation, whole-column references, absolute references,
  row / column insertion: out of scope in `task.md`, unchanged.

## Reproduction

From the repository root:

```sh
cargo test -p spreadsheet                          # 68 unit + 10 scenarios
cargo test -p egui-react-elements --test virtual_list
cargo test -p gallery                              # a11y, gallery, bench (ignored)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace --target wasm32-unknown-unknown
cargo run -p spreadsheet
cargo run -p gallery spreadsheet
trunk serve --config examples/spreadsheet/Trunk.toml
```

The full workspace test build needs about 40 GB of scratch with default
debuginfo; the step-5 run hit a full disk at link time and was repeated
with `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`, which peaked at 39%
of a 252 GB disk.
