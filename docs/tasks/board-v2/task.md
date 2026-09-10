# Task: board rework (v2)

Rework the [board](../board/task.md) example per the request: simpler, and closer to how Trello feels. Details in [plan.md](plan.md).

## Request (summary of the original)

- A card has only a title. Drop the body.
- "edit" is a pen icon. Pressing it turns the title into an input in place, with auto focus + select all. Enter commits, Esc cancels. Drop the old edit screen (title / body / save / cancel).
- Pressing "+ card" adds a new card in edit state (empty title).
- Drop the color label at the left edge of the card. Use a Done checkbox instead.
- `cursor: pointer` when hovering a card.
- Fix the bug where text behind the dragged range gets selected during a drag.
- During a drag, show the insert position as an empty card preview at the target, not as a blue line.

## Out of scope

- Changes to core / `egui-react-elements` (if needed, record in plan.md section 9 and do a separate PR).
- Animation, multiple boards, label editing UI.

## Done when

- The react version and the plain version give the same result for the same operations (the tests in plan.md section 8 are green for both).
- `cargo fmt --check` / `clippy --workspace --all-targets -- -D warnings` / `test --workspace` / `check --target wasm32-unknown-unknown` are green. The gallery a11y test is green too.
- The header comments of `lib.rs` / `plain.rs`, `docs/tasks/board/task.md`, and the README table match the new spec.
