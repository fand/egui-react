# Task: board (PR8 = first half of Phase 6.6. One PR together with [patch](../patch/task.md))

## Goal

Show, in one example, the React benefits that the existing examples do not show. That is:

- **Elements that are added, removed, and reordered at runtime can each hold local state.**
- **That state follows the key (identity).** Move a card to another column and its "draft being edited" and "expanded" state move with it. They do not get swapped with the neighbor card's state.
- **You can build a screen from your own components that take props / children / `#[event]`.** The current examples only use library elements. The only example of a user-made part that gets reused is `Themed` in `showcase`.
- **You can pull behavior out into a custom hook and reuse it.** Show that more practical hooks than the three in the `custom-hook` example (DnD, debounce, undo) can be written in user land without touching the library.

The subject is a Trello-style kanban board. Put a plain egui version of the same UI next to it. Show, in code and in line counts, that the caller no longer has to hold per-item local state in a `HashMap<CardId, _>` and clean it up by hand.

The second half of the same PR ([patch](../patch/task.md)) reuses the DnD custom hook this example builds, as-is. **Finish board first, then start patch.** The split is: board is "the place to prove each benefit one at a time", patch is "the place to show it holds up on a large screen".

Do not touch core (`egui-reactor`, `egui-reactor-macros`). Keep additions to `egui-reactor-elements` to the bare minimum (decide per [plan.md](plan.md) section 4).

## Scope

### In scope

- `examples/board` (lib + bin + `plain.rs`).
  - Four columns (Backlog / Doing / Review / Done) side by side. Each column is a vertical list of cards.
  - Card: title and a done checkbox (reworked in v2. Label color and body were dropped). **Inline editing (`editing` / `draft`) is the card's local state**.
  - Drag and drop: move between columns and reorder within a column.
  - Column: inline rename, card count, add a card at the end.
  - Toolbar: search (debounce), open / done filter (v2. Old: labels), undo / redo, dark / light.
  - `use_reducer` holds the board contents and `use_persisted` carries them across restarts (same combination as `showcase`).
  - undo / redo use `use_undoable` (a custom hook that wraps the reducer with history).
  - `Dispatch` and the theme are handed out with `provide_context`. The `<View>`s in between know nothing.
  - The visible cards per column (result of search + filter) come from `use_memo`.
  - Own components: `<Card>` `<Column>` `<Chip>` `<IconButton>` `<Toolbar>`. Each takes `style: ItemStyle` to follow the caller's layout, and returns events to the parent with `#[event]`.
  - Own hooks: `use_dnd` / `use_debounced` / `use_undoable`.
- `plain.rs`: same UI, same look, in plain egui. Includes holding and cleaning up per-card state, DnD, and undo.
- Register in the gallery (`Meta`), kittest, the README table, and the line count comparison.
- Record "what the library lacks" found during implementation in plan.md section 8.

### Out of scope

- Changes to core (`egui-reactor` / `egui-reactor-macros`). If a need comes up, write it in plan.md section 8 and handle it in a separate PR.
- DnD animation (ghost interpolation, reorder easing). Positions may swap at once.
- Multiple boards, label editing UI, due dates, attachments, assignees, search highlighting.
- Server sync, real data, file I/O. Only one `use_persisted` key.
- Virtualization. Assume a few dozen cards (`list-10k` covers large element counts).

## Deliverables

- `examples/board/` (`src/lib.rs` / `src/board.rs` / `src/hooks.rs` / `src/plain.rs` / `src/main.rs` / `src/plain_main.rs` / `Trunk.toml` / `index.html` / `tests/board.rs`).
- Registration in `examples/gallery` (two places: `EXAMPLES` and `match name`), and the gallery snapshot.
- One row in the README examples table.
- If needed, a minimal addition to `crates/egui-reactor-elements` and an update to `docs/ARCHITECTURE.md` section 6.
- plan.md section 8 (differences found during implementation, and homework for the library side).

## Done when

- kittest is green. The two highlights are (plan.md section 6 B-2 / B-3):
  - Move a card being edited to another column. Its draft and expanded state follow the card.
  - Reorder within a column. State does not get swapped with the neighbor card.
- Run the same operations on the egui-reactor version and the plain egui version and get the same result (same method as `todo` / `form`). The gallery snapshot matches with one image.
- `cargo run -p board` and `cargo run -p board --bin board-plain` work. `trunk serve` also works in the browser (checked by eye).
- `board` appears in the gallery, `#board` links to it directly, and you can switch to the plain egui version.
- `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` / `cargo check --workspace --target wasm32-unknown-unknown` / the trunk loop are green in CI.
- The README table and ARCHITECTURE match the implementation.

## Decisions (assumptions at the start)

- It goes in the gallery, so do not use `Panel` / `CentralPanel`. Make it a component that fills the area it is given. Do not use `std::time::Instant`. Take the time from `ctx.input(|i| i.time)`. The `use_persisted` key is `"board/..."`.
- For DnD, first try egui 0.36's `Ui::dnd_drag_source` / `Ui::dnd_drop_zone` as-is. If it does not fit taffy, fall back to `Ui::interact` + `DragAndDrop::set_payload` with our own hit testing (criteria in plan.md section 3). Either way, keep "it can be written as a user-land custom hook / component".
- Do not make undo / redo a library feature. Showing that a custom hook wrapping `use_reducer` is enough is part of what this example claims.
- Write the plain egui version fairly. Do not pick a broken style on purpose, like holding per-card state by `Vec` index. Write it correctly with `HashMap<CardId, CardUi>`, and show as the difference that creating, dropping, and cleaning up that map is the caller's job.
- Keep the look within what one snapshot can match (a rounded `Frame`, a thin bar of label color, a count badge, about that much). No fancy decoration.
