# Plan: perf (VirtualList passes, then egui_taffy)

The task definition is in [task.md](task.md). Current numbers are in
[measurements.md](measurements.md). This plan covers three steps, each
followed by a measurement, and one decision at the end.

## 0. Order and gates

| Step | Where | What | Gate to continue |
|---|---|---|---|
| A | egui-reactor | Key VirtualList row trees by slot, not row index | Scroll passes/frame drop from 2.00; no new node per scrolled row |
| B | egui_taffy fork | Skip `request_discard` when layout output is unchanged | Scroll and Filter reach 1 pass/frame |
| C | egui_taffy fork | Compute layout before drawing when only the root size changed | Resize drops from 2.98 passes/frame |
| D | decision | Keep egui_taffy (upstream PR) or replace it with an own thin layer over taffy | See section 5 |

Do A first and on its own: it needs no fork and its result tells how much B
and C can still gain. B and C each get their own commit and their own
measurement row so the effect of each is visible.

Product constraint (unchanged): `<Row>` and `<View>` stay declarative. Direct
egui row layout is a reference only.

## 1. Measurement protocol (used after every step)

Run from the repository root, release only:

```sh
PERF_CSV="$PWD/docs/tasks/list-perf/samples-<step>.csv" \
  cargo test --release -p list-10k --test scenarios -- --ignored --nocapture
cargo test --release -p list-10k --test list_10k
cargo test -p egui-reactor --test multi_pass
```

- Add one row per scenario to a new "After <step>" table in measurements.md:
  mean / p50 / p95 ms, passes/frame, discard frames, final rows. Same columns
  as the existing table so the rows compare directly.
- Keep the baseline CSV. Name new ones `samples-a.csv`, `samples-b.csv`,
  `samples-c.csv`.
- Record the commit hash and the egui_taffy source (crates.io 0.14.0 or fork
  commit) with each table.
- Before A, add one thing to the scenario benchmark: count taffy node
  creations per frame. Cheapest way is a `log::trace!` in the fork later; for
  step A, count `first_frame` discards indirectly by logging the discard
  reason and the row range. The gate for A is "Scroll discard frames fall"
  and "Idle unchanged"; no node counter is needed for that.

The benchmark is CPU only. Web and GPU stay out of scope until D.

## 2. Step A: slot-keyed row trees in VirtualList

### 2.1 Why

Each `<VirtualList>` row runs under `cx.scope(i)`. `<View>` inside the row
opens a new egui_taffy tree with id `scope_id()`, which contains the row index.
egui_taffy stores one `TaffyState` per tree id in egui memory
(`egui_taffy/src/lib.rs:195`) and creates a node for every unseen id
(`add_child_node`, line 274: `first_frame = true`, drawn invisible in a sizing
pass). So every row that scrolls in is a new tree, a new set of nodes, one
invisible pass, and one discard. Old trees stay in egui memory forever.

### 2.2 Change

Split the hook scope from the layout scope in `Cx`:

```rust
pub struct Cx<'s, 'u> {
    pub store: &'s Store,
    surface: Surface<'u>,
    scope: egui::Id,   // hook ids (unchanged)
    layout: egui::Id,  // egui_taffy tree id and node auto-id prefix (new)
}
```

- `Cx::new` / `new_taffy`: `layout = scope`.
- `Cx::scope(source, f)`: deepen both, `push_id(source)` as today. In the
  taffy branch use `with_auto_id_prefix(layout)` instead of `scope`.
- `Cx::hook_scope` / `scope_sharing_ui`: deepen `scope` only (as today).
- New `Cx::with_layout_id(id, f)`: run `f` with `layout = id`, hooks and the
  egui id stack untouched. `pub` but documented as an internal for list
  elements.
- `Cx::layout_id()` getter. `<View>` passes `cx.layout_id()` to
  `cx.container` instead of `cx.scope_id()`.

VirtualList:

```rust
for (slot, i) in range.enumerate() {
    cx.scope(i, |cx| {
        cx.with_layout_id(scope.with(("vl-slot", slot)), |cx| render(cx, i))
    });
}
```

Row `i` keeps its hook state under `scope.with(i)`. The egui `Ui` for the row
is still under `push_id(i)`, and egui_taffy salts each leaf `Ui` from that
parent (line 390), so widget ids (button hover, TextEdit focus) stay per row.
Only the taffy tree id and the taffy node ids become positional.

### 2.3 What A does and does not fix

- Fixes: node creation per scrolled row, the invisible sizing pass, unbounded
  `TaffyState` growth in egui memory.
- Does not fix: a row whose text width changed updates the node context
  (`set_node_context`, line 591), taffy marks it dirty, egui_taffy recomputes
  and still calls `request_discard`. Expect Scroll to go from 3 passes on
  60/120 frames to 2 passes on those frames, not to 1. That is what B is for.
- The visible range grows by one row on some scroll frames. That last slot is
  a new tree once, then reused. Acceptable.

### 2.4 Tests

- `crates/egui-reactor-elements` test: scroll a VirtualList with a
  `use_state` counter per row; click row 5, scroll away and back, the count
  is still on row 5. This pins "hooks keyed by row, not by slot".
- Same test asserts the number of `TaffyState` entries in egui memory does
  not grow with scroll distance (read `ctx.data(|d| d.len())` before and
  after a long scroll; it must be equal).
- Existing `multi_pass` and `list_10k` tests pass unchanged.
- Snapshots: expect no pixel change. If a snapshot changes, stop and explain
  in this document before updating it.

## 3. Step B: egui_taffy fork, skip discard when layout is unchanged

### 3.1 Setup

- Fork egui_taffy 0.14.0 into a sibling checkout (not a workspace member).
- Add to the workspace `Cargo.toml` on this branch only:

```toml
[patch.crates-io]
egui_taffy = { path = "../egui_taffy" }
```

- Do not commit the patch to main. If the branch is merged before upstream
  accepts, the patch line is removed and the measurement table says so.

### 3.2 Change (`recalculate`, `lib.rs:632`)

```text
before compute:
    if any node was created this frame (first_frame) -> must_discard = true
    old = every node's Layout (iterate id_to_node_id, copy taffy.layout())
compute as today
after compute:
    changed = any node's Layout != old
    if must_discard || changed { request_discard("Taffy recalculation") }
```

- `Layout` holds floats; compare with exact equality. A recompute that yields
  the same floats is exactly the case to skip; rounding is not the problem here.
- Found during implementation: `content_size` on a leaf is just what the leaf
  measured, so it changes whenever a label gets wider and would defeat the
  check. Compare every other field on every node; compare `content_size` only
  on the root and on `overflow: scroll` nodes, the two places egui_taffy reads
  it (`lib.rs:137`, `lib.rs:466`).
- A node removed this frame also forces a discard: it was drawn at its old
  place before the sweep dropped it.
- `first_frame` nodes drew invisible, so they always need the second pass.
  Track a `created_this_frame: bool` on `TaffyState`, set in `add_child_node`,
  cleared in `recalculate`.
- Keep `last_size` update as today.

### 3.3 Expected effect

With A in place, a scrolled-in row reuses nodes, its text widths change, the
tree recomputes, and the layout is the same (fixed `h`, `grow` text absorbs
the width). No discard. Scroll and Filter should reach 1 pass/frame. Idle is
unchanged (already 1 pass). Resize is still 3.

### 3.4 Tests

- Fork unit test: build a row-shaped tree, change a leaf's measured width
  within the slack, recompute, assert no discard was requested.
- Fork unit test: change a leaf so the layout does move, assert a discard.
- egui-reactor `multi_pass` test still sees a second pass for the case it
  covers (a new tree).

## 4. Step C: egui_taffy fork, layout-first on root size change

### 4.1 Why

Order today: draw children with last frame's layout, update measurements,
compute, discard. A nested tree learns its new root size only in the pass
after its parent settled, so each nesting level costs one pass. Root tree +
row trees = 3 passes on Resize.

### 4.2 Change (`Tui::create`, around `lib.rs:185`)

Before calling `f(state)`:

```text
if the state has a root node
   and !taffy.dirty(root)
   and last_size != root_rect.size()
then
    compute_layout_with_measure(root, available_space, same measure closure)
    last_size = root_rect.size()
```

Children added during `f` read `state.layout(node_id)` in `add_child_node`,
so they draw at the new positions. `recalculate` then finds nothing dirty and
the size equal, and does not compute or discard. If a measurement did change,
it is dirty, recomputes, and B decides whether to discard.

- The root node's style is applied inside `f` via `.style(style)`. If the
  root style changed, `set_style` marks it dirty and the normal path runs.
  That is correct, only one pass slower.
- The measure closure is shared code; move it into a method on `TaffyState`
  so both call sites use one copy.

### 4.3 Expected effect

Resize: root tree recomputes before drawing; each row tree gets its new size
in the same pass and recomputes before drawing its children. One pass for
stable trees. Expected 2.98 to about 1.0 to 1.3 passes/frame.

### 4.4 Tests

- Fork unit test: tree from frame N, resize the root in frame N+1, assert
  children were drawn with the new layout in the first pass.
- egui-reactor: a resize snapshot pair (before / after a width change) matches
  the current output pixel for pixel. If the first-pass drawing is now correct,
  the final frame is the same as before.

## 5. Decision D: keep egui_taffy or replace it

Decide after C, with three tables in measurements.md.

### 5.1 Keep egui_taffy if

- B + C bring VirtualList to at most 1.5x Plain on every scenario (task.md
  criterion), and
- upstream accepts the two PRs, or the maintainers are responsive enough that
  the branch can wait for a release.

Then: remove the `[patch]`, bump egui_taffy, delete the fork, update
ARCHITECTURE.md 5.3 with the new pass rules.

### 5.2 Replace egui_taffy if

- B or C gives a large gain but upstream declines, or
- the remaining gap after C is still in egui_taffy's per-tree bookkeeping
  (state lookup in egui memory, per-node `Style` compare, `id_to_node_id`
  retain sweep) and closing it needs changes upstream would not take.

What a replacement is: egui_taffy is used only from `crates/egui-reactor/src/cx.rs`
(`tui`, `.id().style().add()`, `.ui()`, `.ui_manual()`,
`with_auto_id_prefix`, `egui_ui_mut`, `reserve_available_*`) and
`layout.rs` uses the `taffy` types only. So "drop egui_taffy" means
rewriting the `Surface::Taffy` half of `cx.rs` on top of `taffy` directly:

- a tree state per root, keyed by id, stored in the `Store` instead of egui
  memory (one lock per frame instead of one per tree);
- node reuse by id, as egui_taffy does, plus B and C built in;
- leaf measurement as today (draw, report `min_size`), with a later option
  to pre-measure `<Text>` through `ui.fonts()`;
- no egui_taffy containers, backgrounds, or sticky scrolling; egui-reactor does
  not use them.

Estimate: about the size of `cx.rs` today, plus tests. This is a separate
task (`docs/tasks/layout-engine/`), not part of this PR. The decision and
its numbers go into ARCHITECTURE.md section 6, which currently rejects
replacing the engine; the argument there was about egui_flex, not about a
thin own layer over taffy, so it needs a rewrite either way.

### 5.3 Out of scope for this PR either way

- Step D from the earlier proposal list (one tree for the whole visible
  range) and per-node style caching. Both wait for a profile of the idle
  gap, which A to C do not touch.
- Equality-aware `Handle::set`. Separate small PR.
- Browser and 120 Hz measurements.

## 6. Deliverables

- `Cx::with_layout_id`, VirtualList change, tests (A).
- Fork commits for B and C, each as a candidate upstream PR with its own
  unit tests.
- measurements.md: three "After" tables plus the decision in section 5.
- progress.md: step status and the D decision.
- ARCHITECTURE.md 5.3 (passes) and 6 (engine) updated after D.

## 7. Checks before each commit

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --target wasm32-unknown-unknown -p egui-reactor -p egui-reactor-elements
```

Snapshot tests: run with `--features snapshot` in gallery; a diff means stop
and explain here before updating.
