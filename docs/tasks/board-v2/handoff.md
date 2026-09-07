# Handoff: phase 6.6 (board + patch)

Moved here from the repository root's `progress.md`. Translated into English
from the original handoff; its status and environment limitations belong to
that work and were not revalidated afterwards.

PR: https://github.com/fand/egui-react/pull/10, branch
`claude/complex-ui-demo-idea-lmigbs`, base `main`.
Task definitions: `docs/tasks/board/` and `docs/tasks/patch/`.
Section 8 of each plan.md records implementation findings.

## Status (2026-09-06)

| Item | Status |
|---|---|
| board, including plain version, gallery registration, tests | Complete: `6e8b8a8` |
| board v2: title-only cards, done checkbox, pen for inline editing, whole-card dragging, empty-card insertion preview | Complete; changes and notes in `docs/tasks/board-v2/plan.md` section 11 |
| patch, including gallery registration and tests | Complete: `12ba0fb` |
| Merge origin/main (a11y PR #5) | Complete: `cdf4246`; only conflict was gallery's setup closure, resolved by calling shader/patch `gpu::setup` and registering WebA11y |
| fmt, workspace clippy, wasm check | Passed after merge |
| Workspace tests | Passed; board v2 fixed the a11y failures below |
| trunk build, GPU snapshots, manual device inspection | Not done; the earlier container had neither trunk nor a GPU |

## Accessibility fixes (complete in board v2)

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

## Remaining verification on a GPU machine

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

## PR description cleanup

The earlier handoff reports a claude.ai session link appended by the UI; remove
it under the repository's CLAUDE.md rule against signatures/session links in
commits and PR bodies. Also correct the claim that both examples include gallery
snapshot tests: patch does not. Neither change was checked during perf work.

## Library follow-ups for separate PRs

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

## Design details for the next contributor

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
