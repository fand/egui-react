# Task: perf (layout frame cost, schedule TBD)

## Result (2026-09-06)

Done in six steps: A (slot-keyed `<VirtualList>` row trees), B and C (an
egui_taffy fork), D1 and D2 (an own layout engine over taffy, replacing
egui_taffy), D3 (docs). Every number below is the native benchmark in
[measurements.md](measurements.md), "After D2"; "Summary D" there has the whole
sequence and what remains.

| Completion criterion | Status |
|---|---|
| `<VirtualList>` at most 1.5x plain egui | **Partly.** Idle 1.32x pass, Filter 1.07x pass. Scroll 1.59x and Resize 1.59x miss by 0.09x |
| No PERF WARNING scrolling the web gallery at 120 Hz | **Unmeasured.** Native scrolling is one pass per frame with zero discard requests over 120 frames, which is what produced the warning; the browser was never measured |
| Idle frame time within 8.3 ms on the web | **Unmeasured.** Native idle is 0.153 ms for `<VirtualList>`. The 16.7 ms in the symptoms below was `stable_dt`, a frame interval, not CPU time |
| All existing tests and snapshots pass | **Pass.** The 13 gallery snapshots that passed before are byte-identical after D1 and after D2 |

The two scenarios that miss, and the two candidate follow-ups for them, are in
measurements.md under "Summary D". Neither was done.

## Objective

Bring react-egui's per-frame cost closer to plain egui while preserving the React-like component API and declarative row layout. Investigate layout passes and per-element overhead through measurement.

## Current measurements

See [measurements.md](measurements.md) for the matched native CPU benchmark and
[samples.csv](samples.csv) for frame/pass data. These measurements supersede the
unverified causal claims below: idle VirtualList uses one pass in this fixture;
scrolling and resizing add passes. The example's `stable_dt` display measures a
frame interval or prediction, not CPU rendering time, so 16.7 ms alone does not
prove that an 8.3 ms CPU budget was exceeded. Browser performance remains unmeasured.

The earlier observations and hypotheses below are retained as investigation context.
Direct egui row layout is not the intended production solution.

## Symptoms (observed in list-10k from examples PR B, 2026-09)

- Scrolling `<VirtualList>` on the web produces `egui PERF WARNING: request_discard has been called N frames in a row` on every frame while scrolling. The FPS display also fluctuates.
- Even when idle, react-egui takes 16.7 ms (equivalent to 60 Hz), versus 8.3 ms (120 Hz) for plain egui. react-egui is also slower on desktop.
- CPU measurements with kittest (`examples/list-10k/tests/bench.rs`, release, 600×800, 20 frames):

| rows | `<ScrollArea>` + `for` | `<VirtualList>` | Plain egui `show_rows` |
|---|---|---|---|
| 100 | 0.88 ms | 0.36 ms | 0.16 ms |
| 1,000 | 5.06 ms | 0.28 ms | 0.13 ms |
| 10,000 | 78.04 ms | 0.27 ms | 0.17 ms |

`<VirtualList>` removes the dependency on row count, but even a dozen or so visible rows cost twice as much as plain egui.

## Causes (from the egui_taffy 0.14 source; superseded, kept as context)

Written before any measurement, against a dependency that is gone: egui_taffy
was replaced by `crates/egui-react/src/engine.rs` in step D. Read with
measurements.md next to it. Where each item stands: (1) and (2) were real and
are fixed (steps B, C and A; the created / removed / moved rule is now in
ARCHITECTURE 5.3); (3) was the largest cost and was the reason for step D,
though the profile named per-node `Ui` construction rather than hashing or
store lookups; (4) never happened — idle was one pass per frame from the first
recording on.

1. **Two-pass layout.** egui_taffy draws children to measure their sizes, computes the taffy layout, then uses `request_discard` to draw the same frame again if the result differs from the previous one (`egui_taffy/src/lib.rs`, line 632: recompute when `taffy.dirty(node) || state.last_size != root_rect.size()`; line 712: `request_discard`). New nodes, changes in measured sizes, and changes in the root size mark the layout dirty. Plain egui uses one pass.
2. **New trees always trigger a discard.** With no measurements available, the first pass is always dirty. Each `<VirtualList>` row creates a small taffy tree through `<View>`, so scrolling creates new trees as rows are replaced, triggering a discard every frame. This directly causes the PERF WARNING.
3. **Fixed per-element overhead.** Each element requires one taffy node, a child `Ui` from `Ui::push_id`, Id hashing, and a store lookup. The cost scales with node count and is 2–3 times higher on wasm than on native.
4. **Dirty layout while idle (unconfirmed).** If floating-point fluctuations change measured sizes every frame, even an idle UI takes two passes per frame. Confirm this through measurement.

Related known behavior (examples plan.md, sections 7–8): taffy leaves return their previously drawn size as both the min-content and max-content sizes. On the first pass, they are drawn in a `Ui` with zero width. `grow` distributes extra space; it does not specify a size.

## Scope

### Included

- **Measurement** (do this first; record results here)
  - Add temporary instrumentation to the runner to track `ctx.will_discard()` and count passes per frame on native and web (a debug flag in `Options` is sufficient).
  - Measure pass counts and frame times while idle and scrolling in the gallery's list-10k (`for` / `VirtualList` / plain), showcase, and counter examples. Use Chrome's Performance panel for the web.
  - Determine whether discards occur while idle (cause 4).
- **egui_taffy improvements** (prototype in a fork, then submit an upstream PR)
  - Measure without drawing: text leaves can obtain their sizes through `ui.fonts(|f| f.layout(..))`. Premeasure leaves that support it so their layout settles in one pass from the start.
  - Settle new trees whose parent supplies their size (`leaf_fill`, `VirtualList` rows) immediately, without discarding.
  - Round measurements to stabilize dirty checks (if cause 4 is confirmed).
- **react-egui improvements**
  - Provide a way for `VirtualList` rows to avoid creating taffy trees: a `Row` element that uses egui's `horizontal` only inside each row, or reuse rows as child nodes of a single taffy tree.
  - Profile and reduce the fixed overhead in `Cx::leaf` / `container` (child `Ui` creation, Id hashing).
  - Revisit the runner's default `max_passes`.
- Preserve measurements in the same format as `examples/list-10k/tests/bench.rs` and compare results before and after improvements.

### Excluded

- Changes to egui itself.
- Replacing the layout engine (e.g. egui_flex). Already rejected in ARCHITECTURE section 6.
- wgpu / rendering optimizations (egui handles rendering).

## Deliverables

- Measurement results (add tables to this document).
- An egui_taffy PR (or the fork diff and an explanation of why it cannot be submitted upstream).
- react-egui changes with before/after benchmarks.
- Updates to ARCHITECTURE.md sections 5.3 (multiple passes) and 6.

## Completion criteria

- In the web gallery (120 Hz), scrolling list-10k's `VirtualList` produces no PERF WARNING, and idle frame time stays within 8.3 ms.
- The `<VirtualList>` column in the table above is no more than 1.5 times the plain egui cost.
- All existing tests and snapshots pass (document the reason in the plan if snapshots change).

## Decisions (initial assumptions)

- Measure first, then improve egui_taffy, then react-egui. Do not change react-egui before confirming the causes.
- Write egui_taffy changes with upstream submission in mind. Do not add a fork as a dependency on react-egui's main branch.
- Schedule this around phase 8 (release preparation), after examples PR C (wgpu).
