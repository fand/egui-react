# Plan E: VirtualList rows without taffy (same API)

Follows [plan-d.md](plan-d.md). Numbers are in [measurements.md](measurements.md)
("After E", "What remains").

## 0. Why

After D and the fixed row rect, a `<VirtualList>` row still costs about
0.9 µs more than the plain egui row at idle, and on a scrolled frame every
row tree recomputes because its texts changed. Neither cost is taffy's
algorithm. Both are the price of holding a general retained tree per row:

- `Store` lookup and `Rc<RefCell>` borrow per tree;
- four `HashMap` lookups keyed by `egui::Id`, a `taffy::Style` built and
  compared per node (`ItemStyle::to_taffy`, ~300 bytes);
- a `Vec<(NodeId, Layout)>` snapshot for the moved check on every recompute;
- taffy's `compute_layout_with_measure` with its measure callbacks.

A row is a single-line flex box of two or three children with a fixed
height. That needs none of the above. This step lays out such rows with a
small solver of our own over a `Vec` of nodes rebuilt every frame, and keeps
the current API: `render` still draws `<View>` / `<Text>` / `<Button>`, and a
row that uses something the solver does not cover falls back to the taffy
path, row by row.

Target: `<VirtualList>` within 1.2x plain egui on Idle and Scroll, all tests
and snapshots unchanged.

## 1. Where the lite path applies

A tree opened with `Cx::with_root_size` (today: only `<VirtualList>` rows)
takes the lite path when every node in it is inside the supported subset
(section 3). Anything else, and any tree not opened with a fixed root size,
keeps the taffy path unchanged. The decision is made per row while the row is
collected: the first unsupported style seen switches that slot to taffy for
good (a flag on the slot; the slot is drawn again through taffy in the same
frame's next pass, one discard, once).

Extending the lite path to every fixed-size tree, or to all trees, is a later
decision with its own measurement.

## 2. Data and frame protocol

Per slot (kept in the `Store` under the same key as today's tree, replacing
it):

```rust
struct LiteTree {
    nodes: Vec<LiteNode>,   // rebuilt each frame in draw order; allocation reused
    rects: Vec<Rect>,       // last computed rect per node index, in screen space
    sizes: Vec<Vec2>,       // last measured size per widget leaf index
    fallback: bool,         // this slot uses taffy
}
```

**Deviation E1 made here.** `rects` holds each node's rect relative to the
corner of the rect the row was given, not in screen space. A scrolled row is
the same layout at another place, so screen rects would differ on every
scrolled frame and the "did anything move?" comparison would ask for a discard
every frame. The screen position is added back when a leaf draws and when a
text paints.

```rust
struct LiteNode {
    parent: Option<usize>,
    kind: Kind,             // Container(ContainerStyle) | Text(galley, job hash) | Leaf
    item: ItemStyle,        // by value; Copy
}
```

Frame of one row (same shape as the engine's today, so `Cx` semantics do not
change):

1. `render` runs with a `Cx` in lite mode. `container` pushes a node and
   recurses; `text` pushes a node and lays the galley out now (galley cache
   keyed by job hash + wrap width, as in D2); `leaf` pushes a node, opens one
   child `Ui` at `rects[i]` from the last frame (zero rect + invisible sizing
   pass if this index has no rect yet), draws, and records `ui.min_size()`
   into `sizes[i]`. Texts paint through the same deferred `Shape::Noop` slot
   or `LabelSelectionState` path as D2b, using `rects[i]` once solved.
2. Solve (section 3) into `new_rects`.
3. If any node's rect changed, or a node index is new, or the node count
   changed: request a discard (same created / removed / moved rule). Swap
   `rects`.
4. `ui.allocate_space(root size)` as today.

No `HashMap`, no `taffy::Style`, no per-node `Id`. Node identity is the
index in draw order, which is stable for a row template; a row whose shape
differs from the previous occupant of the slot is a "new node" case and
costs one discard, like today.

## 3. The solver

Single-line CSS flexbox, the parts `ItemStyle` and `ContainerStyle` can
express, resolved against a definite container box (the row rect, or the
parent node's content box for nested `<View>`s):

- `direction` row / column / reverse; `justify` all values; `align` and
  `align_self` start / end / center / stretch (`normal` = stretch, as in
  CSS and taffy); `gap` both axes.
- Item: `w h min_w min_h max_w max_h` in px, percent of the container's
  content box, or auto; `grow shrink basis`; `m*` and `p*` in px or percent
  (percent margins resolve against the container's inline size, as taffy
  does); `display: none` on an item.
- Auto main size of an item = its content: a `<Text>` galley, a widget's
  last measured size, or a nested container's own solved content size. Auto
  cross size = stretch when align is stretch and the item has no definite
  cross size; else content.
- Rounding: taffy rounds after layout (`TaffyTree` rounding is on by
  default): absolute positions are rounded to whole points, sizes are the
  difference of rounded edges. Do the same so rects match taffy exactly.

Not supported (fall back to taffy): `display` grid or block, `wrap`,
`align_content`, `align: baseline`, `col_span` / `row_span`, an `auto` margin.
`<View>` nesting is supported at any depth as long as every level is in the
subset.

**Deviation E1 made here.** A percentage `basis` against a container whose main
size is not definite is *in* the subset, not a fallback as this section first
had it. taffy resolves it to "measure the content", which is one line in the
solver, and the parity corpus has a case for it
(`percent_basis_without_a_main_size`). What did move to the fallback list
instead is an `auto` margin, which absorbs free space and would need a second
distribution step.

The solver is one file, `crates/egui-reactor/src/engine/lite.rs`, with the flex
algorithm spelled out in comments step by step (the CSS spec's 9.2 to 9.7 in
order: basis, hypothetical size, free space, grow/shrink with min/max clamping
and re-freezing, main positions with justify, cross size and align).

**The size estimate here was wrong.** "About 250 lines" became about 1,650
lines of code (2,169 with the comments). Four things the estimate left out, all
of them needed to match taffy rect for rect: measuring a node's min-content and
max-content size, which is a second pass of the same algorithm; taffy's
rounding, which is cumulative from the root, so a nested container's children
round against the already rounded parent; hypothetical sizes, kept per item
through the whole grow/shrink step; and the re-freezing loop, which redoes the
distribution every time an item hits a min or a max. None of it is optional —
the parity test compares exact rects — and none of it is expensive at run time:
E1's solver is 0.36 µs a scrolled row.

## 4. Parity with taffy

The lite path must produce the same rects taffy does for the subset. That is
tested directly, not by snapshot alone:

- `crates/egui-reactor/tests/lite_parity.rs`: a corpus of row trees (the
  list-10k `Row`, rows with margins and padding, nested `<View>`s, percent
  widths, `justify` variants, `shrink` with a too-narrow container, a
  `display: none` child, min/max clamping) each drawn twice, once through
  the lite path and once through taffy (force the fallback flag), comparing
  every node's rect with exact equality. The test runs each tree for three
  frames so measured leaves settle.
- Existing `virtual_list.rs` tests, the gallery snapshots (byte-identical),
  and `list_10k` tests.
- A test that a row using `wrap` falls back to taffy and still lays out.

If a corpus case disagrees, the lite solver is wrong (taffy is the
reference), unless the difference is a taffy quirk we do not want to copy,
in which case the case is listed here with the reason and the taffy result.

## 5. Steps and gates

| Step | Content | Gate |
|---|---|---|
| E1 | `lite.rs` solver + `LiteTree`, `Cx` lite mode behind `with_root_size`, fallback flag, parity test corpus, texts and widgets through it. | Parity test green on the whole corpus; all tests; snapshots byte-identical; benchmark "After E1": Idle and Scroll VirtualList at or below 1.2x Plain, passes unchanged (1.00 / 1.00 / 1.00 / 1.10). |
| E2 | Docs: ARCHITECTURE 6 (lite path and its subset), VirtualList doc comment (what falls back), measurements Summary and What remains, progress. | — |

**E1 is done** (`65d0274`), and the gate is partly met. Idle 1.15x passes,
Filter 1.04x and Resize 1.40x came along, the passes per frame are
1.00 / 1.00 / 1.00 / 1.10 as asked, the parity corpus is equal on all 18 trees,
and the snapshots are byte-identical. **Scroll misses at 1.30x** (1.33x on a
second run). Per the rule below that bought exactly one profile and no further
optimisation; the breakdown is in measurements.md, "Where the scrolled frame's
0.95 µs per row goes".

If E1 lands at 1.2x to 1.3x, profile once more before touching anything else
and write the breakdown into measurements.md; do not add a third layout path.

## 6. Risks

- **Two layout implementations to keep in step.** Mitigated by the parity
  corpus; every future `ItemStyle` / `ContainerStyle` addition must add a
  corpus case or be listed as fallback.
- **Rounding.** taffy's rounding is cumulative from the root; a nested
  container's children round against the rounded parent. Copy that order.
- **Widget leaves still need a sizing pass on their first frame in a slot**
  (one discard per slot, as today). Not changed by this step.
- **Node identity by index.** A row whose children are conditional (`if`)
  changes shape between occupants; that is the "new node" discard, once per
  shape change. Rows that alternate shapes every frame would thrash; the
  VirtualList docs say rows must be uniform, so this is documented, not
  solved.

## 7. Open questions (answered 2026-09-06: 1 rows only; 2 debug log once per slot)

1. Scope: VirtualList rows only (this plan), or every tree opened with a
   fixed root size? Default: rows only; widen after E1's numbers.
2. Fallback: silent (default), or a debug-build `log::debug!` naming the
   style that forced taffy so a row author can see why the row is slower?
   Default: log once per slot at `debug` level.
