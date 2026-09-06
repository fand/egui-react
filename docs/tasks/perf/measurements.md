# Matched list-10k CPU measurements

Date: 2026-09-06. Native macOS arm64, Rust 1.95.0, release profile;
egui 0.36.1 and egui_taffy 0.14.0. Base commit: b166a7d, plus the benchmark
introduced alongside this report. One recorded run, 120 measured frames per
scenario/mode after 30 idle warmup frames. Mode execution order rotates each
frame. Timings are machine-dependent, not regression thresholds.

## Reproduce

From the repository root:

```sh
PERF_CSV="$PWD/docs/tasks/perf/samples.csv" \
  cargo test --release -p list-10k --test scenarios -- --ignored --nocapture
```

`PERF_CSV` is optional. Use an absolute output path: Cargo runs the test from the
package directory. The test prints summaries and writes one CSV row per pass.
`frame_cpu_ms` is the entire frame duration repeated on each pass row; do not sum
that column over passes. Frame and pass indices start at zero.

## What is matched

- 10,000 source rows, the same row generator and cache invalidation logic.
  Filter cache rebuilding is included in timing for every mode.
- Identical egui toolbar, reserved header region, panel margins, fonts, input,
  initial 600×800 viewport, and 20-point row pitch. A live FPS label is omitted
  because its text can itself change the layout.
- Both virtualized variants call the production `ScrollArea::show_rows`, with
  zero external vertical spacing and a 20-point height argument. The benchmark
  asserts equal final-pass row counts on every frame.
- `Virtual` uses the real `<VirtualList>` and the example's real `<Row>` unchanged.
  `All` uses `<ScrollArea>` + `<View>` and the same Row for every item.
  `Plain` uses direct egui row layout as the reference implementation.
- Three allowed passes. The React modes use the root taffy container, Store pass
  lifecycle, and the application's repaint fallback when a discard is refused.
- No AccessKit, GPU, browser, presentation/vsync, or OS event delivery.

This is a controlled list-body benchmark, not an end-to-end benchmark of `App`:
its header and data cache are deliberately shared so they do not confound the
layout comparison. It does not measure the App's hooks or keyboard/focus event
handling. The existing `bench.rs` remains available for the original full-App
comparison, whose conditions differ.

CPU time means elapsed wall time around `Context::run_ui` plus tessellation of
the final shapes. It includes per-pass row-index collection and discard-reason
cloning. CSV formatting and summaries run outside timing. It is not process CPU
time or `stable_dt`. Idle frames are explicitly driven to measure their cost;
this says nothing about the app's idle wakeup rate or power consumption.

## Scenarios

| Scenario | Input during the 120 measured frames |
|---|---|
| Idle | No changes after warmup |
| Scroll | Trackpad Start, then a -20-point vertical Move each frame; pointer at (300,400); synthetic time advances at 120 Hz |
| Filter | Set `a`, `al`, `alp`, `alpha`, empty in a repeating cycle before drawing; cache rebuild occurs inside the timed region |
| Resize | Width `600 + 4*(frame % 40)`, height `800 + 2*(frame % 30)`; first frame matches warmup size |

The scroll gesture uses Start to disable egui's mouse-wheel smoothing. It is
still delivered through normal egui input and ScrollArea processing.

## Results

| Scenario | Mode | Mean ms | p50 ms | p95 ms | Passes/frame | Discard frames /120 | Final rows | Row callbacks/frame |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Idle | All | 44.135 | 43.951 | 45.579 | 1.00 | 0 | 10000 | 10000.00 |
| Idle | Virtual | 0.225 | 0.230 | 0.256 | 1.00 | 0 | 37 | 37.00 |
| Idle | Plain | 0.124 | 0.124 | 0.145 | 1.00 | 0 | 37 | 37.00 |
| Scroll | All | 45.678 | 45.587 | 47.410 | 1.00 | 0 | 10000 | 10000.00 |
| Scroll | Virtual | 0.452 | 0.553 | 0.760 | 2.00 | 60 | 37 | 74.00 |
| Scroll | Plain | 0.153 | 0.148 | 0.210 | 1.00 | 0 | 37 | 37.00 |
| Filter | All | 57.106 | 21.400 | 186.006 | 1.60 | 72 | 1250–10000 | 7000.00 |
| Filter | Virtual | 1.528 | 1.457 | 1.952 | 1.60 | 72 | 37 | 59.20 |
| Filter | Plain | 1.286 | 1.288 | 1.625 | 1.00 | 0 | 37 | 37.00 |
| Resize | All | 148.590 | 148.716 | 153.877 | 2.98 | 119 | 10000 | 29833.33 |
| Resize | Virtual | 0.662 | 0.679 | 0.739 | 2.98 | 119 | 37–40 | 116.06 |
| Resize | Plain | 0.153 | 0.149 | 0.195 | 1.00 | 0 | 37–40 | 38.90 |

| Scenario | Virtual pass histogram (passes: frames) | Virtual discard requests |
|---|---|---:|
| Idle | 1: 120 | 0 |
| Scroll | 1: 60, 3: 60 | 4440 |
| Filter | 1: 48, 2: 72 | 2136 |
| Resize | 1: 1, 3: 119 | 4750 |

Rows mean render callbacks, including `show_rows` overscan, not strictly
pixel-visible rows. All-mode counts include offscreen work. The final count is
asserted equal between Virtual and Plain; total callbacks across all passes
expose repeated work. Discard frame counts mean at least one request during the
frame. Request totals count each request separately, not extra passes.

## Interpretation

- Idle VirtualList is one pass with no discard in this fixture. Its remaining
  overhead requires profiling; this result does not isolate hashing, allocation,
  node maintenance, or hook overhead individually.
- Scrolling adds layout passes even with a fixed row count. Recordings show
  alternating one/three-pass Virtual frames, rather than two passes every frame.
  The aggregate reason is `Taffy recalculation` (egui_taffy `src/lib.rs:712`).
  It does not distinguish a new node, changed measurement, or root-size change.
- The final submitted index ranges differ between Virtual and Plain on half of
  scroll frames, by one row at the start. Counts match. This is consistent with
  an additional pass seeing the updated scroll position in the same frame;
  attribution needs a separate scroll-offset trace. Do not claim identical row
  identities or visual timing in this scenario.
- Filter changes include rebuilding strings for 10,000 source items in both
  modes. The narrowing queries produce 1,250 matches; `a` produces 5,000 and an
  empty filter 10,000. Additional React layout passes add cost beyond that shared
  data work. This is an intentionally continuous-edit workload.
- Resizing nearly always uses three React passes. The unchanged first resize
  frame needs one pass. None of the recorded modes requests a discard that the
  pass limit refuses.
- All-row rendering remains expensive even when layout is stable; clipping does
  not avoid executing all row bodies. The all-row baseline makes this work
  explicit rather than conflating it with virtualized overhead.

The production direction remains declarative React-like rows. Next investigate
which nodes/root sizes become dirty on scrolling and resizing, then eliminate
unnecessary layout invalidation or redraw passes while preserving the component
API. This benchmark does not adopt direct egui rows for React mode, establish
web/120-Hz performance, or prove that floating-point jitter causes idle discards.

## After step A (slot-keyed VirtualList row trees)

Date: 2026-09-06. Same machine and conditions as the baseline above: native
macOS arm64, Rust 1.95.0, release profile, egui 0.36.1, egui_taffy 0.14.0 from
crates.io (no `[patch]`). Code state: uncommitted, on top of `cd7b599`. Samples
in [samples-a.csv](samples-a.csv). One recorded run.

Reproduce:

```sh
PERF_CSV="$PWD/docs/tasks/perf/samples-a.csv" \
  cargo test --release -p list-10k --test scenarios -- --ignored --nocapture
```

| Scenario | Mode | Mean ms | p50 ms | p95 ms | Passes/frame | Discard frames /120 | Final rows | Row callbacks/frame |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Idle | All | 39.380 | 38.932 | 42.846 | 1.00 | 0 | 10000 | 10000.00 |
| Idle | Virtual | 0.222 | 0.226 | 0.253 | 1.00 | 0 | 37 | 37.00 |
| Idle | Plain | 0.114 | 0.115 | 0.129 | 1.00 | 0 | 37 | 37.00 |
| Scroll | All | 39.682 | 39.339 | 42.451 | 1.00 | 0 | 10000 | 10000.00 |
| Scroll | Virtual | 0.466 | 0.598 | 0.741 | 2.00 | 60 | 37 | 74.00 |
| Scroll | Plain | 0.149 | 0.136 | 0.306 | 1.00 | 0 | 37 | 37.00 |
| Filter | All | 50.269 | 18.222 | 166.684 | 1.60 | 72 | 1250–10000 | 7000.00 |
| Filter | Virtual | 1.263 | 1.233 | 1.661 | 1.60 | 72 | 37 | 59.20 |
| Filter | Plain | 1.021 | 1.002 | 1.307 | 1.00 | 0 | 37 | 37.00 |
| Resize | All | 130.745 | 130.700 | 136.807 | 2.98 | 119 | 10000 | 29833.33 |
| Resize | Virtual | 0.653 | 0.662 | 0.734 | 2.98 | 119 | 37–40 | 116.06 |
| Resize | Plain | 0.147 | 0.140 | 0.197 | 1.00 | 0 | 37–40 | 38.90 |

| Scenario | Virtual pass histogram (passes: frames) | Virtual discard requests |
|---|---|---:|
| Idle | 1: 120 | 0 |
| Scroll | 1: 60, 3: 60 | 4440 |
| Filter | 1: 48, 2: 72 | 2136 |
| Resize | 1: 1, 3: 119 | 4750 |

### Compared with the baseline

Nothing in the pass behaviour moved. Pass histograms, discard frames, discard
request totals, row callbacks and final row ranges are identical to the
baseline, scenario for scenario. Frame times shifted by roughly the same
fraction in every mode, `Plain` included (Idle All 44.1 to 39.4 ms, Idle Plain
0.124 to 0.114 ms), so the timing difference is run-to-run variation on the
machine, not the change. The Virtual/Plain ratio is unchanged within that
noise: 1.95 idle and 3.13 scrolling, against 1.81 and 2.95 in the baseline.
What did change is not in this table: `<VirtualList>` no longer opens a taffy
tree per scrolled row, so egui memory stops growing with the scroll distance
(`egui_memory_does_not_grow_with_scroll_distance` in
`crates/egui-react-elements/tests/virtual_list.rs`: 85 entries against 771 for
the same scroll before the change).

**The gate in section 0 of the plan is not met.** "Scroll passes/frame drop
from 2.00" did not happen; "no new node per scrolled row" did.

The reason the gate cannot be met by keying alone: the discard on a scroll
frame is not driven by new nodes. Every row tree recomputes on every scroll
frame because its *root rect* changes size. `TuiInitializer::show`
(`egui_taffy/src/lib.rs:129`) takes `ui.available_rect_before_wrap()` as the
tree's root rect, and `recalculate` (`src/lib.rs:632`) recomputes and discards
when `state.last_size != root_rect.size()`. Inside `ScrollArea::show_rows` that
rect runs from the row down to the bottom of the viewport, so its height is a
function of where the row sits on screen. Measured with a temporary probe that
printed `cx.ui().available_rect_before_wrap()` at the top of each row body,
scrolling one row per frame: on three consecutive frames the top slot saw root
heights of 322, 345 and 322 points, and row 1's own tree saw 301, 324 and 322.
Every tree gets a different root height on every scroll frame, whether it is
keyed by row index (each row's own tree, moving up the screen) or by slot (one
tree per screen position, taking a new row each frame).

So the recompute is unavoidable here, but the *result* of it is not: the row has
a fixed height and a `grow` text, so recomputing yields the same layout it had
before. That is exactly the case step B skips the discard for. Step A's value
is the node and memory work it removes, and it is a precondition for B: with
per-row trees, a scrolled-in row is a `first_frame` tree that has to discard
whatever B does.

## After step B (egui_taffy skips the discard when the layout is unchanged)

Date: 2026-09-06. Same machine and conditions as above: native macOS arm64,
Rust 1.95.0, release profile, egui 0.36.1. egui_taffy is now the fork at
`../egui_taffy`, branch `skip-unchanged-discard`, commit `d618550` ("Ask for a
discard only when the layout changed"), wired in through `[patch.crates-io]` in
the workspace `Cargo.toml`. Code state: step A committed (`798f327`), the patch
line and this section uncommitted. Samples in [samples-b.csv](samples-b.csv).
One recorded run.

Reproduce:

```sh
PERF_CSV="$PWD/docs/tasks/perf/samples-b.csv" \
  cargo test --release -p list-10k --test scenarios -- --ignored --nocapture
```

| Scenario | Mode | Mean ms | p50 ms | p95 ms | Passes/frame | Discard frames /120 | Final rows | Row callbacks/frame |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Idle | All | 39.096 | 38.780 | 41.543 | 1.00 | 0 | 10000 | 10000.00 |
| Idle | Virtual | 0.225 | 0.229 | 0.257 | 1.00 | 0 | 37 | 37.00 |
| Idle | Plain | 0.113 | 0.113 | 0.127 | 1.00 | 0 | 37 | 37.00 |
| Scroll | All | 39.217 | 38.892 | 42.506 | 1.00 | 0 | 10000 | 10000.00 |
| Scroll | Virtual | 0.298 | 0.283 | 0.483 | 1.00 | 0 | 37 | 37.00 |
| Scroll | Plain | 0.146 | 0.130 | 0.314 | 1.00 | 0 | 37 | 37.00 |
| Filter | All | 50.293 | 18.436 | 166.957 | 1.60 | 72 | 1250–10000 | 7000.00 |
| Filter | Virtual | 1.158 | 1.142 | 1.442 | 1.00 | 0 | 37 | 37.00 |
| Filter | Plain | 1.014 | 0.968 | 1.303 | 1.00 | 0 | 37 | 37.00 |
| Resize | All | 131.082 | 130.828 | 136.556 | 2.98 | 119 | 10000 | 29833.33 |
| Resize | Virtual | 0.649 | 0.660 | 0.718 | 2.98 | 119 | 37–40 | 116.06 |
| Resize | Plain | 0.149 | 0.141 | 0.205 | 1.00 | 0 | 37–40 | 38.90 |

| Scenario | Virtual pass histogram (passes: frames) | Virtual discard requests |
|---|---|---:|
| Idle | 1: 120 | 0 |
| Scroll | 1: 120 | 0 |
| Filter | 1: 120 | 0 |
| Resize | 1: 1, 3: 119 | 4750 |

### Compared with step A

| Scenario | Passes/frame after A | Passes/frame after B | Virtual mean ms A → B |
|---|---:|---:|---|
| Idle | 1.00 | 1.00 | 0.222 → 0.225 |
| Scroll | 2.00 | 1.00 | 0.466 → 0.298 |
| Filter | 1.60 | 1.00 | 1.263 → 1.158 |
| Resize | 2.98 | 2.98 | 0.653 → 0.649 |

**The gate in section 0 for step B is met.** Scroll and Filter are both at one
pass per frame, with zero discard requests over the 120 measured frames, down
from 4,440 and 2,136. Row callbacks follow: Scroll 74.00 to 37.00 and Filter
59.20 to 37.00 per frame, so every row body now runs once. Scroll Virtual mean
frame time drops 36% and is now 2.04x Plain, against 3.13x after A. Filter loses
its extra passes but only 8% of its time, because most of that scenario is the
shared 10,000-item cache rebuild, which both modes pay.

Resize is untouched, as expected: it is not the same problem. There the layout
really does change every frame, so the compare in the fork finds a difference
and asks for the discard the old code asked for anyway. That is step C.

Filter in `All` mode still runs 1.60 passes: the filter adds and removes rows,
so nodes are created and removed in the one big tree, and both are reasons to
discard.

### One deviation from the plan

Plan section 3.2 says to compare the whole `taffy::Layout`. That does not work:
`Layout` carries `content_size`, and for a leaf that is simply what the leaf
measured. On a scroll frame the row's text leaf measures a different width every
frame while its box stays put, so a whole-`Layout` compare reports a change on
every frame and nothing improves. Measured on the fork's own row-shaped test
tree: between two frames only `content_size` moved, 50.0 to 60.0, on the growing
leaf; `location`, `size`, `border`, `padding`, `margin`, `scrollbar_size` and
`order` were identical on all four nodes, including the root.

So the fork compares every field except `content_size` on every node, and
`content_size` as well on the two nodes egui_taffy reads it from: the root,
where it is the space the tree allocates in the surrounding `Ui`
(`TuiInitializer::show`, `src/lib.rs:137`), and any node with `overflow: scroll`,
where it is the size of the scrolled content (`src/lib.rs:466`). On any other
node nothing reads it.

The other two discard reasons are kept as the plan describes: a node created
this frame drew invisible in a sizing pass, so it always needs a second pass;
a node removed this frame is treated as a change, because it is already out of
the id map when the compare runs and the compare cannot see what dropping it
did.

## After step C (egui_taffy computes the layout before drawing on a resize)

Date: 2026-09-06. Same machine and conditions as above: native macOS arm64,
Rust 1.95.0, release profile, egui 0.36.1. egui_taffy is the fork at
`../egui_taffy`, branch `layout-first`, commit `ee07d38` ("Compute the layout
before drawing when only the root rect resized"), which sits on top of step B's
`d618550`, wired in through `[patch.crates-io]` in the workspace `Cargo.toml`.
Code state: step A committed (`798f327`), the patch line, the resize guard test
and this section uncommitted. Samples in [samples-c.csv](samples-c.csv). One
recorded run.

Reproduce:

```sh
PERF_CSV="$PWD/docs/tasks/perf/samples-c.csv" \
  cargo test --release -p list-10k --test scenarios -- --ignored --nocapture
```

| Scenario | Mode | Mean ms | p50 ms | p95 ms | Passes/frame | Discard frames /120 | Final rows | Row callbacks/frame |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Idle | All | 43.029 | 43.048 | 46.206 | 1.00 | 0 | 10000 | 10000.00 |
| Idle | Virtual | 0.241 | 0.250 | 0.270 | 1.00 | 0 | 37 | 37.00 |
| Idle | Plain | 0.123 | 0.123 | 0.143 | 1.00 | 0 | 37 | 37.00 |
| Scroll | All | 42.645 | 42.743 | 46.340 | 1.00 | 0 | 10000 | 10000.00 |
| Scroll | Virtual | 0.303 | 0.306 | 0.366 | 1.00 | 0 | 37 | 37.00 |
| Scroll | Plain | 0.146 | 0.139 | 0.185 | 1.00 | 0 | 37 | 37.00 |
| Filter | All | 54.006 | 19.967 | 177.632 | 1.60 | 72 | 1250–10000 | 7000.00 |
| Filter | Virtual | 1.222 | 1.187 | 1.575 | 1.00 | 0 | 37 | 37.00 |
| Filter | Plain | 1.065 | 1.018 | 1.376 | 1.00 | 0 | 37 | 37.00 |
| Resize | All | 54.385 | 54.937 | 56.758 | 1.00 | 0 | 10000 | 10000.00 |
| Resize | Virtual | 0.311 | 0.310 | 0.471 | 1.02 | 3 | 37–40 | 39.88 |
| Resize | Plain | 0.153 | 0.145 | 0.209 | 1.00 | 0 | 37–40 | 38.90 |

| Scenario | Virtual pass histogram (passes: frames) | Virtual discard requests |
|---|---|---:|
| Idle | 1: 120 | 0 |
| Scroll | 1: 120 | 0 |
| Filter | 1: 120 | 0 |
| Resize | 1: 117, 2: 3 | 3 |

### Compared with step B

| Scenario | Passes/frame after B | Passes/frame after C | Virtual mean ms B → C |
|---|---:|---:|---|
| Idle | 1.00 | 1.00 | 0.225 → 0.241 |
| Scroll | 1.00 | 1.00 | 0.298 → 0.303 |
| Filter | 1.00 | 1.00 | 1.158 → 1.222 |
| Resize | 2.98 | 1.02 | 0.649 → 0.311 |

**The gate in section 0 for step C is met.** Resize drops from 2.98 to 1.02
passes per frame. Discard frames fall from 119 to 3 and discard requests from
4,750 to 3. Row callbacks per frame fall from 116.06 to 39.88, so every row body
runs once instead of three times, and the Virtual mean frame time halves, 0.649
to 0.311 ms. `All` mode gains the same way: 2.98 to 1.00 passes and 131 to 54 ms
per frame, because its one big tree also lays out before it draws.

The other three scenarios are unchanged, as expected: they were already at one
pass and their root rect never resizes, so the early layout never runs. Their
frame times moved 1–6% up, in the same direction as `Plain` (Idle Plain 0.113 to
0.123 ms), so that is run-to-run variation on the machine, not the change.

### The passes that remain

Three Resize frames still need two passes: frames 1, 11 and 21. The window
height cycles `800 + 2*(frame % 30)`, so the visible row count grows 37, 38, 39,
40 on exactly those frames. Each new row is a `<VirtualList>` slot that has no
tree yet, so a whole tree of taffy nodes is created, drawn invisible in a sizing
pass, and has to be drawn again.

Measured, not inferred: with the discard reason temporarily extended to carry
the three flags, all three requests read
`Taffy recalculation c=true r=false m=true` — created and moved, never removed.
`moved` is trivially true there, because a node created this frame is compared
against the zero layout a fresh taffy leaf has. So the single reason is
**created**, in the row tree of a slot the taller window just added. Nothing
discards for `removed`: when the window shrinks again at frame 30 the row count
drops back to 37 and no slot tree is torn down, because a slot that is not drawn
simply is not visited. And slots 37 to 39 are reused when the window grows a
second time, which is why the three frames are the first cycle only.

### Virtual against Plain

| Scenario | Virtual / Plain after B | Virtual / Plain after C |
|---|---:|---:|
| Idle | 1.99x | 1.96x |
| Scroll | 2.04x | 2.08x |
| Filter | 1.14x | 1.15x |
| Resize | 4.36x | 2.03x |

Against task.md's "no more than 1.5 times the plain egui cost": Filter passes at
1.15x. Idle (1.96x), Scroll (2.08x) and Resize (2.03x) do not. Resize is the
scenario C moved, from 4.36x to 2.03x, and it is now in the same band as Idle
and Scroll rather than a case of its own.

The remaining gap is no longer extra passes: every scenario runs one pass per
frame apart from three frames out of 480. It is the per-frame cost of one React
pass against one direct egui pass, and Idle is the cleanest reading of it: 0.241
against 0.123 ms with no discard, no node creation and no layout change on
either side. Closing that needs a profile of the idle path (state lookup in egui
memory, per-node `Style` comparison, the `id_to_node_id` retain sweep, hook
overhead), which is what decision D in the plan weighs.

## Idle gap attribution (after step C)

Date: 2026-09-06. Instruments Time Profiler via `xctrace`, release build with
line tables, one scenario and one mode per process (40,000 Virtual frames,
7,524 samples). Samples are charged to the nearest enclosing egui_taffy /
egui-react / egui / epaint frame. Temporary ablation modes were added to the
benchmark and reverted; nothing here is committed code. Isolated runs are
faster than the mixed benchmark (Virtual 0.187 / Plain 0.099 ms here vs
0.241 / 0.123 published) with the same 1.9x ratio.

| Mode | Idle ms |
|---|---:|
| Plain | 0.099 |
| Flat: whole visible range in one `<View>` tree (ablation) | 0.149 |
| Virtual (root tree + 37 row trees) | 0.187 |
| Virtual without the per-row `push_id` (ablation) | 0.173 |

| Bucket | Virtual µs/frame | Flat | Plain |
|---|---:|---:|---:|
| (1) egui_taffy per tree: `TuiInitializer::show`, `Tui::create`, `recalculate` | 8.3 | 1.0 | 0 |
| (2) egui_taffy per node: `add_container_dyn`, `add_child_dyn`, `add_child_node`, and the egui `Ui` work they induce | 71.9 | 68.9 | 0 |
| (3) egui-react: `Cx::scope` 28.2, `ItemStyle::to_taffy` 2.1, `View`/`leaf`/`container` ~4, `Store` 0.05 | 38.3 | 11.3 | 0 |
| (4) drawing the rows themselves: galleys, tessellation, scroll area | 69.7 | 67.6 | 63.3 |
| egui's own row layout, paid by Plain only (`horizontal`, `with_layout`, `push_id`) | 0 | 0 | 33.0 |
| Total | 188.1 | 148.9 | 96.7 |

Findings:

- Taffy's algorithm costs nothing at idle: no sample lands in a `taffy::`
  frame. `recalculate` is about 7 ns per tree. Per-tree state lookup and lock
  are negligible.
- Per-node work dominates. egui_taffy builds one child `Ui` per node
  (`add_child_dyn`, `lib.rs:453`) and a second one per leaf
  (`add_container_dyn`, `lib.rs:622`). Each `Ui::new_child` clones Arcs,
  registers an accesskit parent, and calls `create_widget` / `get_response`.
  A `<Row>` is 4 nodes = 7 `Ui`s, plus the tree `Ui` and the row `push_id`
  `Ui`: 9 per row against 4 in Plain. That ratio (2.25) matches the measured
  1.94x.
- The per-row `push_id` in VirtualList is worth 0.013 to 0.028 ms per frame
  and exists only because rows sit in a plain `Ui` between trees.
- Top self-time symbols (owner-attributed, µs/frame): `Ui::new_child` 23.2,
  `Tui::add_container_dyn` 17.0, `Context::get_response` 16.7,
  `Context::create_widget` 12.6, `Tui::add_child_dyn` 6.3, `Ui::scope_dyn`
  6.1, `WidgetRects::insert` 5.5, `tessellate_text` 5.4.

Implication for plan section 5: the remaining gap is not the per-tree
bookkeeping the plan named (9%, cheap). It is per-node `Ui` construction in
egui_taffy. Fixing that upstream means collapsing the leaf's two `Ui`s into
one and skipping widget/accesskit registration for non-interactive nodes,
which egui_taffy's backgrounds, interactive containers and sticky scrolling
rely on, and may need an egui change. An own thin layer over taffy could
create one `Ui` per leaf and register a response only when the element asks,
with an expected floor near Plain + 0.03 to 0.05 ms instead of + 0.12 ms.
Steps B and C stand on their own and should go upstream either way.

## After D1 (own layout engine, egui_taffy dropped)

Date: 2026-09-06. Same machine and conditions as above: native macOS arm64,
Rust 1.95.0, release profile, egui 0.36.1, taffy 0.9.2 as a direct dependency
with the features egui_taffy 0.14.0 used. egui_taffy and the
`[patch.crates-io]` line are gone; `crates/egui-react/src/engine.rs` is the
replacement. Code state: step D1 uncommitted on top of `84ddeff`. Samples in
[samples-d1.csv](samples-d1.csv). One recorded run.

Reproduce:

```sh
PERF_CSV="$PWD/docs/tasks/perf/samples-d1.csv" \
  cargo test --release -p list-10k --test scenarios -- --ignored --nocapture
```

| Scenario | Mode | Mean ms | p50 ms | p95 ms | Passes/frame | Discard frames /120 | Final rows | Row callbacks/frame |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Idle | All | 26.699 | 26.355 | 29.427 | 1.00 | 0 | 10000 | 10000.00 |
| Idle | Virtual | 0.160 | 0.162 | 0.182 | 1.00 | 0 | 37 | 37.00 |
| Idle | Plain | 0.112 | 0.112 | 0.127 | 1.00 | 0 | 37 | 37.00 |
| Scroll | All | 26.950 | 26.598 | 29.860 | 1.00 | 0 | 10000 | 10000.00 |
| Scroll | Virtual | 0.221 | 0.214 | 0.314 | 1.00 | 0 | 37 | 37.00 |
| Scroll | Plain | 0.139 | 0.134 | 0.189 | 1.00 | 0 | 37 | 37.00 |
| Filter | All | 41.226 | 15.058 | 140.088 | 1.60 | 72 | 1250–10000 | 7000.00 |
| Filter | Virtual | 1.127 | 1.099 | 1.423 | 1.00 | 0 | 37 | 37.00 |
| Filter | Plain | 1.038 | 1.006 | 1.329 | 1.00 | 0 | 37 | 37.00 |
| Resize | All | 38.620 | 38.182 | 41.152 | 1.00 | 0 | 10000 | 10000.00 |
| Resize | Virtual | 0.247 | 0.241 | 0.398 | 1.10 | 12 | 37–40 | 42.80 |
| Resize | Plain | 0.144 | 0.136 | 0.220 | 1.00 | 0 | 37–40 | 38.90 |

| Scenario | Virtual pass histogram (passes: frames) | Virtual discard requests |
|---|---|---:|
| Idle | 1: 120 | 0 |
| Scroll | 1: 120 | 0 |
| Filter | 1: 120 | 0 |
| Resize | 1: 108, 2: 12 | 12 |

### Virtual against Plain

| Scenario | Virtual / Plain after C | Virtual / Plain after D1 |
|---|---:|---:|
| Idle | 1.96x | 1.43x |
| Scroll | 2.08x | 1.59x |
| Filter | 1.15x | 1.09x |
| Resize | 2.03x | 1.72x |

### Compared with step C

| Scenario | Passes/frame after C | Passes/frame after D1 | Virtual mean ms C → D1 |
|---|---:|---:|---|
| Idle | 1.00 | 1.00 | 0.241 → 0.160 |
| Scroll | 1.00 | 1.00 | 0.303 → 0.221 |
| Filter | 1.00 | 1.00 | 1.222 → 1.127 |
| Resize | 1.02 | 1.10 | 0.311 → 0.247 |

Plain moved 0.123 → 0.112 (Idle) and 0.153 → 0.144 (Resize), so about a tenth
of the Virtual gain is the machine being a little quicker on this run. The rest
is the engine: a `<Row>` used to cost nine egui `Ui`s (one per taffy node, one
more per leaf, one for the tree, one for the row's `push_id`) and now costs
three (the tree's, and one per leaf), because a container node is a rect and no
longer a `Ui`. Idle Virtual lands at **0.160 ms**, under the 0.20 ms the D1 gate
asked for, and the Idle ratio falls from 1.96x to 1.43x — under task.md's 1.5x
for the first time. Scroll (1.59x) and Resize (1.72x) are close; Filter was
already through at 1.15x and is now 1.09x.

`All` mode gains the same way, 43 → 27 ms per frame at Idle: it is one big tree
with the same nodes.

### The one metric that moved the wrong way

Resize goes from 1.02 to 1.10 passes per frame, 3 discard frames to 12. The
cause is measured, not inferred: with the tree sweep in `Store::end_pass`
disabled and nothing else changed, the same run gives Resize Virtual 1.02
passes and 3 discard frames again (0.250 ms mean), and the other three
scenarios do not move.

Why: the window height cycles `800 + 2 * (frame % 30)`, so the visible row
count runs 37, 38, 39, 40 and back, four times over 120 frames. Slots 37 to 39
exist only at the top of each cycle. egui_taffy kept every tree in egui memory
for ever, so those three trees were built once, in frames 1, 11 and 21, and
reused in the three later cycles. The engine keys its trees the same way but
drops a tree that was not drawn in the pass, so each cycle builds them again:
3 × 4 = 12 frames where a new node has to draw in an invisible sizing pass.
That is the same bookkeeping that makes a tree left behind by an unmounted
subtree go away instead of growing egui's `IdTypeMap` for ever — the growth
step A worked around.

It is a trade, not a bug, and it is cheap to change: keeping a tree for a
bounded number of passes after it was last drawn would give back the 1.02 and
still bound the memory. That is a follow-up, not part of D1, because a
time-to-live is a knob and picking its value from this benchmark would be
tuning to the benchmark. Nothing else regressed: mean frame time is *better*
on Resize too (0.311 → 0.247), because the twelve two-pass frames are cheaper
than step C's three were.

## After D2 (`<Text>` is a galley on a taffy node, not a `Label` in a `Ui`)

Date: 2026-09-06. Same machine and conditions as above: native macOS arm64,
Rust 1.95.0, release profile, egui 0.36.1, taffy 0.9.2. `<Text>` no longer puts
an `egui::Label` in a leaf `Ui`: the galley is laid out inside the taffy measure
function, painted straight onto the tree's own `Ui`, and registered with the
widget rect and the `WidgetInfo` that `Label` writes. Code state: step D2
uncommitted on top of `5a9f04a`. Samples in [samples-d2.csv](samples-d2.csv).
One recorded run; a second run of the same build agreed to within 0.003 ms on
every Virtual figure.

Reproduce:

```sh
PERF_CSV="$PWD/docs/tasks/perf/samples-d2.csv" \
  cargo test --release -p list-10k --test scenarios -- --ignored --nocapture
```

| Scenario | Mode | Mean ms | p50 ms | p95 ms | Passes/frame | Discard frames /120 | Final rows | Row callbacks/frame |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Idle | All | 21.651 | 21.392 | 23.680 | 1.00 | 0 | 10000 | 10000.00 |
| Idle | Virtual | 0.153 | 0.157 | 0.185 | 1.00 | 0 | 37 | 37.00 |
| Idle | Plain | 0.116 | 0.114 | 0.140 | 1.00 | 0 | 37 | 37.00 |
| Scroll | All | 21.740 | 21.466 | 23.260 | 1.00 | 0 | 10000 | 10000.00 |
| Scroll | Virtual | 0.214 | 0.221 | 0.257 | 1.00 | 0 | 37 | 37.00 |
| Scroll | Plain | 0.135 | 0.134 | 0.166 | 1.00 | 0 | 37 | 37.00 |
| Filter | All | 41.527 | 8.893 | 162.080 | 1.60 | 72 | 1250–10000 | 7000.00 |
| Filter | Virtual | 1.094 | 1.071 | 1.410 | 1.00 | 0 | 37 | 37.00 |
| Filter | Plain | 1.025 | 0.996 | 1.304 | 1.00 | 0 | 37 | 37.00 |
| Resize | All | 34.423 | 34.319 | 36.600 | 1.00 | 0 | 10000 | 10000.00 |
| Resize | Virtual | 0.211 | 0.210 | 0.353 | 1.10 | 12 | 37–40 | 42.80 |
| Resize | Plain | 0.133 | 0.131 | 0.154 | 1.00 | 0 | 37–40 | 38.90 |

| Scenario | Virtual pass histogram (passes: frames) | Virtual discard requests |
|---|---|---:|
| Idle | 1: 120 | 0 |
| Scroll | 1: 120 | 0 |
| Filter | 1: 120 | 0 |
| Resize | 1: 108, 2: 12 | 12 |

### Virtual against Plain

| Scenario | Virtual / Plain after D1 | Virtual / Plain after D2 |
|---|---:|---:|
| Idle | 1.43x | 1.32x |
| Scroll | 1.59x | 1.59x |
| Filter | 1.09x | 1.07x |
| Resize | 1.72x | 1.59x |

### Compared with step D1

| Scenario | Passes/frame after D1 | Passes/frame after D2 | Virtual mean ms D1 → D2 |
|---|---:|---:|---|
| Idle | 1.00 | 1.00 | 0.160 → 0.153 |
| Scroll | 1.00 | 1.00 | 0.221 → 0.214 |
| Filter | 1.00 | 1.00 | 1.127 → 1.094 |
| Resize | 1.10 | 1.10 | 0.247 → 0.211 |

Plain moved 0.112 → 0.116 (Idle) and 0.144 → 0.133 (Resize) between the two
recordings, so part of the Resize gain and none of the Idle gain is the machine.
The pass behaviour is untouched: the same histograms, the same twelve Resize
discard frames from D1's tree sweep, and every row body still runs once.

A `<Row>` is a `<View>` holding two `<Text>` and one `<Button>`. In D1 it cost
the tree's own `Ui` plus one per leaf: four. Now the two `<Text>` leaves open no
`Ui` at all, so it costs two, and `<Text>` skips `Label`'s own work as well —
the `Ui`'s placer, the allocation, the alignment, and the text selection state
that `Label` runs for every label whether or not anything is selected. (The
selection state came back in D2b below, and cost nothing measurable; the `Ui`s
are the part that mattered.)

Where that shows and where it does not:

- **Idle** falls to 1.32x, from 1.43x. This is the cleanest reading: no
  computation, no new nodes, so what is left is the per node cost of one React
  pass, and the two `Ui`s a row no longer builds are most of what went.
- **Resize** falls to 1.59x, from 1.72x.
- **Scroll** does not move (1.59x). A scrolled frame recomputes every row tree,
  because the rect a row inside a `ScrollArea` is given changes height on every
  frame, and D2 makes that computation slightly dearer: the text nodes are
  measured inside it instead of before it. The `Ui`s saved and the measurement
  added roughly cancel, so the whole D2 gain lands on the frames that do not
  recompute. Idle 0.153 against Scroll 0.214 is that computation, 0.061 ms for
  37 trees.
- **Filter** was through the criterion already, and is now 1.07x.
- **All** mode gains most of all, 26.7 → 21.7 ms per frame at Idle: it is one
  tree with 20,000 `<Text>` in it, and every one of them used to open a `Ui`.

Against task.md's 1.5x: Idle (1.32x), Filter (1.07x) pass; Scroll (1.59x) and
Resize (1.59x) do not, both within a tenth of it.

### What D2 changed about passes

A `<Text>` is measured, not drawn: the galley is laid out while taffy asks the
node for its size, so a new `<Text>` needs no invisible sizing pass and does not
force a second one. Two rules follow from that and are now in the engine:

- only a leaf that draws to be measured sets `created_this_frame`;
- a node created during the frame takes no part in the "did anything move?"
  comparison, because taffy leaves a new node at zero and there is no layout it
  was drawn with.

For that to be safe the text has to be painted where the node ends up, not where
it was last frame, so `<Text>` claims its shape in the painter in draw order and
fills it in after the layout is computed. A tree of `<View>` and `<Text>` alone
is then right on its first frame and costs one pass
(`crates/egui-react/tests/engine_text.rs`). A tree with any other widget in it
still costs two, as before: that widget has to draw to be measured. This is why
the twelve Resize frames are unchanged — a new `<VirtualList>` slot draws a
`<Button>`.

## After D2b (text selection back on the engine's `<Text>`)

Date: 2026-09-06. Same machine and conditions as above: native macOS arm64,
Rust 1.95.0, release profile, egui 0.36.1, taffy 0.9.2. D2 dropped `Label`'s
text selection; D2b puts it back. A `<Text>` now takes the sense `Label` takes
(`click_and_drag` minus `FOCUSABLE` when selectable), and a `<Text>` whose place
on screen is already known paints itself in draw order through
`LabelSelectionState::label_text_selection` instead of adding a bare
`TextShape`. A `<Text>` created this frame keeps D2's deferred paint for that
one frame, so a new `<View>` / `<Text>` tree is still right in one pass. Code
state: step D2b uncommitted on top of `2c1a3ac`. Samples in
[samples-d2b.csv](samples-d2b.csv). One recorded run; a second run of the same
build agreed to within 0.006 ms on every Virtual figure but Resize, which moved
0.016 ms while its `Plain` moved 0.012 ms the same way.

Reproduce:

```sh
PERF_CSV="$PWD/docs/tasks/perf/samples-d2b.csv" \
  cargo test --release -p list-10k --test scenarios -- --ignored --nocapture
```

| Scenario | Mode | Mean ms | p50 ms | p95 ms | Passes/frame | Discard frames /120 | Final rows | Row callbacks/frame |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Idle | All | 22.020 | 21.846 | 23.729 | 1.00 | 0 | 10000 | 10000.00 |
| Idle | Virtual | 0.152 | 0.156 | 0.182 | 1.00 | 0 | 37 | 37.00 |
| Idle | Plain | 0.118 | 0.116 | 0.140 | 1.00 | 0 | 37 | 37.00 |
| Scroll | All | 21.975 | 21.733 | 23.833 | 1.00 | 0 | 10000 | 10000.00 |
| Scroll | Virtual | 0.216 | 0.220 | 0.266 | 1.00 | 0 | 37 | 37.00 |
| Scroll | Plain | 0.134 | 0.131 | 0.172 | 1.00 | 0 | 37 | 37.00 |
| Filter | All | 42.386 | 9.361 | 162.953 | 1.60 | 72 | 1250–10000 | 7000.00 |
| Filter | Virtual | 1.102 | 1.070 | 1.403 | 1.00 | 0 | 37 | 37.00 |
| Filter | Plain | 1.032 | 1.004 | 1.323 | 1.00 | 0 | 37 | 37.00 |
| Resize | All | 34.255 | 34.157 | 36.248 | 1.00 | 0 | 10000 | 10000.00 |
| Resize | Virtual | 0.217 | 0.213 | 0.347 | 1.10 | 12 | 37–40 | 42.80 |
| Resize | Plain | 0.137 | 0.134 | 0.174 | 1.00 | 0 | 37–40 | 38.90 |

| Scenario | Virtual pass histogram (passes: frames) | Virtual discard requests |
|---|---|---:|
| Idle | 1: 120 | 0 |
| Scroll | 1: 120 | 0 |
| Filter | 1: 120 | 0 |
| Resize | 1: 108, 2: 12 | 12 |

### Virtual against Plain

| Scenario | Virtual / Plain after D2 | Virtual / Plain after D2b |
|---|---:|---:|
| Idle | 1.32x | 1.29x |
| Scroll | 1.59x | 1.61x |
| Filter | 1.07x | 1.07x |
| Resize | 1.59x | 1.58x |

### What selection costs per frame

| Scenario | Virtual mean ms D2 → D2b | Plain mean ms D2 → D2b |
|---|---|---|
| Idle | 0.153 → 0.152 | 0.116 → 0.118 |
| Scroll | 0.214 → 0.216 | 0.135 → 0.134 |
| Filter | 1.094 → 1.102 | 1.025 → 1.032 |
| Resize | 0.211 → 0.217 | 0.133 → 0.137 |

Nothing here is a cost. Every Virtual move is at most 0.008 ms, `Plain` moved
by the same amount in the same direction on three of the four scenarios, and
the two D2b runs of the *same* build differ by as much. The benchmark's rows
are the case that matters: the `<Text>` nodes of the 37 visible rows exist
before the frame, so all 74 of them run the placed path and call
`label_text_selection` every frame. So the honest statement is an upper bound:
selection costs less than this benchmark's noise floor, about 0.01 ms per
frame, which over 74 texts is 0.0001 ms each.

That is the expected shape. `label_text_selection` on a label with nothing
selected reads the pointer position, finds no cursor range for the widget, and
adds the same `TextShape` the fast path added. The work D2 removed was the
`Ui`s, not the selection state.

The pass behaviour is untouched: the same histograms, the same twelve Resize
discard frames, every row body still runs once. It has to be, and the engine is
what makes it so — an immediate paint sits at the layout the node was drawn
with, and a layout that moves is already a discard, so the picture can never be
left stale. Rule B still holds as well: a text whose galley changed but whose
rect did not still does not discard, because nothing about the paint path
touches the "did anything move?" comparison.

## After E (a fixed root rect for `<VirtualList>` rows)

Date: 2026-09-06. Same machine and conditions as above: native macOS arm64,
Rust 1.95.0, release profile, egui 0.36.1, taffy 0.9.2. Code state: step E
uncommitted on top of `dfc5dfc`. Samples in [samples-e.csv](samples-e.csv).

What changed. A `<VirtualList>` row's tree is no longer laid out into "the space
that is left": `Cx::with_root_size` hands it a rect of `ui.available_width()` by
`row_h`, both axes definite, and the row reserves exactly that much room instead
of the height its content came to. `engine::show` takes a `Reserve` (`Content` /
`AllSpace` / `Fixed`) instead of the `all_space` flag. Nothing else moved: the
B and C rules, the other elements and the benchmark are untouched.

Reproduce:

```sh
PERF_CSV="$PWD/docs/tasks/perf/samples-e.csv" \
  cargo test --release -p list-10k --test scenarios -- --ignored --nocapture
```

| Scenario | Mode | Mean ms | p50 ms | p95 ms | Passes/frame | Discard frames /120 | Final rows | Row callbacks/frame |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Idle | All | 21.551 | 21.325 | 23.472 | 1.00 | 0 | 10000 | 10000.00 |
| Idle | Virtual | 0.145 | 0.146 | 0.169 | 1.00 | 0 | 37 | 37.00 |
| Idle | Plain | 0.114 | 0.112 | 0.130 | 1.00 | 0 | 37 | 37.00 |
| Scroll | All | 21.565 | 21.366 | 23.069 | 1.00 | 0 | 10000 | 10000.00 |
| Scroll | Virtual | 0.219 | 0.221 | 0.264 | 1.00 | 0 | 37 | 37.00 |
| Scroll | Plain | 0.145 | 0.137 | 0.204 | 1.00 | 0 | 37 | 37.00 |
| Filter | All | 41.766 | 9.084 | 161.644 | 1.60 | 72 | 1250–10000 | 7000.00 |
| Filter | Virtual | 1.093 | 1.068 | 1.390 | 1.00 | 0 | 37 | 37.00 |
| Filter | Plain | 1.027 | 1.004 | 1.309 | 1.00 | 0 | 37 | 37.00 |
| Resize | All | 34.010 | 33.780 | 37.179 | 1.00 | 0 | 10000 | 10000.00 |
| Resize | Virtual | 0.240 | 0.228 | 0.359 | 1.10 | 12 | 37–40 | 42.80 |
| Resize | Plain | 0.152 | 0.138 | 0.231 | 1.00 | 0 | 37–40 | 38.90 |

| Scenario | Virtual pass histogram (passes: frames) | Virtual discard requests |
|---|---|---:|
| Idle | 1: 120 | 0 |
| Scroll | 1: 120 | 0 |
| Filter | 1: 120 | 0 |
| Resize | 1: 108, 2: 12 | 12 |

The pass behaviour is exactly D2b's, which is what step E had to leave alone:
the twelve Resize frames are the new slot trees, and a real window resize still
takes the early layout path (traced below).

### Virtual against Plain

| Scenario | Virtual / Plain after D2b | Virtual / Plain after E |
|---|---:|---:|
| Idle | 1.29x | 1.27x |
| Scroll | 1.61x | 1.51x |
| Filter | 1.07x | 1.06x |
| Resize | 1.58x | 1.58x |

**Read those two Scroll figures as one number, not as a change.** A second run
of the same build gives Virtual 0.144 / 0.208 / 1.082 / 0.210 ms and Plain
0.113 / 0.128 / 0.995 / 0.139 ms, that is 1.27x / **1.63x** / 1.09x / 1.51x. The
two runs of the same build differ by 0.011 ms on Scroll Virtual and 0.017 ms on
Scroll Plain, and by 0.030 ms on Resize Virtual; the ratio swings by 0.1x with
them. Nothing here separates E from D2b on time.

### Scroll did not move, and why

The expectation was Scroll Virtual dropping to about 0.17-0.18 ms, because
"After D2b" attributed the 0.064 ms scroll surcharge to the root rect of the 37
row trees changing height on every scrolled frame. Measured: 0.219 ms against
D2b's 0.216, inside the noise above.

That attribution was wrong, and the trace says so. Temporary `println!`s in
`Tree::layout_first_on_resize` (one line per early layout) and in `Tree::finish`
(the two triggers, `dirty(root)` and `last_size != root_rect.size()`), run over
40 frames of the same fixture — 30 idle, 10 scrolled by 20 pt, then 6 window
widths — give the same picture with and without step E:

| Frames | Trees | Computed | dirty | root size changed | Early layouts |
|---|---:|---:|---:|---:|---:|
| Idle (1-29) | 40 | 0 | 0 | 0 | 0 |
| Scrolled (31-39), before E | 40 | 39 | 39 | 0 | 0 |
| Scrolled (31-39), after E | 40 | 39 | 39 | 0 | 0 |
| Window width changed, after E | 40 | 0 | 0 | 0 | 40 |

So on a scrolled frame the root size was **already** constant before step E.
The benchmark scrolls by exactly 20 pt, which is exactly one row, and
`show_rows` hands the callback a `Ui` whose rect covers the visible rows only.
Slot *k*'s rect therefore runs from `y_min + k * 20` to `y_max`, and both ends
move by one row together: the same size on every frame. The 39 recomputations
are all `dirty`, and dirty because every slot shows a different row's text one
frame later, so `set_text` writes a new job onto the `<Text>` node. That work is
not the root rect, and a fixed root rect cannot remove it.

The last line of the table is the other half: after E a real width change still
takes the early layout in all 40 trees, so `layout_first_on_resize` was not
disabled by accident, and the Resize scenario keeps its 1: 108 / 2: 12.

### What E did change: the rows sit where `show_rows` put them

Not a timing result, a correctness one, and it is why the step is worth keeping.
A row used to reserve the height its content measured, which is not the `row_h`
the visible range was worked out from. In list-10k, `row_h` is 20 (`ROW_H` 18
plus `ROW_GAP` 2) and the rows landed 18 apart:

| | reserved by `show_rows` | measured pitch of `#0` .. `#11` |
|---|---:|---:|
| Before E | 20 | 18 |
| After E | 20 | 20 |

Two points per row, so the 21st visible row was drawn 40 points above where the
scroll area had put it, and the gap showed up as blank space at the bottom of
the viewport. `crates/egui-react-elements/tests/virtual_list.rs` now holds both
halves of this: rows at exactly `row_h` pitch over 20 scrolled frames with no
discard requested, and a row that draws twice as tall still moving the list on
by `row_h`.

## After E1 (`<VirtualList>` rows laid out without taffy)

Date: 2026-09-06. Same machine and conditions as above: native macOS arm64,
Rust 1.95.0, release profile, egui 0.36.1, taffy 0.9.2. Code state: step E1
uncommitted on top of `b0f2ba1`. Samples in [samples-e1.csv](samples-e1.csv).

What changed. A `<VirtualList>` row no longer holds a taffy tree. Its nodes are
a `Vec` rebuilt in draw order every frame, its styles are the `ItemStyle` and
`ContainerStyle` the elements already carry, and its boxes are solved by
`crates/egui-react/src/engine/lite.rs`, a single-line flexbox solver ported
from taffy 0.9 for the subset those two structs can express. A row using
anything outside the subset (`wrap`, `align_content`, grid, block, `baseline`,
`col_span` / `row_span`, an `auto` margin) falls back to the taffy path, row by
row, and says so once at `debug` level. Everything else is shared with the
taffy path: the measure function, the galley cache, `<Text>`'s widget rect and
selection, and the two rules from steps B and C (do not discard when nothing
moved; recompute before drawing when only the root rect resized). The other
layout paths are untouched.

Reproduce:

```sh
PERF_CSV="$PWD/docs/tasks/perf/samples-e1.csv" \
  cargo test --release -p list-10k --test scenarios -- --ignored --nocapture
```

| Scenario | Mode | Mean ms | p50 ms | p95 ms | Passes/frame | Discard frames /120 | Final rows | Row callbacks/frame |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Idle | All | 21.733 | 21.430 | 23.851 | 1.00 | 0 | 10000 | 10000.00 |
| Idle | Virtual | 0.132 | 0.134 | 0.153 | 1.00 | 0 | 37 | 37.00 |
| Idle | Plain | 0.114 | 0.113 | 0.132 | 1.00 | 0 | 37 | 37.00 |
| Scroll | All | 21.858 | 21.530 | 23.907 | 1.00 | 0 | 10000 | 10000.00 |
| Scroll | Virtual | 0.184 | 0.183 | 0.252 | 1.00 | 0 | 37 | 37.00 |
| Scroll | Plain | 0.141 | 0.135 | 0.177 | 1.00 | 0 | 37 | 37.00 |
| Filter | All | 42.202 | 9.117 | 163.934 | 1.60 | 72 | 1250–10000 | 7000.00 |
| Filter | Virtual | 1.067 | 1.046 | 1.363 | 1.00 | 0 | 37 | 37.00 |
| Filter | Plain | 1.029 | 0.998 | 1.306 | 1.00 | 0 | 37 | 37.00 |
| Resize | All | 33.977 | 33.726 | 35.833 | 1.00 | 0 | 10000 | 10000.00 |
| Resize | Virtual | 0.203 | 0.191 | 0.322 | 1.10 | 12 | 37–40 | 42.80 |
| Resize | Plain | 0.145 | 0.137 | 0.219 | 1.00 | 0 | 37–40 | 38.90 |

| Scenario | Virtual pass histogram (passes: frames) | Virtual discard requests |
|---|---|---:|
| Idle | 1: 120 | 0 |
| Scroll | 1: 120 | 0 |
| Filter | 1: 120 | 0 |
| Resize | 1: 108, 2: 12 | 12 |

The pass behaviour is exactly E's, which is what E1 had to leave alone: one
pass everywhere but the twelve Resize frames where the visible row count grows
and a slot draws its first widget leaf.

### Virtual against Plain

| Scenario | After E | After E1 | Second E1 run |
|---|---:|---:|---:|
| Idle | 1.27x | **1.15x** | **1.15x** |
| Scroll | 1.51x | 1.30x | 1.33x |
| Filter | 1.06x | **1.04x** | **1.04x** |
| Resize | 1.58x | 1.40x | 1.40x |

The second run of the same build reads Virtual 0.132 / 0.174 / 1.045 / 0.212 ms
against Plain 0.115 / 0.131 / 1.004 / 0.151 ms. Both runs agree to within
0.01 ms on every Virtual figure, so the ratios above are not a noise artefact
this time.

### Compared with E

| Scenario | E Virtual ms | E1 Virtual ms | Change |
|---|---:|---:|---:|
| Idle | 0.145 | 0.132 | −9% |
| Scroll | 0.219 | 0.184 | −16% |
| Filter | 1.093 | 1.067 | −2% |
| Resize | 0.240 | 0.203 | −15% |

Filter is dominated by rebuilding ten thousand strings, which neither path
touches, so its 2% is the layout share of that scenario and nothing more.

### The gate

| Gate | Target | Measured | |
|---|---|---|---|
| Idle | ≤ 1.20x Plain | 1.15x, 1.15x | pass |
| Scroll | ≤ 1.20x Plain | 1.30x, 1.33x | **miss** |
| Passes/frame | 1.00 / 1.00 / 1.00 / 1.10 | 1.00 / 1.00 / 1.00 / 1.10 | pass |
| Parity | every corpus rect equal | 18 row trees, every node | pass |
| Snapshots | byte-identical | 15 pass, 2 board missing as before | pass |

Scroll misses. As plan-e.md section 5 says for that case, what follows is one
profile of it and no further optimisation.

### Where the scrolled frame's 0.95 µs per row goes

Date: 2026-09-06. Instruments Time Profiler via `xctrace`, release build with
line tables and frame pointers, one mode per process, 120,000 scrolled frames
each (17,537 samples for Virtual, 13,307 for Plain). The scroll is a saw wave —
250 frames down, 250 up, 20 points a frame — so every frame's rows carry
different text without running off the end of the list. Samples are charged to
the deepest frame in the backtrace that belongs to a named bucket. The
benchmark was temporarily given a single-mode entry point for this and reverted;
nothing here is committed code.

Isolated, the two modes come to 146.1 µs (Virtual) against 110.9 µs (Plain) per
frame, a 1.32x ratio — the same ratio the benchmark reports, so the profile is
measuring the same gap.

| Bucket | Virtual µs/frame | Plain | Difference | Per row |
|---|---:|---:|---:|---:|
| lite solver (`LiteTree::compute` and everything under it) | 13.48 | 0 | +13.48 | +364 ns |
| lite tree building (`LiteCx::container` / `leaf` / `text`, `lite::show`) | 8.42 | 0 | +8.42 | +227 ns |
| egui-react component layer (`Cx::scope`, `rsx!`, `<Row>`, `<Text>`, `Store`) | 7.57 | 0 | +7.57 | +205 ns |
| taffy engine (the app root tree and the `<VirtualList>` leaf, not the rows) | 4.87 | 0 | +4.87 | +132 ns |
| text layout (galleys, `GalleyCache`, harfrust) | 19.72 | 15.72 | +3.99 | +108 ns |
| egui widgets and `Ui` (`create_widget`, `get_response`, `Ui::new_child`) | 67.97 | 64.78 | +3.18 | +86 ns |
| tessellation | 16.36 | 16.97 | −0.61 | −16 ns |
| egui's own row layout, paid by Plain (`horizontal`, `with_layout`, `push_id`) | 5.97 | 11.95 | −5.97 | −161 ns |
| epaint other, unattributed | 1.79 | 1.46 | +0.32 | +9 ns |
| **Total** | **146.1** | **110.9** | **+35.2** | **+953 ns** |

Findings:

- The solver is 38% of the gap, and it runs on a scrolled frame because every
  slot shows a different row's text: the `<Text>` job hash changes, so the
  frame's nodes differ from the last frame's and the layout is solved again.
  On an idle frame the node comparison finds them identical and no box is
  solved at all, which is why Idle is 1.15x and Scroll is 1.30x. The same
  cause was traced for the taffy path in "After E"; E1 made that computation
  about four times cheaper (E's scroll surcharge over idle was 0.074 ms for 37
  rows, E1's is 0.052 ms) but did not remove it.
- The next 205 ns a row is not layout at all. It is one `Cx::scope` per
  element, the `rsx!` closures, and the `<Row>` / `<Text>` component bodies —
  the same layer the step C attribution measured at 28.2 µs/frame for
  `Cx::scope` alone. Nothing in plan E touches it.
- 132 ns a row is charged to the taffy path, which still holds the app's root
  `<View>` and the `<VirtualList>` leaf itself. That is one tree for the whole
  list, not one per row; it is in the table because it is part of the 35.2 µs,
  not because it scales with rows.
- Plain is not free either: it pays 161 ns a row for `ui.horizontal`,
  `with_layout`, `allocate_ui_with_layout` and `push_id`, which is the closest
  thing it has to a layout step.

So a scrolled `<VirtualList>` row costs about 0.95 µs more than a plain egui
row, of which 0.59 µs is the lite path (solve plus build) and 0.36 µs is the
component layer, the app's own root tree and the extra galley work. Getting
Scroll under 1.2x means either not solving a row whose text changed but whose
boxes cannot move — the follow-up already listed under "What remains" — or
making the component layer cheaper, which is a different piece of work again.

## Summary D

Every number is already above; this is the whole sequence in one place.
`<VirtualList>` mean ms and passes per frame, one recorded run per step, all on
the same machine. Timings drift a few percent between runs and `Plain` drifts
with them, so passes per frame and the ratio are the stable signal.

| Scenario | Baseline | A | B | C | D1 | D2 | D2b | E | E1 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Idle | 0.225 / 1.00 | 0.222 / 1.00 | 0.225 / 1.00 | 0.241 / 1.00 | 0.160 / 1.00 | 0.153 / 1.00 | 0.152 / 1.00 | 0.145 / 1.00 | 0.132 / 1.00 |
| Scroll | 0.452 / 2.00 | 0.466 / 2.00 | 0.298 / 1.00 | 0.303 / 1.00 | 0.221 / 1.00 | 0.214 / 1.00 | 0.216 / 1.00 | 0.219 / 1.00 | 0.184 / 1.00 |
| Filter | 1.528 / 1.60 | 1.263 / 1.60 | 1.158 / 1.00 | 1.222 / 1.00 | 1.127 / 1.00 | 1.094 / 1.00 | 1.102 / 1.00 | 1.093 / 1.00 | 1.067 / 1.00 |
| Resize | 0.662 / 2.98 | 0.653 / 2.98 | 0.649 / 2.98 | 0.311 / 1.02 | 0.247 / 1.10 | 0.211 / 1.10 | 0.217 / 1.10 | 0.240 / 1.10 | 0.203 / 1.10 |

`Plain` on the same runs, mean ms (one pass per frame throughout):

| Scenario | Baseline | A | B | C | D1 | D2 | D2b | E | E1 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Idle | 0.124 | 0.114 | 0.113 | 0.123 | 0.112 | 0.116 | 0.118 | 0.114 | 0.114 |
| Scroll | 0.153 | 0.149 | 0.146 | 0.146 | 0.139 | 0.135 | 0.134 | 0.145 | 0.141 |
| Filter | 1.286 | 1.021 | 1.014 | 1.065 | 1.038 | 1.025 | 1.032 | 1.027 | 1.029 |
| Resize | 0.153 | 0.147 | 0.149 | 0.153 | 0.144 | 0.133 | 0.137 | 0.152 | 0.145 |

`<VirtualList>` / `Plain`, against task.md's 1.5x:

| Scenario | Baseline | A | B | C | D1 | D2 | D2b | E | E1 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Idle | 1.81x | 1.95x | 1.99x | 1.96x | 1.43x | **1.32x** | **1.29x** | **1.27x** | **1.15x** |
| Scroll | 2.95x | 3.13x | 2.04x | 2.08x | 1.59x | 1.59x | 1.61x | 1.51x | 1.30x |
| Filter | 1.19x | 1.24x | 1.14x | 1.15x | 1.09x | **1.07x** | **1.07x** | **1.06x** | **1.04x** |
| Resize | 4.33x | 4.44x | 4.36x | 2.03x | 1.72x | 1.59x | 1.58x | 1.58x | 1.40x |

The E column is one run, and a second run of the same build puts Scroll at
1.63x and Resize at 1.51x (see "After E"). E moved no timing; read it as a
third repeat measurement of D2 next to D2b.

Steps: A slot-keyed `<VirtualList>` row trees; B the egui_taffy fork skips the
discard when the layout did not move; C the same fork computes the layout
before drawing on a root resize; D1 the own engine over taffy, with B and C
built in and a container node as a rect instead of a `Ui`; D2 `<Text>` as a
galley on the node instead of a `Label` in a `Ui`; D2b text selection back on
that `<Text>`, which costs nothing measurable; E a fixed root rect for
`<VirtualList>` rows, which cost nothing and saved nothing but put the rows at
the pitch `show_rows` reserved; E1 lays those rows out with a flex solver of
egui-react's own instead of a taffy tree. D2, D2b and E differ by less than one
run of noise, so those three columns are repeat measurements of D2 as much as
steps of their own; E1 is the first move since D2 that is larger than the
noise on every scenario.

### What remains

After E1 all four scenarios are through task.md's 1.5x: Idle 1.15x, Scroll
1.30x, Filter 1.04x, Resize 1.40x. Scroll is the only one still over plan-E's
own 1.2x gate, and it was profiled once for that reason ("Where the scrolled
frame's 0.95 µs per row goes"). That profile is what the two candidates below
come from. Neither is done, and neither should start without a reason beyond
this benchmark.

- **Do not solve a row again when the text that changed cannot move any box.**
  A scrolled frame solves every row because each slot shows a different row's
  text, so the node vector differs from the last frame's — 0.36 µs a row, 38%
  of the whole gap. In the list-10k row the changed text sits in a node with
  `grow`, whose box is decided by the container and not by what it measures, so
  the solve produces the boxes the row already had. Skipping it needs a rule
  for which nodes a measurement can move, and getting that rule wrong leaves a
  wrong picture on screen, so it is a bigger change than E1 was.
- **Make the component layer cheaper.** The next 0.21 µs a row is not layout at
  all: one `Cx::scope` per element, the `rsx!` closures and the `<Row>` /
  `<Text>` bodies. The step C attribution measured `Cx::scope` alone at
  28.2 µs/frame. Nothing in plan D or E touched it, and it is paid by every
  element in every app, not only by list rows.

Still open from D1, unchanged: **keep a swept tree for a grace period** instead
of dropping it in the pass it was not drawn. That is the Resize 1.10 passes per
frame — the visible row count grows and shrinks four times and the three extra
slot trees are rebuilt each cycle. It was left out because a time-to-live is a
knob and picking its value from this benchmark would be tuning to the benchmark.

- ~~Give `<VirtualList>` rows a fixed rect~~ — that was step E. It fixed the row
  pitch and cost nothing, but the scroll time did not move, because the root
  rect was not the trigger.
- ~~Lay the rows out without a taffy tree~~ — that was step E1.

All of that is layout work, not measurement work. The web and 120 Hz criteria in
task.md are still unmeasured and are a separate task: everything here is native,
and nothing recorded so far says what a browser at 120 Hz does.

## Web (after E1)

Date: 2026-09-06. Chrome 152 on macOS, `trunk build --release` (wasm-opt `z`),
WebGPU, canvas 800×900 CSS px at device pixel ratio 2. Served with COOP/COEP
so `performance.now()` is fine-grained. Measured by wrapping
`requestAnimationFrame` and timing eframe's callback: input handling, the
egui pass, tessellation and the WebGPU submit. GPU time and compositing are
not included. Chrome ran `requestAnimationFrame` at 60 Hz on this display, so
the 120 Hz criterion could only be judged against its 8.3 ms budget.

Input is synthetic `WheelEvent`s dispatched to the canvas at 120 Hz for 4 s
(about 246 frames per run): `deltaY = 0` for "repaint" (a frame that draws
with nothing changed; egui web does not repaint at all without input, so this
is the idle cost of a frame that has to draw) and `deltaY = 20` for scroll.
Each mode was measured alone in the foreground tab, twice. The plain list is
`list-10k-plain` built for the web with `index-plain.html`.

| Mode | Repaint median ms | Repaint p95 | Scroll median ms | Scroll p95 | PERF WARNING |
|---|---:|---:|---:|---:|---:|
| egui-react `<VirtualList>`, 10,000 rows | 1.00 / 1.02 | 1.23 / 1.39 | 0.98 / 0.99 | 1.25 / 1.30 | 0 |
| plain egui `show_rows`, 10,000 rows | 0.80 / 0.80 | 0.98 / 1.04 | 0.79 / 0.81 | 0.97 / 1.06 | 0 |
| egui-react `<ScrollArea>` + `for`, 10,000 rows | 66.8 | 69.8 | 67.5 | 68.8 | 0 |

VirtualList / plain: 1.24x on repaint, 1.23x on scroll. Both are well inside
8.3 ms. No `PERF WARNING` was logged over about 1,500 scrolled frames in any
run. The shared floor of about 0.7 ms per frame is eframe's web frame
(input, texture upload, WebGPU submit); the layout layer's share is the
0.2 ms difference, against 0.04 ms on native, so wasm runs that part about
5x slower than native.

The gallery is not usable for this comparison: with the source panel open it
costs 10 to 11 ms per frame whatever example is shown, and the list-10k
column is 200 px wide.

task.md's two web criteria are now measured: no PERF WARNING while scrolling,
and a drawn frame at 1.0 ms against the 8.3 ms budget.
