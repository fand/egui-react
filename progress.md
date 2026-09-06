# Progress and handoff

## Performance investigation (2026-09-06)

PR: https://github.com/fand/egui-react/pull/7, branch `docs-perf`, base `main`.

### Completed

- Rebased PR #7 onto main and pushed (`5206f9e`). Resolved the
  `plan-overview.md` conflict by preserving both accessibility and performance plans.
- Translated `docs/tasks/perf/task.md` and `plan-overview.md` into English;
  committed and pushed as `b166a7d`.
- Inspected list-10k, VirtualList, Cx, Store, state/hooks, the runner, and the
  installed egui_taffy 0.14.0 source.
- Added a controlled benchmark for idle, scrolling, filter changes, and resizing:
  [scenarios.rs](examples/list-10k/tests/scenarios.rs).
- Saved [measurement details](docs/tasks/perf/measurements.md) and
  [per-frame/per-pass CSV](docs/tasks/perf/samples.csv).
- Updated the perf task to distinguish measurements from earlier hypotheses.

The benchmark and measurement changes are included with this handoff. The example's
`Row` was made public so the integration benchmark can reuse it; its declarative
implementation is unchanged. No production performance optimization has been applied.

### Steps A-C done (2026-09-06)

Plan: [docs/tasks/perf/plan.md](docs/tasks/perf/plan.md). Numbers per step in
[measurements.md](docs/tasks/perf/measurements.md) (After step A / B / C).

| Step | Commit | Result |
|---|---|---|
| A: slot-keyed VirtualList row trees (`Cx::with_layout_id`) | `798f327` | Passes unchanged (row tree root rect height depends on scroll position). No tree per scrolled row; egui memory stops growing with scroll. |
| B: egui_taffy fork skips discard when layout unchanged | `20a22c5` | Scroll 2.00 -> 1.00, Filter 1.60 -> 1.00 passes/frame, 0 discards. |
| C: egui_taffy fork computes layout before drawing on root resize | `c1c417e` | Resize 2.98 -> 1.02 passes/frame. All-rows mode 131 -> 54 ms. |

VirtualList mean ms per frame and passes/frame per step (Plain reference in
the last row of each block, same run):

| Scenario | Baseline | After A | After B | After C |
|---|---:|---:|---:|---:|
| Idle | 0.225 / 1.00 | 0.222 / 1.00 | 0.225 / 1.00 | 0.241 / 1.00 |
| Scroll | 0.452 / 2.00 | 0.466 / 2.00 | 0.298 / 1.00 | 0.303 / 1.00 |
| Filter | 1.528 / 1.60 | 1.263 / 1.60 | 1.158 / 1.00 | 1.222 / 1.00 |
| Resize | 0.662 / 2.98 | 0.653 / 2.98 | 0.649 / 2.98 | 0.311 / 1.02 |
| Plain (Idle / Scroll / Filter / Resize) | 0.124 / 0.153 / 1.286 / 0.153 | 0.114 / 0.149 / 1.021 / 0.147 | 0.113 / 0.146 / 1.014 / 0.149 | 0.123 / 0.146 / 1.065 / 0.153 |

Timings drift a few percent between runs (Plain moves too); passes/frame and
discard counts are the stable signal.

Fork: `../egui_taffy` (sibling checkout, not pushed), branches
`skip-unchanged-discard` (`d618550`, step B) and `layout-first` (`ee07d38`,
step C, on top of B). Wired in through `[patch.crates-io]` in the workspace
`Cargo.toml` while steps B and C were measured. Step D1 removed both the patch
and the egui_taffy dependency, so a fresh clone needs neither.

After C, VirtualList / Plain: Idle 1.96x, Scroll 2.08x, Filter 1.15x, Resize
2.03x. Only Filter meets task.md's 1.5x. The remaining gap is per-frame
overhead at one pass, not extra passes. Profiled (measurements.md, "Idle gap
attribution"): taffy itself costs nothing at idle; ~80% of the gap is
egui_taffy creating one or two egui `Ui`s per taffy node (9 `Ui`s per row vs
4 in Plain). Decision D (keep egui_taffy and upstream B + C, or replace it
with an own layer over taffy) was open at this point; plan section 5 lists the
criteria and the profile favours the own layer. Decided in the next block.

Known pre-existing: `cargo fmt --all -- --check` fails on ~30 untouched files
(import order); the two board gallery snapshots are missing.

### Steps D1-D3 done (2026-09-06)

Decision D was taken: replace egui_taffy with an own layout engine over taffy.
Plan: [docs/tasks/perf/plan-d.md](docs/tasks/perf/plan-d.md). Numbers in
[measurements.md](docs/tasks/perf/measurements.md) ("After D1", "After D2",
"After D2b", "Summary D").

| Step | Commit | Result |
|---|---|---|
| D1: `crates/egui-react/src/engine.rs`, `Cx` on it, trees in the `Store`, egui_taffy and `[patch]` removed | `5a9f04a` | A row costs 3 egui `Ui`s instead of 9. Idle 0.241 -> 0.160 ms, 1.96x -> 1.43x Plain. |
| D2: `<Text>` is a galley on the node, not a `Label` in a `Ui` | `bf8d2f2` | A row costs 2. Idle 0.153 ms, 1.32x. All-rows idle 27 -> 22 ms. |
| D3: docs (ARCHITECTURE 3.1 / 5.3 / 6 / 7 / 11, README, task.md result, this block) | this commit | – |
| D2b: text selection back on the engine's `<Text>` | `dfc5dfc` | Idle 0.152 ms, 1.29x. Selection costs nothing measurable. |
| E: a fixed root rect for `<VirtualList>` rows (`Cx::with_root_size`) | `fcb8697` | Rows now sit at the `row_h` pitch `show_rows` reserved (list-10k: 20, was 18). No timing change: the scrolled-frame recompute was never the root rect (traced). See the next block. |

After D2b, VirtualList / Plain: Idle **1.29x**, Filter **1.07x**, Scroll 1.61x,
Resize 1.58x. Two of the four are through task.md's 1.5x; the other two miss by
about 0.1x. Passes per frame: 1.00 everywhere except Resize at 1.10. Gallery
snapshots byte-identical through all of it. The web and 120-Hz criteria are
still unmeasured.

Text selection follows `interaction.selectable_labels` exactly as
`egui::Label` does, and `<Text selectable={false}>` turns it off. A `<Text>`
drawn for the first time is not selectable for that one frame: it has no place
on screen until the layout is computed, so it keeps D2's deferred paint for
that frame and paints itself through `LabelSelectionState` from the next one.

Follow-ups, neither done (measurements.md, "What remains"): keep a swept tree
for a grace period, which gives Resize back its 1.02 passes; skip laying a row
out again when only a galley changed size inside a node that cannot move, which
is what a scrolled frame really pays.

Fork: `../egui_taffy` is no longer a dependency of anything here. Its two
branches stay as the source of possible upstream PRs — `skip-unchanged-discard`
(`d618550`, step B) and `layout-first` (`ee07d38`, step C). Both fixes are built
into the engine.

### Steps E and E1-E2 done (2026-09-06)

Plan: [docs/tasks/perf/plan-e.md](docs/tasks/perf/plan-e.md). Numbers in
[measurements.md](docs/tasks/perf/measurements.md) ("After E", "After E1",
"Summary D", "What remains").

| Step | Commit | Result |
|---|---|---|
| E: a fixed root rect for `<VirtualList>` rows (`Cx::with_root_size`) | `fcb8697` | Rows sit at the `row_h` pitch `show_rows` reserved (list-10k: 20 apart, was 18). No timing change. |
| Plan E written, then its two open questions answered (rows only; one debug log per slot) | `e0c148f`, `b0f2ba1` | – |
| E1: rows laid out by a solver of our own, `crates/egui-react/src/engine/lite.rs` | `65d0274` | Idle 0.145 -> 0.132 ms, Scroll 0.219 -> 0.184, Resize 0.240 -> 0.203. |
| E2: docs (ARCHITECTURE 3.1 / 6 / 7 / 11, VirtualList doc comment, plan-e, measurements, task.md result, this block) | this commit | – |

After E1, VirtualList / Plain: Idle **1.15x**, Scroll **1.30x**, Filter
**1.04x**, Resize **1.40x**. All four are through task.md's 1.5x for the first
time; two runs of the same build agree to within 0.01 ms on every Virtual
figure. Passes per frame are unchanged at 1.00 everywhere except Resize at
1.10. The parity corpus (18 row trees, `crates/egui-react/tests/lite_parity.rs`)
is rect-equal against taffy on every node, and the gallery snapshots are
byte-identical.

E1's own gate asked for Scroll at or below 1.2x as well, and 1.30x misses it.
Per the plan that bought one profile and nothing else: of the 0.95 µs a
scrolled row costs over a plain egui row, 0.36 µs is solving, 0.23 µs is
building the node vector, and 0.36 µs is the component layer, the app's root
tree and the extra galley work (measurements.md, "Where the scrolled frame's
0.95 µs per row goes").

What falls back to taffy, per slot and for good, with one `log::debug!` naming
the attribute: `display="grid"` or `"block"`, `wrap`, `align_content`, a
`baseline` align or `align_self`, `col_span` / `row_span`, an `auto` margin.
Everything else a `<View>` and `<ItemStyle>` can express is solved by the lite
path, at any depth of nesting.

Follow-ups, none done (measurements.md, "What remains"): skip the solve when
the text that changed cannot move any box (the Scroll cost); make the component
layer cheaper (`Cx::scope`, `rsx!`, component bodies — paid by every app, not
just lists); keep a swept tree for a grace period (the Resize 1.10 passes, open
since D1). The web and 120-Hz criteria in task.md are still unmeasured.

### Step F: tree grace period (2026-09-06)

Real trackpad scrolling still logged egui's PERF WARNING: the visible range
alternates 37/38 rows with fractional offsets, and the store dropped the
38th slot's tree every time it was not drawn. Trees now survive 120 passes
(`TREE_GRACE_PASSES`). Resize back to 1.02 passes/frame. New test
`fractional_scrolling_asks_for_no_second_pass`. See measurements.md
"After F".

### Web measurement (2026-09-06)

Chrome, wasm release, synthetic wheel input: `<VirtualList>` 1.0 ms per drawn
frame vs plain 0.8 ms (1.24x), zero PERF WARNING, all-rows 67 ms. Details in
measurements.md "Web". `list-10k-plain` now builds for the web
(`examples/list-10k/index-plain.html`).

### Product constraint

Preserve the React-like component API and declarative row layout. The user does
not want direct egui row layout adopted as the production solution: that would
undermine the library's main benefit. Direct egui remains a benchmark reference.

### Why list-10k is expensive

- The example defaults to `virtualise=false`. Its `for` loop executes every row,
  including offscreen rows: approximately 40,000 taffy nodes for 10,000 rows.
  The plain example virtualizes from the start, so that initial comparison is
  not between equivalent workloads.
- VirtualList wraps egui's `ScrollArea::show_rows`, removing dependence on total
  row count during steady rendering. Its visible rows still open taffy trees
  through `<View>`, unlike the plain egui implementation.
- egui_taffy recomputes when a tree is dirty or its root size changes, then
  requests a discard. Additional passes rerun surrounding UI as well. Multiple
  requests do not imply one additional pass per request.
- Stable trees can run in one pass. The claim that layout always requires two
  passes is incorrect.
- The example displays `stable_dt`, a frame interval or prediction, not CPU
  rendering duration. A displayed 16.7 ms does not prove an 8.3 ms CPU budget was
  exceeded. Browser/120-Hz performance remains unmeasured.

The original full-App release benchmark measured 109.67 / 0.34 / 0.19 ms for
10,000 rows (all rows / VirtualList / plain). It has unequal controls and row
pitch and uses a different harness, so do not combine those numbers with the
controlled results below.

### Controlled benchmark conditions

Native macOS arm64, Rust 1.95.0, release, egui 0.36.1, egui_taffy 0.14.0.
Each mode/scenario uses 30 idle warmup frames and 120 measured frames. Mode order
rotates each frame. There are 10,000 source rows, an initial 600×800 viewport,
a common toolbar/cache, a 20-point row pitch, and a three-pass limit.

- Idle: no changes after warmup; frames are explicitly driven.
- Scroll: trackpad Start followed by -20-point Move events, with synthetic time
  advancing at 120 Hz. Start avoids mouse-wheel smoothing.
- Filter: cycle through `a`, `al`, `alp`, `alpha`, and empty before drawing.
  Rebuilding the shared row cache is inside the timed region.
- Resize: vary width by `4*(frame % 40)` and height by `2*(frame % 30)` from the
  initial size. The first measured frame has the warmup size.

CPU duration is elapsed wall time around `Context::run_ui` plus final-shape
tessellation, including instrumentation. It excludes GPU, browser scheduling,
AccessKit, and OS event delivery. This isolates the list body using the real
VirtualList and Row; it is not an end-to-end App/hooks/keyboard-input benchmark.
Idle results do not measure wakeup frequency or power consumption.

### Recorded results

| Scenario | All rows mean ms | VirtualList mean ms | Plain mean ms | VirtualList passes/frame | Final Virtual/Plain row count |
|---|---:|---:|---:|---:|---:|
| Idle | 44.135 | 0.225 | 0.124 | 1.00 | 37 |
| Scroll | 45.678 | 0.452 | 0.153 | 2.00 | 37 |
| Filter | 57.106 | 1.528 | 1.286 | 1.60 | 37 |
| Resize | 148.590 | 0.662 | 0.153 | 2.98 | 37–40 |

| Scenario | VirtualList pass distribution | Frames requesting discard /120 | Discard requests |
|---|---|---:|---:|
| Idle | 120 × one pass | 0 | 0 |
| Scroll | 60 × one pass, 60 × three passes | 60 | 4,440 |
| Filter | 48 × one pass, 72 × two passes | 72 | 2,136 |
| Resize | 1 × one pass, 119 × three passes | 119 | 4,750 |

All recorded discard reasons were `Taffy recalculation`
(egui_taffy `src/lib.rs:712`). No discard was refused by the pass limit. Plain
used one pass throughout. See measurements.md for p50/p95 and all-mode statistics.

Row counts mean render callbacks, including overscan, not strictly pixel-visible
rows. Final-pass counts match Virtual and Plain in every measured frame.
Scroll ranges nevertheless differ by one row on 60/120 frames. An extra pass
seeing the updated scroll offset is a plausible explanation, not yet proven.
Other scenarios have matching final index ranges.

The CSV contains 1,440 frames and 2,180 passes. `frame_cpu_ms` repeats the full
frame duration on each pass row: do not sum it across passes. Measurements are
one recorded run, not portable performance thresholds.

### Conclusions and next work (written before steps A-C; items 1-3 are done, see above)

1. Trace the exact invalidation source per tree/node during scrolling and
   resizing: new nodes, style/measurement changes, root sizes, and scroll offsets.
   The aggregate discard reason cannot distinguish these causes.
2. Investigate the one-row scroll-range difference alongside pass timing.
3. Reduce unnecessary invalidation and redraw passes while preserving `<Row>`
   and `<View>`. Candidate upstream work: skip discards when recomputation leaves
   relevant geometry unchanged; premeasure eligible fixed-size/text leaves.
   Validate positions, clipping, wrapping, fonts, DPI, and interaction behavior.
4. Profile the remaining idle overhead before optimizing IDs, allocations,
   per-node bookkeeping, or Store operations. Their individual contribution has
   not been measured. Idle floating-point jitter and a universal wasm slowdown
   factor are not established.
5. Measure the full App and browser separately, including CPU stages, pass counts,
   presentation timing, and accessibility overhead under matched conditions.

Do not simply set `max_passes=1`: unresolved nested layouts can carry into later
frames, potentially causing visual instability or continued repaints.

Other library-wide candidates, pending profiling:

- Equality-aware state updates to avoid repainting after unchanged values;
  mutable State access and Handle updates currently request repaint.
- Small revision-based memo dependencies instead of hashing large collections
  every pass. list-10k already caches generated strings.
- Store sweep and ID-cost improvements for hook-heavy trees. list-10k rows have
  no hooks, so these are lower-priority explanations for this benchmark.

### Reproduction and validation

From the repository root:

```sh
PERF_CSV="$PWD/docs/tasks/perf/samples.csv" \
  cargo test --release -p list-10k --test scenarios -- --ignored --nocapture
cargo test --release -p list-10k --test list_10k
cargo clippy --release -p list-10k --test scenarios -- -D warnings
rustfmt --edition 2024 --check examples/list-10k/tests/scenarios.rs
```

The scenario benchmark, all six existing list tests, targeted clippy, formatting,
and diff checks passed. CSV frame/pass totals and matching final row counts were
also checked. Use an absolute `PERF_CSV` path: Cargo runs tests from the package
directory. Workspace-wide CI, GPU, and web checks were not run for this work.

## Earlier handoff: phase 6.6 (board + patch)

The following preserves the earlier handoff, translated into English. Its status
and environment limitations belong to that work and were not revalidated during
the performance investigation.

PR: https://github.com/fand/egui-react/pull/10, branch
`claude/complex-ui-demo-idea-lmigbs`, base `main`.
Task definitions: `docs/tasks/board/` and `docs/tasks/patch/`.
Section 8 of each plan.md records implementation findings.

### Status (2026-09-06)

| Item | Status |
|---|---|
| board, including plain version, gallery registration, tests | Complete: `6e8b8a8` |
| board v2: title-only cards, done checkbox, pen for inline editing, whole-card dragging, empty-card insertion preview | Complete; changes and notes in `docs/tasks/board-v2/plan.md` section 11 |
| patch, including gallery registration and tests | Complete: `12ba0fb` |
| Merge origin/main (a11y PR #5) | Complete: `cdf4246`; only conflict was gallery's setup closure, resolved by calling shader/patch `gpu::setup` and registering WebA11y |
| fmt, workspace clippy, wasm check | Passed after merge |
| Workspace tests | Passed; board v2 fixed the a11y failures below |
| trunk build, GPU snapshots, manual device inspection | Not done; the earlier container had neither trunk nor a GPU |

### Accessibility fixes (complete in board v2)

The whole-card drag surface (`ui.interact(Sense::drag())`) was named
`card: <title>`; checkboxes `done: <title>`; pen/remove buttons `edit` / `remove`;
inline fields `title` / `column name` / `new card`. The board search TextEdit and
five patch elements were added to `KNOWN_UNNAMED` with explanations. Patch's
seven Labels and one Unknown were named in patch code.

Earlier diagnostic notes:

| Example / role | Element | Resolution or option |
|---|---|---|
| board: TextInput ×1 | Search TextEdit (`lib.rs:291`) | Like showcase/todo, the element cannot provide a name; add `board: TextInput` to KNOWN_UNNAMED |
| board: Label ×12 | Card-title Label with click/drag sense (`lib.rs:502`) | egui puts text in AccessKit value, not label; set label through `accesskit_node_builder` after drawing; Button role is also reasonable for a draggable item |
| patch: Label ×7 | Node-header labels (`lib.rs:725`, also inspector) | Same naming fix |
| patch: MultilineTextInput ×2 | SourceEdit (`lib.rs:1077`) | Name the handwritten leaf `<node> shader source` |
| patch: ComboBox ×2 | MixParams / GrayParams (`lib.rs:974`, `997`) | A label prop also draws text; either name them `mode` / `method` or document them in KNOWN_UNNAMED as with form |
| patch: Unknown ×1 | Draggable patch-canvas leaf_fill (`lib.rs:539`) | Use labeled WidgetInfo with `patch canvas`, or document as unnamed like shader |

`KNOWN_UNNAMED` is compared in tree traversal order. Insert entries according to
EXAMPLES ordering: showcase, board, patch, counter, etc.

### Remaining verification on a GPU machine

```sh
cargo run -p board
cargo run -p board --bin board-plain
cargo run -p patch
cargo run -p gallery board
trunk serve --config examples/patch/Trunk.toml
UPDATE_SNAPSHOTS=1 cargo test -p gallery --features snapshot egui_react
cargo test -p gallery --features snapshot
```

Check patch previews, slider updates, shader recompilation after expression edits,
connection wires, gallery plain switching, and naga-to-pipeline behavior with
WebGL fallback. The two board snapshots (`board_react`, `board_plain`) were not
generated. Patch has no snapshots, like shader.

Pay particular attention to patch wires (reserve `Shape::Noop`, then replace via
`set`) and whether `leaf_fill` height causes board columns or patch side panes to
extend past the window (board/patch plan section 8.3).

### PR description cleanup

The earlier handoff reports a claude.ai session link appended by the UI; remove
it under the repository's CLAUDE.md rule against signatures/session links in
commits and PR bodies. Also correct the claim that both examples include gallery
snapshot tests: patch does not. Neither change was checked during perf work.

### Library follow-ups for separate PRs

1. `use_keyed(cx, key, init)` / `Store::keyed_slot`: nonpersistent state tied to
   identity, independent of position. Board works around this with `use_identity`
   and a rebuilt Cx scope (board section 8.1). `key=` distinguishes siblings;
   moving across parents creates another slot, as in React. Similar intent to
   Compose's `movableContentOf`.
2. `Handle::with_mut` without marking dirty: needed for per-frame registration
   of DnD slots and port positions. Current workaround is `Rc<RefCell<_>>` plus
   Handle::with (board/patch section 8.2).
3. `leaf_fill` reports the root rectangle's full height instead of remaining
   height. Toolbar + growing ScrollArea can overflow; showcase's sidebar and
   gallery's left column share this shape. Provide a runner root with fixed
   window height (board/patch section 8.3).
4. `<View>` does not return its rectangle. Drag/drop/wire anchors currently need
   leaves. Consider an `on_rect` event (board section 8.4, patch section 8.2).
5. Bound elements' `on_change` has no payload. Reporting TextEdit contents
   requires a handwritten leaf (patch section 8.4).
6. Code-size claims do not hold for board: React 435 lines versus plain 460.
   The distinction is who manages `HashMap<CardId, CardUi>` and its `retain`.
   Align README/gallery explanations with that (board section 8.6).

### Design details for the next contributor

- Board: card draft/expanded state uses `hooks::use_identity`, a use_state under
  a card-ID scope. Ordinary `key=` loses state across columns. Props carry data;
  context carries Dispatch, theme, and DnD session. Undo/debounce/DnD are custom
  hooks in `examples/board/src/hooks.rs`; no library changes were needed.
- Patch: Graph separates `topology_rev` and `param_rev`. WGSL generation and naga
  validation memoize on topology; uniforms follow parameters. Pipeline creation
  occurs in `CallbackTrait::prepare` only when the source hash changes. Each node
  uses `ui.scope_builder(max_rect)` → Cx::new → cx.scope(node.id, ..) inside
  leaf_fill, with ordinary `<View>` inside. Patch depends on board to reuse its
  use_dnd/use_undo hooks.
- Earlier environment: approximately 38 GB disk, with debug builds exhausting
  space. That work used `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`;
  these overrides may be unnecessary on a local machine.
