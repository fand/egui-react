# list-perf: what we learned, what is left

Closing notes for PR #7 (`docs-perf`). The full record is in
[task.md](task.md) (criteria and result), [measurements.md](measurements.md)
(every number), the plans (`plan.md`, `plan-d.md`, `plan-e.md`), and
[progress.md](progress.md) (steps, commits, how it is built, reproduction).
This file is the short version.

## Result

`<VirtualList>` against plain egui `show_rows`, native, 37 rows in view:
Idle 1.81x → 1.16x, Scroll 2.95x → 1.36x, Filter 1.19x → 1.04x, Resize
4.33x → 1.29x, one pass per frame everywhere. Web: about 1 ms per drawn frame
at 10k and 100k rows. egui_taffy is gone; the layout engine is our own, over
taffy, with a single-line flex solver for list rows.

## Lessons

- **Measure the matched pair, count passes.** The one number that moved
  every decision was passes per frame, not milliseconds. Timings drift by a
  few percent between runs; a discard count does not. The benchmark draws the
  same 37 rows three ways in one fixture, so a ratio means something.
- **Multi-pass is the cost model.** Every 2x we found was egui running the
  frame twice. The engine may ask for a second pass only when the frame on
  screen is wrong: a widget that drew in a sizing pass, a node that went away,
  a node whose *paint* moved. What "moved" means depends on the node: a widget
  leaf on any field, a `<Text>` on its paint anchor (not its size), a container
  on its location (not its size). Getting that rule right removed more passes
  than any optimisation of the layout itself.
- **Name the reason.** `request_discard("layout changed")` told us nothing for
  a week. `"the text "last frame 9.8 ms…" moved [..] -> [..]"` in the PERF
  WARNING overlay and in the debug log found the last two causes in minutes.
  Diagnostics that name the node are worth the format call.
- **Fixtures hide what they normalise.** The benchmark zeroed `item_spacing`,
  scrolled whole rows, and drew no frame-time label. Real trackpads scroll by
  fractions of a row, real headers have labels that change every frame, and
  real `Ui`s have egui's default spacing. Each of those was a bug the fixture
  could not see (F, G, the row pitch). Keep one path that runs the real app
  with real input; the web build in Chrome with synthetic wheel events served
  as that here.
- **Layout in one frame needs sizes in one frame.** A widget leaf draws before
  it can be measured, so its first frame is a sizing pass; `<Text>` is measured
  inside the layout function, so a tree of `<View>` and `<Text>` settles in one
  pass from its first frame. The more a tree can be measured without drawing,
  the fewer passes it costs. Containers that paint nothing are free.
- **Row trees are a slot, not a row.** Keying a `<VirtualList>` row's layout by
  its slot (RecyclerView style) while keying its hooks by the row index gave
  reuse without breaking state. Keeping a slot's tree alive for a grace period
  after it stops being drawn is what made fractional scrolling one pass.
- **The remaining gap is the layer, not the algorithm.** At 1.36x on scroll,
  about half the difference is the lite solver re-solving a row whose boxes
  cannot move, and half is the component layer (scope lookup, props, emitters)
  — around 200 ns per row. Neither is taffy.
- **A comparison switch in the app pays for itself.** Putting the plain version
  behind a checkbox in the same window found the row-pitch difference on the
  first look. A benchmark table never would have.

## Remaining work

- **VirtualList redesign.** Today it is react-window's `FixedSizeList` with
  slot-recycled layout trees. Wanted, drawing on react-virtualized and
  TanStack Virtual: variable row heights (measure once, cache by key,
  estimate the rest, prefix sums, binary search for the range; egui's
  multi-pass can absorb the correction frame), `overscan`, `scroll_to(index)`
  / initial offset, an "items rendered" callback, horizontal lists and grids.
  This is a separate task; the current element stays as it is.
- **Scroll at 1.36x** (plan-E gate was 1.2x). Skip the row solve when no box
  can move (fixed-width and `grow` columns; only the text changed). Cheaper
  component layer per row: `Cx::scope`, props construction, event emitters.
- **Filter at 100k rows** costs about 22 ms per keystroke in both versions:
  `rows()` rebuilds 100,000 `String`s. Build names once and filter to a
  `Vec<usize>`, or narrow the previous result when the filter only grew.
- **Verify on a real trackpad.** Everything after F was checked with synthetic
  input (kittest and Chrome wheel events). If a PERF WARNING still appears
  while scrolling natively, the overlay now names the node; run with
  `RUST_LOG=egui_reactor=debug` to log each discard with old and new rect.
- **Gallery source panel** costs 10 to 11 ms per frame regardless of the
  example shown: it draws the whole highlighted file every frame. Needs a
  row-virtualised source view. Also seen: the panel overlaps the example
  column at 800 px, the showcase heading renders garbled glyphs.
- **Upstream B and C** (skip the discard when nothing moved; layout before
  drawing on a root-only resize) from the `../egui_taffy` fork branches
  `skip-unchanged-discard` and `layout-first`. Nothing here depends on them;
  file them or drop the fork.
- **Housekeeping.** `cargo fmt --all -- --check` fails on ~30 files (import
  order, pre-existing); the two board gallery snapshots were never generated.
