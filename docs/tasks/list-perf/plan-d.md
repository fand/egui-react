# Plan D: own layout layer over taffy (replace egui_taffy)

Follows [plan.md](plan.md) steps A to C and the decision material in
[measurements.md](measurements.md) ("After step C", "Idle gap attribution").
The task definition and the 1.5x criterion are in [task.md](task.md).

## 0. Why, in three lines

- After A to C every scenario runs at one pass, but VirtualList still costs
  about 2x Plain. Taffy's algorithm is 0% of that; egui_taffy's per-tree
  bookkeeping is 9%.
- About 80% is egui_taffy creating one egui `Ui` per taffy node and a second
  one per leaf: 9 `Ui`s per row against 4 in Plain. That is its design, not a
  bug, and its backgrounds, interactive containers and sticky scrolling rely on
  it. egui-reactor uses none of those.
- egui_taffy is called only from `crates/egui-reactor/src/cx.rs`. `layout.rs`
  uses taffy types only. So the replacement is one module plus the `Surface`
  half of `Cx`.

Target: VirtualList at most 1.5x Plain on all four scenarios (task.md), with
the existing tests and snapshots unchanged.

## 1. Shape of the new layer

New module `crates/egui-reactor/src/engine.rs` (private; `Cx` is the only user).
Dependency `taffy` directly, same version and features egui_taffy 0.14 uses
(taffy 0.9: `block_layout`, `content_size`, `flexbox`, `grid`). The
`egui_reactor::taffy` re-export stays, pointing at the direct dependency.

### 1.1 Data

```rust
struct Tree {
    taffy: taffy::TaffyTree<Measure>,   // Measure = min/max/intrinsic like egui_taffy's Context
    nodes: HashMap<egui::Id, NodeSlot>, // NodeSlot { id: NodeId, keep: bool }
    root: Option<NodeId>,
    last_size: egui::Vec2,
    created_this_frame: bool,
    last_visited: u64,                  // pass number, for the sweep
}
```

Trees live in `Store` as `RefCell<HashMap<egui::Id, Rc<RefCell<Tree>>>>`,
not in egui memory:

- one map lookup per tree per frame, no `Arc<Mutex>`, no `data_mut` lock;
- `Store::end_pass` sweeps trees not visited this pass, so a tree left behind
  by an unmounted subtree is dropped (the egui-memory growth step A worked
  around goes away at the root);
- nested trees (a row tree inside the root tree's `leaf_fill`) each hold their
  own `Rc`, so the outer tree's borrow does not block the inner one.

### 1.2 One frame of one tree

`engine::show(store, ui, id, style, reserve, f)`, called from
`Cx::container_reserving` in `Surface::Ui` mode:

1. Root rect = `ui.available_rect_before_wrap()`. `reserve` mirrors
   egui_taffy: width only (`set_min_width`, definite width, height
   max-content) for nested containers, both axes for the runner root.
2. **Layout-first (step C):** if the tree has a root, is not dirty, and
   `last_size != root_rect.size()`, compute now so children draw at their new
   places in this pass.
3. Run `f` with a `Cx` in tree mode. Children register nodes (1.3).
4. Sweep nodes whose `keep` is false (remove from taffy). Set
   `created`/`removed` flags.
5. If dirty or the size changed: snapshot every node's `Layout`, compute,
   compare with the step B rule (`content_size` only on the root and on
   `overflow: scroll` nodes). Request a discard only if a node was created,
   removed, or moved. Nodes created this frame drew in an invisible sizing
   pass (1.4), so `created` always discards.
6. `ui.allocate_space(root content size)` so the surrounding egui `Ui` knows
   how much the tree took, as egui_taffy's `TuiInitializer::show` does.

The measure closure is a copy of egui_taffy's (`min_size` / `max_size` /
`infinite` handling, NaN guards, `ceil`) so that existing layouts come out the
same. Keep it byte-for-byte in behaviour first; improvements are follow-ups.

### 1.3 Nodes: a rect, not a `Ui`

`Cx` tree mode carries `{ tree: &Rc<RefCell<Tree>>, parent: NodeId, child_index,
root_ui: &mut Ui, prefix: egui::Id }`.

- **Node key** = `prefix.with(child_index)` for anonymous nodes, or the id a
  `<View>` passes. `prefix` is `Cx::layout_id()` (the slot id inside a
  VirtualList). Same reuse rule as egui_taffy: existing key reuses the node,
  style set only if it changed, child order checked, tail removed on mismatch.
- **Containers (`Cx::container` in tree mode)** add a node and recurse. No
  `Ui`, no widget registration, no response. egui-reactor draws no background
  on a `<View>`, so nothing is lost.
- **Leaves (`Cx::leaf`)** create exactly one child `Ui`:
  `root_ui.new_child(UiBuilder::new().max_rect(rect).id_salt(scope_id.with(child_index)))`.
  The salt comes from the **hook scope**, not the node key, so widget ids stay
  per row while node keys stay per slot. This also retires the per-row
  `push_id` in VirtualList (0.013 to 0.028 ms per frame).
  After `f` runs, the measure is `ui.min_rect().size().ceil()`; set the node's
  context only if it changed (which marks it dirty).
- **`leaf_fill`** is the same with no measure (`infinite` both axes, zero
  min), so the node is sized by style and parent only.
- **`cx.ui()` in tree mode** returns the tree's root `Ui` (what egui_taffy's
  `egui_ui_mut` returned). `Panel` / `CentralPanel` rely on this to carve the
  window; unchanged.
- **`Cx::scope` in tree mode** only changes `prefix` and the hook scope; no
  `Ui` is pushed.

### 1.4 First frame of a node

egui_taffy draws a new leaf in a zero-sized invisible `sizing_pass` `Ui`, then
discards. Mirror that in D1. D2 adds the text fast path, after which a tree of
`<View>` and `<Text>` settles in one pass from the start.

### 1.5 Text fast path (D2)

`Cx::text(style, WidgetText, wrap)` used by `<Text>`:

- Measure inside the taffy measure closure: `ctx.fonts(|f| f.layout_job(job))`
  with the wrap width taken from the available space when `wrap` is set,
  infinite otherwise. Cache the galley on the node context (keyed by the text
  hash and wrap width) so the paint step reuses it.
- Paint: `root_ui.painter().galley(rect.min, galley, color)`. No child `Ui`.
- Accessibility and hover: register a widget rect with
  `ctx.create_widget(WidgetRect { id, rect, sense: Sense::hover(), .. }, false)`
  and set `WidgetInfo::labeled(Label, ..)` through `response.widget_info`, the
  same two calls `egui::Label` makes. That keeps `egui_kittest` label queries,
  screen readers, and hover tooltips working. It skips only the `Ui` and
  `Label`'s own layout work. (Open question 1 in section 5 offers cheaper
  variants.)

Other widgets (`Button`, `TextEdit`, `Checkbox`, ...) keep the one-`Ui` leaf
path. They need a `Ui` anyway.

## 2. What changes where

| File | Change |
|---|---|
| `crates/egui-reactor/Cargo.toml`, workspace `Cargo.toml` | drop `egui_taffy`, add `taffy`; remove `[patch.crates-io]` |
| `crates/egui-reactor/src/engine.rs` | new: `Tree`, frame protocol, measure, B + C rules, sweep |
| `crates/egui-reactor/src/store.rs` | tree map, `tree(id)` accessor, sweep in `end_pass` |
| `crates/egui-reactor/src/cx.rs` | `Surface::Taffy(&mut Tui)` becomes `Surface::Tree(TreeCx)`; `leaf`, `leaf_fill`, `container`, `scope`, `ui`, `ctx`, `with_layout_id` reimplemented; new `text` |
| `crates/egui-reactor/src/lib.rs` | `pub use taffy;` from the direct dependency |
| `crates/egui-reactor-elements/src/view.rs` | `<Text>` uses `cx.text` (D2) |
| `crates/egui-reactor-elements/src/virtual_list.rs` | drop the per-row `push_id` once leaf salts come from the scope (D1) |
| `crates/egui-reactor/tests/multi_pass.rs` | rewrite: it drives egui_taffy directly today |
| `crates/egui-reactor/tests/engine_*.rs` | port the fork's `tests/discard.rs` and `tests/layout_first.rs` |
| `docs/ARCHITECTURE.md` 5.3, 6, 7, 11 | passes, engine, crate deps, decision log entry |
| `README.md` | the egui_taffy sentence |

Elements other than `View`/`Text`/`VirtualList` should need no change: they
use `cx.leaf`, `cx.leaf_fill`, `cx.ui()`, `cx.in_taffy()`, which keep their
signatures. `in_taffy` keeps its name.

## 3. Steps and gates

Each step is one commit and one benchmark row ("After D1" etc.), same
protocol as plan.md section 1, plus `cargo test -p gallery --features snapshot`.

| Step | Content | Gate |
|---|---|---|
| D1 | `engine.rs`, `Cx` switched, `Store` trees, `multi_pass` rewritten, fork tests ported, egui_taffy and `[patch]` removed. `<Text>` still draws `egui::Label` in a one-`Ui` leaf. VirtualList `push_id` removed. | All tests pass. Gallery snapshots byte-identical (13 that pass today). Benchmark: passes/frame unchanged (1.00 / 1.00 / 1.00 / 1.02); Idle Virtual below 0.20 ms (expect about 0.16 to 0.17: 9 `Ui`s per row become 3). |
| D2 | Text fast path. | Snapshots byte-identical. `egui_kittest` label queries pass. Idle Virtual at or below 1.5x Plain. New-tree first frame with only `<View>`/`<Text>` settles in one pass (test). |
| D3 | Docs: ARCHITECTURE 5.3 / 6 / 7 / 11, README, task.md completion table, progress.md. Final "After D" table with the Virtual/Plain ratio per scenario. | Ratio at or below 1.5x on all four scenarios, or a written explanation of the remainder with a profile. |

If D1's snapshot gate fails on a layout difference: stop, diff the two
layouts (egui_taffy fork vs engine) on that example, and fix the engine to
match. Do not update snapshots to make D1 pass. D2 may legitimately change
text placement by sub-pixel rounding; if it does, list every changed snapshot
with its cause before updating.

## 4. Risks and how each is checked

- **Measurement semantics drift** (rounding, `infinite` handling, first-frame
  sizing pass): snapshots plus `tests/layout.rs`, `taffy_cx.rs`,
  `scroll_fill.rs`, `resize.rs`, `containers.rs`. The measure closure is
  copied, not rewritten.
- **`leaf_fill` max-content quirk** (ARCHITECTURE 6: it reports the root
  rect height, so a `ScrollArea` under a toolbar overflows): keep the quirk in
  D1 for parity. Fixing it is a follow-up with its own snapshot changes.
- **Accessibility of `<Text>`**: covered by the widget rect + `WidgetInfo`
  in 1.5; `a11y` tests in the gallery (`KNOWN_UNNAMED`) must not change.
- **Panels and `cx.ui()`**: `examples/shell` snapshot.
- **Suspense offscreen path**: draws into an invisible `Ui` with a fixed rect;
  engine must accept a `Ui` that is not the window. `tests/suspense.rs`.
- **wasm**: taffy is pure Rust; `cargo check --target wasm32-unknown-unknown`.
- **Fork obsolete**: `../egui_taffy` stays only as the source of the upstream
  PRs for B and C (open question 3).

## 5. Open questions (answers go here, then the plan is final)

1. **`<Text>` in D2**: (a) paint the galley and register a widget rect plus
   `WidgetInfo` as `Label` does (keeps a11y, hover, kittest; recommended);
   (b) keep `egui::Label` in a one-`Ui` leaf (no fast path; D2 is skipped);
   (c) paint only, no widget rect (fastest; `<Text>` disappears from the
   accessibility tree and from kittest label queries).
2. **Snapshot parity**: (a) byte-identical is a hard gate for D1 and D2;
   deviations need the user's approval one by one (recommended); (b) small
   diffs allowed if explained.
3. **Upstream PRs for B and C**: file them from the fork now, in parallel
   with D; file later; or not at all.
4. **Branch**: continue on `docs-perf` (PR #7 already carries A to C), or a
   new branch `layout-engine` off `docs-perf` with its own PR.

Answered 2026-09-06: 1(a) galley + widget rect; 2(a) byte-identical, stop
on any diff; 3 later, fork stays; 4 continue on `docs-perf` (PR #7).

## 6. Out of scope

- One tree for the whole VirtualList window, style caching, equality-aware
  `Handle::set`: revisit after D3 with a fresh profile.
- Backgrounds, borders, sticky headers on `<View>`: not features today; the
  engine leaves room (a node is a rect, painting a frame around it is a leaf
  concern) but does not add them.
- Web / 120 Hz measurement: still separate.
