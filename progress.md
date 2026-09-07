# Progress and handoff

## Performance work (2026-09-06)

PR: https://github.com/fand/egui-react/pull/7, branch `docs-perf`, base `main`.
24 commits on top of main, none pushed yet. Plans: `docs/tasks/perf/plan.md`
(A to C), `plan-d.md` (D), `plan-e.md` (E). Every number is in
`docs/tasks/perf/measurements.md`; the task's result table is at the top of
`docs/tasks/perf/task.md`.

### What was done, in order

| Step | Commit | Change | Effect |
|---|---|---|---|
| Benchmark | `2c1a3ac` | `examples/list-10k/tests/scenarios.rs`: Idle / Scroll / Filter / Resize × all rows / VirtualList / plain, CPU time and passes per frame | Baseline: VirtualList 1.8x to 4.3x plain, extra passes on three scenarios |
| A | `798f327` | VirtualList row trees keyed by slot (`Cx::with_layout_id`) | No tree per scrolled row; egui memory stops growing. Passes unchanged |
| B | `20a22c5` | egui_taffy fork: no discard when the layout did not move | Scroll and Filter 1 pass/frame |
| C | `c1c417e` | egui_taffy fork: compute before drawing when only the root resized | Resize 2.98 to 1.02 passes/frame |
| D1 | `5a9f04a` | Own engine over taffy (`crates/egui-react/src/engine/mod.rs`); egui_taffy and the `[patch]` removed. A node is a rect; one `Ui` per widget leaf | Idle 1.96x to 1.43x |
| D2 | `bf8d2f2` | `<Text>` painted as a galley, no `Ui` (`Cx::text`) | Idle 1.32x |
| D2b | `dfc5dfc` | Text selection restored through `LabelSelectionState`, `selectable` prop | No measurable cost |
| E | `fcb8697` | VirtualList rows get a fixed root rect (`Cx::with_root_size`) | Row pitch bug fixed (rows were 18 pt apart under a 20 pt reservation). Scroll unchanged |
| E1 | `65d0274` | VirtualList rows laid out by a single-line flex solver (`engine/lite.rs`), taffy as per-row fallback; parity test over 18 row trees | Idle 1.15x, Scroll 1.30x |
| Web | `57d4f8b` | Chrome measurement; `list-10k-plain` builds for the web | 1.0 ms vs 0.8 ms per drawn frame, 0 PERF WARNING |
| `43d1cc4` | | list-10k starts with virtualise on | Gallery snapshot updated |
| F | `4be1a36` | Trees survive 120 passes without being drawn (`TREE_GRACE_PASSES`) | Real trackpad scrolling no longer discards every other frame; Resize back to 1.02 |

### Results

Native, VirtualList mean ms per frame (ratio to plain), 37 visible rows of 10,000:

| Scenario | Baseline | Now (after F) | Passes/frame now |
|---|---:|---:|---:|
| Idle | 0.225 (1.81x) | 0.140 (1.18x) | 1.00 |
| Scroll | 0.452 (2.95x) | 0.197 (1.36x) | 1.00 |
| Filter | 1.528 (1.19x) | 1.098 (1.04x) | 1.00 |
| Resize | 0.662 (4.33x) | 0.205 (1.34x) | 1.02 |

All-rows mode (10,000 `<Row>`s through taffy): 44 ms to 22 ms at idle.
Web (Chrome, wasm release, 800×900): VirtualList 1.0 ms per drawn frame,
plain 0.8 ms (1.24x), all rows 67 ms, zero `PERF WARNING` over synthetic
whole-row, fractional and momentum-style scrolling. Chrome ran at 60 Hz on
the test display, so 120 Hz was not exercised. Gallery snapshots stayed
byte-identical through every step except the deliberate list-10k default
change. Ratios move by a few percent between runs; passes and discard counts
are the stable signal.

task.md criteria: 1.5x on all four scenarios met; no PERF WARNING met; web
frame within 8.3 ms met. Plan E's own tighter 1.2x gate is missed on Scroll
(1.36x), see "What remains".

### How it is built now

- `engine/mod.rs`: one taffy tree per `<View>` root, kept in the `Store`.
  Containers are rects; only widget leaves get a `Ui`; `<Text>` is measured
  inside taffy's measure function and painted as a galley. Discard only when
  a node was created (drew in a sizing pass), removed, or moved; layout is
  computed before drawing when only the root rect resized. Trees not drawn
  for 120 passes are swept.
- `engine/lite.rs`: rows opened with `Cx::with_root_size` (VirtualList) use a
  single-line flexbox solver over a `Vec` of nodes rebuilt per frame; no
  `HashMap`, no `taffy::Style`. Rows using `wrap`, grid, block,
  `align_content`, baseline, spans or auto margins fall back to taffy per
  slot with one `debug` log. `tests/lite_parity.rs` asserts rect equality
  with taffy on 18 row trees.
- `Cx`: `layout_id` / `with_layout_id` (node keys vs hook scope),
  `with_root_size`, `text`; `container` takes `(&ContainerStyle, &ItemStyle)`;
  `in_taffy` means "inside a `<View>`" on either path.
- egui_taffy is gone from the workspace; `taffy` 0.9 is a direct dependency.
  The fork `../egui_taffy` (branches `skip-unchanged-discard` d618550,
  `layout-first` ee07d38, not pushed) is kept only as the source of possible
  upstream PRs for B and C. Decision: file them later.

### Behaviour changes to know about

- `<Text>` inside a `<View>` is selectable per `selectable_labels`, as
  `Label`; a text created this frame is not selectable for that one frame.
- list-10k opens with virtualise on.
- VirtualList rows land at the pitch `show_rows` reserved (was 2 pt short
  per row).
- `Cx::container` signature changed; `Cx::new_taffy` removed; `<View>` passes
  styles by reference.

### What remains

- Scroll at 1.36x: each slot's text changes, the lite solver re-solves the
  row (about 360 ns/row) although fixed-width and `grow` columns cannot
  move. Candidate: skip the solve when no box can move. Second candidate:
  cheaper component layer (`Cx::scope`, props, about 200 ns/row).
- Gallery costs 10 to 11 ms per frame whatever example is shown: the source
  panel draws the whole highlighted file every frame. Separate issue; a
  row-virtualised source view would fix it. Also seen: at 800 px width the
  source panel overlaps the example column, and the showcase heading renders
  garbled glyphs.
- Real trackpad scrolling after F was verified with synthetic events only;
  check on a device.
- Upstream PRs for B and C from the fork: not filed.
- Pre-existing, untouched: `cargo fmt --all -- --check` fails on ~30 files
  (import order); the two board gallery snapshots were never generated.

### Reproduction

From the repository root:

```sh
PERF_CSV="$PWD/docs/tasks/perf/samples-<tag>.csv" \
  cargo test --release -p list-10k --test scenarios -- --ignored --nocapture
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace --target wasm32-unknown-unknown
cargo test -p gallery --features snapshot
cd examples/list-10k && trunk build --release            # egui-react list
cd examples/list-10k && trunk build --release index-plain.html   # plain list
```

The web measurement method (rAF wrapper, COOP/COEP server, synthetic wheel
events) is described in measurements.md, "Web".

### Product constraint

Preserve the React-like component API and declarative row layout. Direct
egui row layout stays a benchmark reference, not the production path; the
lite solver keeps the `<View>` / `<Text>` / `<Button>` row as written.

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
