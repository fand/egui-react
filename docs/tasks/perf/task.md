# Task: perf (layout frame cost, schedule TBD)

## Objective

Bring react-egui's per-frame cost closer to plain egui. The same UI currently costs more in react-egui than in plain egui; on the web (120 Hz), it exceeds the 8.3 ms frame budget and drops to 16.7 ms. The causes are the layout layer's (egui_taffy) two-pass approach and fixed per-element overhead.

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

## Causes (from the egui_taffy 0.14 source)

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
