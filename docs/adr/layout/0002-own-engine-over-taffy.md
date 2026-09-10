# 0002: Our own layout engine over taffy, replacing egui_taffy

Date: 2026-09-06 · Status: accepted

## Context

[0001](0001-taffy-over-egui-flex.md) chose taffy. Reaching it was a separate, unrecorded decision: layout went through `egui_taffy` 0.14, called from one file (`cx.rs`). This ADR replaces that wrapper, not the choice of taffy, so it supersedes nothing.

Measured, in `docs/tasks/list-perf/`. The benchmark is a list of 37 visible rows drawn three ways at 10,000 source rows: `<VirtualList>`, `<ScrollArea>` + `for` over every row, and plain egui `ScrollArea::show_rows` as the reference. Four scenarios: idle, scrolling, filter edits, window resize.

The starting point was `<VirtualList>` at about 2x plain egui, with extra layout passes on three of the four scenarios. Three steps fixed the passes:

- **A** keys a `<VirtualList>` row's tree by the slot on screen rather than by the row index. Passes did not move, but egui memory stopped growing with the scroll distance (85 entries against 771).
- **B**, in an egui_taffy fork, asks for a discard only when the layout actually moved. Scroll 2.00 → 1.00 and Filter 1.60 → 1.00 passes per frame, 6,576 discard requests → 0.
- **C**, in the same fork, computes the layout before drawing when only the root rect resized. Resize 2.98 → 1.02 passes per frame, and the all-rows mode 131 → 54 ms.

That left every scenario at one pass and the gap still at about 2x. A Time Profiler run said why: taffy's own algorithm costs nothing at idle (no sample lands in a `taffy::` frame) and egui_taffy's per-tree bookkeeping is 9%. About 80% is egui_taffy building one egui `Ui` per taffy node and a second one per leaf — **nine `Ui`s per row against four in plain egui**, a ratio of 2.25 against the measured 1.94x. That is egui_taffy's design, not a bug: its backgrounds, interactive containers and sticky scrolling need those `Ui`s. egui-react uses none of them, and egui_taffy was called from one file (`cx.rs`).

## Decision

egui_taffy was replaced by `crates/egui-react/src/engine.rs` (ARCHITECTURE section 6), which ports egui_taffy's measure function, node reuse rule and sweep, builds B and C in, and makes a container node a rect instead of a `Ui`:

- **D1**, the engine: a row costs three `Ui`s instead of nine. Idle 0.241 → 0.160 ms, 1.96x → 1.43x of plain egui.
- **D2**, `<Text>` as a galley on the node: a row costs two. Idle 0.153 ms, 1.32x; Filter 1.07x; Scroll and Resize 1.59x, a tenth over the 1.5x the task asked for. All-rows idle 43 → 22 ms.
- **D2b**, text selection back on the engine's `<Text>`: it follows `interaction.selectable_labels` as `Label` does, and is not selectable on the one frame it is created (ARCHITECTURE section 6).

Two more steps took the `<VirtualList>` row itself:

- **E** gives a row a fixed root rect (`Cx::with_root_size`): the row is laid out into `width × row_h` and reserves exactly that. It changed no timing — the scrolled-frame recompute comes from the row's text, not from its root rect, which a trace showed was already constant — but the rows now sit at the pitch `show_rows` reserved (list-10k: 20 apart, was 18).
- **E1** lays those rows out with a solver of our own instead of a taffy tree (the lite path, ARCHITECTURE section 6). Idle **1.15x** plain egui, Scroll 1.30x, Filter **1.04x**, Resize 1.40x, on two runs that agree to within 0.01 ms. Passes per frame unchanged; the parity corpus and the snapshots agree with the taffy path node for node.

## Rejected

- **Staying on egui_taffy and optimising inside it.** A Time Profiler run said the cost was neither taffy's algorithm nor a bug: about 80% was egui_taffy building one egui `Ui` per taffy node and a second per leaf, which its backgrounds, interactive containers and sticky scrolling need. egui-react uses none of those, so there was nothing to optimise away without changing its design.
- **One layout implementation instead of two.** See "Why a second layout implementation was accepted" below; the price is paid by the parity corpus.

## Consequences

Snapshots stayed byte-identical through all three steps. What was given up: the engine drops a tree nothing drew in the pass, which costs 12 two-pass frames out of 120 in the resize scenario where egui_taffy's for-ever cache cost 3. It is recorded in `docs/tasks/list-perf/measurements.md` with the follow-ups.

**Why a second layout implementation was accepted.** The profiles say the cost was never taffy's algorithm — no sample lands in a `taffy::` frame at idle — but the retained per-row tree around it: a `HashMap` keyed by `egui::Id`, a `taffy::Style` rebuilt and compared per node, and a layout snapshot for the moved check, all for a single-line flex box of three children. A row does not need any of that, and nothing else in the library changes: the API, the elements and the taffy path are as they were, and a row using anything outside the solver's subset falls back to taffy on its own. The price is two implementations to keep in step, paid by the parity corpus (`crates/egui-react/tests/lite_parity.rs`), which is why every new layout attribute must get a case there or be added to the fallback list.

E1's gate asked for Scroll at or below 1.2x as well, and it landed at 1.30x. Per the plan that bought one profile and no more optimisation: of the 0.95 µs a scrolled row costs over a plain egui row, 0.59 µs is the lite path (0.36 solving, 0.23 building the node vector), and the other 0.36 µs is the component layer (`Cx::scope`, `rsx!`, the component bodies), the app's own root tree and the extra galley work. The solver runs on a scrolled frame at all because every slot shows a different row's text, so the node vector differs from the last frame's. The follow-ups are in measurements.md, "What remains".

B and C were also prototyped as egui_taffy fork commits and are worth upstreaming on their own: `../egui_taffy` (sibling checkout, not pushed), branches `skip-unchanged-discard` (`d618550`) and `layout-first` (`ee07d38`, on top of it). They are kept for that, and nothing in this repository depends on them.

## Links

- [ARCHITECTURE section 6](../../ARCHITECTURE.md#6-layout)
- [ARCHITECTURE section 6, the lite path](../../ARCHITECTURE.md#the-lite-path-a-virtuallist-row-without-a-taffy-tree)
- `docs/tasks/list-perf/measurements.md`
