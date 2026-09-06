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
