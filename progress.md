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
`Cargo.toml` on this branch only; CI and a fresh clone need the sibling checkout
or the patch removed.

After C, VirtualList / Plain: Idle 1.96x, Scroll 2.08x, Filter 1.15x, Resize
2.03x. Only Filter meets task.md's 1.5x. The remaining gap is per-frame
overhead at one pass, not extra passes. Profiled (measurements.md, "Idle gap
attribution"): taffy itself costs nothing at idle; ~80% of the gap is
egui_taffy creating one or two egui `Ui`s per taffy node (9 `Ui`s per row vs
4 in Plain). Decision D (keep egui_taffy and upstream B + C, or replace it
with an own layer over taffy) is open for the user; plan section 5 lists the
criteria and the profile favours the own layer.

Known pre-existing: `cargo fmt --all -- --check` fails on ~30 untouched files
(import order); the two board gallery snapshots are missing.

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
