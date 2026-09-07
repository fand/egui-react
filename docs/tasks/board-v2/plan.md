# Plan: board rework (v2)

> `egui_taffy` below is historical. It was replaced in 2026-09 by egui-react's
> own layout engine over taffy (`crates/egui-react/src/engine.rs`, ARCHITECTURE
> section 6), which ports its measure function and node rules, so the layout
> behaviour described here still holds unless ARCHITECTURE says otherwise.

The task definition is in [task.md](task.md). The original design is in [board/plan.md](../board/plan.md) ("v1 plan" below). This document only writes the diff from v1. If the implementation departs from this, update this document.

## 0. Overview

- Touches `examples/board/` (`board.rs` / `lib.rs` / `hooks.rs` / `look.rs` / `plain.rs` / `tests/board.rs`), `examples/gallery/tests/a11y.rs`, docs, and the README table. core and `egui-react-elements` are unchanged. If something is missing, write it in section 9 and work around it.
- The react version and the plain version behave the same and are driven by the same tests (v1's B-7 method). **Fix both in the same commit.**
- Keep the example's claim (a card's local state follows the card). body and `expanded` go away, so the state that follows the card becomes the two: **`editing` and `draft`**. So **dragging while editing** is a must (section 3).
- Match the existing style (comments and docs in English).

## 1. Data model (`board.rs`)

```rust
pub struct Card { pub id: CardId, pub title: String, pub done: bool }

pub enum Msg {
    /// Add with a title. A card with an empty title never enters the board (section 4).
    AddCard { column: ColumnId, title: String },
    SetTitle { card: CardId, title: String },
    SetDone { card: CardId, done: bool },
    RemoveCard { card: CardId },
    MoveCard { card: CardId, to_column: ColumnId, to_index: usize },
    RenameColumn { column: ColumnId, name: String },
}

/// `done`: `None` means all, `Some(true)` means only done, `Some(false)` means only open.
pub fn visible(column: &Column, search: &str, done: Option<bool>) -> Vec<CardId>;
```

- Remove the `Label` enum, `body`, `EditCard`, and `SetLabel`. Remove `Theme::label` too.
- `Board::demo()`: the same 12 cards, title only. The 3 in the "done" column are `done: true`, the rest `false`.
- `SetTitle` sets `changed = false` (not pushed onto history) if the title is the same. The caller rejects an empty title for `AddCard`, but the reducer also does nothing if `title.trim().is_empty()`.
- Update the unit tests to the new spec (`visible` looks only at title, `done` filter, empty title in `AddCard`).
- If old-format JSON (with `body` / `label`) is left in `use_persisted("board/board")`, `serde_json::from_str` fails and `use_persisted` falls back to `init` (`restored.unwrap_or_else(init)` in `hooks.rs`). That is fine. Do not change the key. The plain version's `STORAGE_KEY` is the same.

## 2. Drawing the card (`<Card>` in `lib.rs`)

Make the card a single line: `[☐] title ........ [✎] [×]`.

```
<Card key={id} card=.. column=.. next=.. on_title on_done on_remove/>
  cx.leaf(..)                         <- the whole card is one leaf (because a rect is needed)
    ui.interact(rect, id, Sense::drag())   background. drag and cursor (2.2)
    egui::Frame::show(..)                  rounded frame (same look as the current <Frame>)
      rsx via Cx::new(store, ui, scope):
        <View row align="center" gap={6}>
          <Checkbox bind=.. />                 done (2.3)
          if editing { TitleEdit } else { Label with the title }   (section 3)
          <IconButton name="edit">"✎"</IconButton>
          <IconButton name="remove">"×"</IconButton>
        </View>
```

### 2.1 Why one leaf

As v1 plan §8.4 says, `<View>` / `<Frame>` do not return a rect. This time three things need the card's rect: "grab the whole card", "make the whole card a drop target", "change the cursor on hover over the card". The `<Frame>` implementation (`containers.rs`) calls `egui::Frame::show` inside `cx.leaf` and draws the children with `Cx::new(store, ui, scope)`, so writing the same thing by hand inside `<Card>` gets us the rect. elements are not touched.

### 2.2 The background drag surface

- Pass `let rect = ui.max_rect();` to `ui.interact(rect, ui.id().with("bg"), egui::Sense::drag())` **before drawing the children**. `max_rect` is the rect taffy gave the leaf. Its height is off only on the first frame and correct from the second (same treatment as the footer's `leaf_fill`).
- **Why register first**: egui's hit test picks click candidates and drag candidates separately, each "topmost first" (`hit_test.rs`). With the background placed first, Checkbox / IconButton (click only) take the click, TextEdit (click+drag) takes drags inside itself, and drags anywhere else fall to the background. If you move after pressing, the click candidate is dropped and it becomes a drag (`interaction.rs`), so **pressing on the checkbox and moving drags the card**. The tests use this (section 8).
- On `bg.drag_started()` call `dnd.pick_up(card.id)`. Remove `Sense` from the title Label (section 3).
- **cursor**: if `bg.contains_pointer()`, `ui.ctx().set_cursor_icon(CursorIcon::PointingHand)`. `contains_pointer()` rather than `hovered()`: being over a child widget is still being over the card, so show the pointer. While dragging (`dnd.carrying().is_some()`) use `Grabbing`.
- Drop slots split **the whole card's rect** top and bottom (drop v1's title row ± `SLOT_PAD`. Remove `SLOT_PAD`). `dnd.slot(top, before: Some(card.id))`, `dnd.slot(bottom, before: *next)`.
- The carried card (`carried`) is still drawn faintly in place as before (removing it would sweep the `use_identity` slot and lose the draft. v1 plan §8.1).

### 2.3 done checkbox

- `<Checkbox bind={done.bind()} on_change=..>` requires `&mut bool` for `bind`. The card's `done` is a prop (`&CardData`), so it cannot be bound directly. The shortest way is **draw `egui::Checkbox::without_text(&mut local)` in `cx.leaf`, and if `changed()` then `on_done.emit(!card.done)`** (`local` is a copy made on the spot). Do not use the elements `<Checkbox>`.
- a11y: `ui.ctx().accesskit_node_builder(response.id, |n| n.set_label(format!("done: {}", card.title)))`. **The tests find the card by this name** (section 8).
- A done card draws its title `weak()` + `strikethrough()`.

### 2.4 IconButton

- `<IconButton name="edit">"✎"</IconButton>`: add a `name: Option<&str>` prop. If given, override the label with `accesskit_node_builder`. The display is a picture, the name in the tree is a word. Tests can still find it with `get_all_by_label("edit")` as before. "×" also gets `name="remove"`.
- "✎" (U+270E) is in the emoji-icon font that ships with egui. If not, fall back to "✏" or the text "edit" (it cannot be checked on a real machine, so before suspecting `egui::FontDefinitions` on the first build, note that the tests pass even without a gallery screenshot. Check the look with the steps in section 10).

## 3. Inline editing (`TitleEdit`)

The same part is used in three places: the card title, the column rename, and the new card (section 4). In `lib.rs`: `#[component] fn TitleEdit(cx, style, bind: &mut String, name: &str, #[event] on_commit: String, #[event] on_cancel: ())`.

- `egui::TextEdit::singleline(bind)` inside `cx.leaf`. `desired_width(ui.available_width())`.
- **Auto focus + select all**: only once, right after mount. `let mut fresh = use_state(cx, || true);` and if `*fresh`:
  ```rust
  response.request_focus();
  let mut state = egui::TextEdit::load_state(ui.ctx(), response.id).unwrap_or_default();
  let end = egui::text::CCursor::new(bind.chars().count());
  state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), end)));
  state.store(ui.ctx(), response.id);
  *fresh = false;
  ```
  `TitleEdit` is unmounted when the edit closes, so it is `fresh` again the next time it opens. When a card moves across columns it is remounted, so it re-focuses and re-selects (the draft stays because it is `use_identity`). Accept this and write it in a comment.
- **Enter commits**: `response.lost_focus() && input.key_pressed(Enter)` -> `on_commit.emit(bind.clone())`.
- **Esc cancels**: `input.key_pressed(Escape)` and this field had focus (egui gives up focus on Esc. `lost_focus() && key_pressed(Escape)`) -> `on_cancel.emit(())`.
- **Lost focus any other way** (clicked somewhere else) -> **commit** (same as Trello). `lost_focus` never fires before the frame where the post-mount `request_focus` takes effect, so the order does not matter.
- a11y: `set_label(name)` via `accesskit_node_builder` (`"title"` / `"column name"` / `"new card"`). The gallery a11y test counts unnamed `TextInput`s.

Callers:

- **`<Card>`**: `editing` / `draft` use `use_identity` as in v1. The pen does `*draft = card.title.clone(); *editing = true`. `on_commit`: `on_title.emit(title)` then `*editing = false`. Committing an empty title (`trim().is_empty()`) is **treated as cancel** (the title is not changed). `on_cancel`: `*editing = false`.
- **`<Column>` rename**: replace `<TextEdit on_submit>` with `TitleEdit` (Esc can now revert). The "rename" button stays.
- Remove v1's `Draft` struct, the `save` / `cancel` buttons, `expanded`, and the body `<Text wrap>`.

## 4. New card ("+ card")

**Do not put it on the board. Edit it in the column's local state.** Send `AddCard { column, title }` once on commit.

- Put `let mut adding = use_state(cx, || false); let mut new_title = use_state(cx, String::new);` in `<Column>`.
- "+ card" does `*new_title = String::new(); *adding = true`.
- If `*adding`, draw `TitleEdit name="new card"` at the end of the card list (right before the footer), in the same frame as a card (`Frame`, same padding). `on_commit`: if the title is not empty, `send(AddCard { column, title })`, and either way `*adding = false`. `on_cancel`: `*adding = false`.
- Why this shape: putting an empty-title card on the board and then removing it makes history two steps, "add" and "remove", and an empty card could be left in `use_persisted`. With the column's local state, undo is one step and only committed cards are saved. `Column`'s `on_add: ()` event becomes `on_add: String`, and `BoardView` sends `AddCard`.
- The new card frame has no drop slot (the footer keeps handling "the end").

## 5. Drag (`lib.rs` / `hooks.rs`)

### 5.1 The text selection bug

Cause: even with `Sense::click_and_drag` on the title `egui::Label`, the Label stays `selectable_labels` (default true), so text selection starts on that Label the moment you press, and `LabelSelectionState` extends the selection to other Labels the drag passes over (`label_text_selection.rs`).

Fix: make the title Label `.selectable(false)` and remove the `Sense` (grabbing is done by the background in 2.2). If the press does not start on a Label, no selection starts. **Put the same fix in the plain version.** Verify with B-12 in section 8 (`egui::text_selection::LabelSelectionState::load(ctx).has_selection()` is false after the drag). If selection still happens, set `ctx.style_mut(|s| s.interaction.selectable_labels = false)` in a `use_effect` in `BoardProvider` (last resort, since it affects the other gallery examples too).

### 5.2 Insert position preview

Replace the blue line (`hline`) with **an empty card placeholder**. Remove the `hline` in `Card`'s `before_me` / `Column`'s `hovered_end`.

- `Column` draws it. Look at `let target = dnd.hovered().filter(|t| t.column == column.id);` and put one `<Placeholder>` in the card list **right before** the card with `target.before == Some(card.id)`, or right before the footer (after the new card frame from section 4) if `before == None`.
- **Do not show it if it equals the carried card's original position**: ignore `target.before == Some(carried)` or `target.before == after[i]` (carried is the i-th). A drop that moves nothing needs no preview.
- `Placeholder` is a `cx.leaf` with the same width as a card, `allocate_exact_size(.., Sense::hover)` with `PLACEHOLDER_H` (= the height of one card row. A constant, about `ui.spacing().interact_size.y + 12.0`), and draws a rounded dashed frame (`theme.accent()`, inside a faint `theme.card()`).
- **Always register `dnd.slot(rect, target)`.** When the placeholder appears, the cards below shift and the pointer ends up over the placeholder. If that rect is not a slot, `hovered` becomes `None`, the placeholder disappears, the cards move back, `hovered` fires again, and this repeats every frame. If the placeholder itself is a slot for the same target, it is stable.
- Name it with `response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "drop here"))` so tests can see it (section 8 B-11).
- The ghost (`look::ghost`) stays. Fix the `Theme::accent` comment ("insert line").
- The plain version draws it the same way (same constants, same slot registration).

### 5.3 `hooks.rs`

No change. The `Dnd` API can be used as-is.

## 6. Toolbar

The label chips lose their meaning, so replace them with **a filter made of two chips, "open" / "done"** (`filter: Option<bool>`). Pressing the pressed chip again clears it (same behavior as v1's `Option<Label>`). `<Chip>` and the `use_memo` deps (`(column.id, rev, search, filter)`) stay as they are. The color is `theme.accent()`. Same in the plain version.

## 7. Plain version (`plain.rs`)

Mirror each item of the react version as-is. Keep v1's premise that the only difference is "where the state lives".

- `CardUi { editing: bool, draft_title: String }`. Remove `expanded` / `draft_body`.
- New card: `adding: Option<(ColumnId, String)>` (same shape as `renaming`. Only one column at a time. The reason is the same as v1 plan §5, written in a comment).
- Auto focus + select all: `PlainState` holds `fresh: Option<egui::Id>` (the id of the field to focus and select on the next frame). This matches the react version's `fresh` state.
- Background drag, cursor, checkbox, placeholder, `.selectable(false)` on the Label, open/done filter.

## 8. Tests (`tests/board.rs`)

Update the helpers to the new spec and run the same functions on both react / plain.

- **Finding a card**: `get_by_label(title)` by title (a Label has its text in `value`. This already passes). While editing there is no Label, so find it by **the checkbox `"done: <title>"`**. `fn card_rect(harness, title) -> Rect` does not use the checkbox rect as the base; instead the center of the `"done: <title>"` checkbox is **the grab point** (2.2: pressing on the checkbox and moving drags the card). `drag`'s `from` is this.
- **Drop target**: `top()+1` / `bottom()-1` of the `onto` title Label's rect (falls in the card's top half / bottom half). The footer stays `"+ card"`.
- `fn open_editor(title)`: click the `"edit"` whose y is close (same as now). Once open, assert that `draft_field()` **has focus** (`accesskit_node().is_focused()` or `harness.ctx.memory(|m| m.focused())`).
- `fn draft_field()`: the last single-line `TextInput` (in order: search -> rename -> editing or new). Finding it by name `"title"` / `"new card"` is more reliable, so use `get_by_label`.
- `fn edit(title, text)`: `open_editor` -> `key_press(Key::End)` (clear the select-all and go to the end) -> `type_text(text)`. `fn editors()` is `query_all_by_label("title").count()`.

| # | Content |
|---|---|
| B-1 | "+ card" -> the new field is focused -> type "new card" and Enter -> 13 cards, backlog 5/5, `column_of("new card") == 0`. Then "+ card" -> Esc -> still 13. "+ card" -> Enter without typing -> still 13 |
| B-2 | `edit("buy milk", " and bread")` -> while still editing, drag onto the top half of "wire the drag" -> column 1, order, `draft() == "buy milk and bread"`, `editors() == 1`, saved title still "buy milk" |
| B-3 | `edit("read the plan", "!")` -> onto the top of "write the plan" -> order swapped, `editors()==1` with draft "read the plan!", "write the plan" not editing -> move back onto the bottom half -> same |
| B-4 | add (steps of B-1) -> "buy milk" to the last column -> undo x2 -> redo x2 (same as now) |
| B-5 | search (same as now. Looks only at title but the result is the same: backlog 2/4, review 0/2) |
| B-6 | persistence (only the add steps change) |
| B-8 | rename: same as now + open `rename` and Esc -> the name does not change and the field closes |
| B-9 (new) | edit: pen -> typing "z" right after it opens gives draft "z" (confirms select-all) -> Esc -> title unchanged, editors()==0. pen -> End -> "!" -> Enter -> title "buy milk!", editors()==0, undo reverts |
| B-10 (new) | done: click the "buy milk" checkbox -> `toggled` -> undo reverts. chip "done" -> done column 3/3, backlog 0/4 -> press again to clear |
| B-11 (new) | preview: grab "buy milk" and hover over the bottom half of "wire the drag" (`drag_at` -> `hover_at` x2, do not release) -> `query_by_label("drop here")` exists, the y of "name the hooks" is lower than before the hover -> `drop_at` -> no "drop here" |
| B-12 (new) | selection: after the same drag as B-11, `LabelSelectionState::load(&ctx).has_selection()` is false |
| B-7 | run B-1 to B-12 on the plain version |

Unit tests for `board.rs` are in section 1.

## 9. gallery / docs

- `examples/gallery/tests/a11y.rs`: the new focusables are the checkbox (named), `TitleEdit` (named), and IconButton (named), so `KNOWN_UNNAMED` should not grow. If it fails, **prefer giving a name**, and add only what really cannot be named (the comparison is strict in tree order, so mind the position).
- The doc at the top of `lib.rs`: change the body / expanded text to title / editing. `META.summary` and `elements` (drop `Frame` since it is hand-written; do not add `Checkbox` since it is not used).
- The doc at the top of `plain.rs` likewise.
- Add one line, "Reworked in v2. The diff is in [board-v2/plan.md](../board-v2/plan.md)", to the scope (card item) of `docs/tasks/board/task.md` and to the top of `docs/tasks/board/plan.md`. Do not rewrite the body of the v1 plan (it is the record from that time).
- Update the board row in the README examples table with the new description.
- Add v2 to the board row in `handoff.md` (was the root `progress.md`).

Things that were tempting to add to elements (not added. Record only):

- `<TextEdit autofocus select_all>`, a non-bind version of `<Checkbox>` (`checked` + `on_change`), `on_response` on `<Frame>`. All worked around with hand-written leaves.

## 10. Steps (for the subagent)

After each step run `cargo fmt && cargo clippy -p board --all-targets -- -D warnings && cargo test -p board`. At the end run the whole workspace. Do not run GPU snapshots.

1. `board.rs`: section 1. Unit tests green.
2. `lib.rs`: `TitleEdit` (section 3) -> `Card` (section 2) -> `Column` (sections 4, 5.2) -> `Toolbar` (section 6) -> `look.rs`. `tests/board.rs` may be broken at this point.
3. `tests/board.rs`: section 8. Make the react version's B-1 to B-6, B-8 to B-12 green.
4. `plain.rs`: section 7. Make B-7 green.
5. Section 9 (a11y, docs, README). `cargo test --workspace`, `cargo check --workspace --target wasm32-unknown-unknown`.
6. If possible, check by eye with `cargo run -p board` / `--bin board-plain`: cursor, select-all, placeholder, rendering of "✎".

## 11. Differences found during implementation

### 11.1 The placeholder is always in the tree (correction to 5.2)

"Insert `<Placeholder>` only when hovered" does not work. **A taffy node that appears mid-list has an empty rect on the frame it appears** (egui_taffy draws `first_frame` in a sizing pass and lays out on the next frame). That one frame is exactly when the pointer asks for the placeholder (the gap opens and the card slips out from under the pointer), so the slot misses, the gap closes, the card comes back, and it oscillates every frame (measured: the card's y bounced between 90 and 162, width between 92 and 13, and the drop never landed).

Fix: **always draw one per card and one before the footer**. Only the height changes via an `open` prop (`CARD_GAP` / `CARD_GAP + PLACEHOLDER_H`). A node that only changes height always has a rect.

### 11.2 `CARD_GAP` replaces the card list's `gap`

A closed placeholder **is the spacing between cards**. The `<View>` `gap` was removed (0), and `const CARD_GAP: f32 = 6.0` became the placeholder's closed height. With both it would be 12px. "nothing here" and the new card frame get `mt={CARD_GAP}`. 6px goes above the first card, but that does no harm. Same in the plain version (`item_spacing.y = 0.0` + `drop_gap`).

### 11.3 `Placeholder` is `leaf_fill`, not `leaf`

`cx.leaf` is measured by its contents. On the first frame it is measured with a zero-width `Ui` and taffy pins that as-is (ARCHITECTURE 6). A zero-width slot cannot be entered by the pointer. Changed to `leaf_fill` + `w("100%")` / `h(..)` in the style.

### 11.4 Keep `Frame` in `META.elements`

The new card frame does not need a rect, so it still uses the `<Frame>` element. Removing it would make the display a lie. `Button` is no longer used, so it was removed.

### 11.5 Two extra things the plain version has to hold (supplement to section 7)

- `CardUi.rect: Option<egui::Rect>`: the background drag surface must be registered **before the children**, but in immediate mode the card's rect is unknown before drawing. Remember the previous frame's rect and use it. The react version just reads what taffy already computed.
- `PlainState.fresh: Option<egui::Id>`: the id of the field to auto focus + select all. The react version uses `use_state` inside `<TitleEdit>`, so it never has to name the field.

### 11.6 Notes on the test side (supplement to section 8)

- **Caret blink**: a focused `TextEdit` requests a repaint every frame, so `Harness::run` fails with `ExceededMaxSteps`. After creating the harness, call `h.ctx.all_styles_mut(|s| s.visuals.text_cursor.blink = false)` (egui 0.36 has no `Context::style_mut`).
- **B-12**: egui 0.36 has no `LabelSelectionState::load`. Use `ctx.plugin::<egui::text_selection::LabelSelectionState>().lock().has_selection()`.
- **`"done"` is ambiguous**: the toolbar chip and the 4th column name have the same label. Narrow it down with something like `get_all_by_label("done").next()` (the toolbar is drawn first).

### 11.7 a11y: the card background is focusable too (supplement to section 9)

`ui.interact(rect, id, Sense::drag())` creates a focusable node, so without a name `a11y.rs` shows 12 `board: Unknown` (in v1 the title Label with a sense showed up at the same position as `board: Label` x12). Named it **`"card: <title>"`** via `accesskit_node_builder`. Not the title itself, because two nodes with the same name next to each other cannot be told apart by a screen reader or by kittest.

`KNOWN_UNNAMED` had not been updated since board / patch landed, and was red before this work. Only what could not be named was added, with reasons: `board: TextInput` (the search `<TextEdit>`. The element has no name prop) and 5 from patch (`GenericContainer` of the overflowing `<ScrollArea>`, `MultilineTextInput` x2, `ComboBox` x2).

### 11.8 Gap animation (extra request)

Since the placeholder is always in the tree (11.1), changing its height smoothly with `animate_bool_with_time` would make the cards below slide. That is what I wrote at first, but **changing a taffy node's height every frame makes egui_taffy re-layout every frame and call `request_discard`**, and egui prints "PERF WARNING: request_discard has been called N frames in a row" on screen. egui_taffy marks dirty if the style differs from the previous frame -> recomputes at the end -> discards (`recalculate` in `egui_taffy/src/lib.rs`).

Fix: **layout is immediate, only the picture moves.**

- `look::gap_amount(ctx, id, open, carrying)`: how far the gap "looks open", 0..1 (`GAP_TIME` = 0.12s, quadratic_out). When not carrying, decide at once with time 0 (the card appears for real on the frame after the drop, so it must not overlap a half-closed gap).
- `<Column>` computes `amount` for each gap and accumulates `lift += (amount - open) * PLACEHOLDER_H` from the top. Pass `Gap { open, amount, lift }` to the placeholder and `lift` to the card.
- `<Card>` / footer draw shifted down by `lift` with `ui.with_visual_transform(look::lifted(lift), ..)`. **Input does not move** (egui behavior), so the drag surface and slots stay at the layout position = landing position. At 120ms that does no harm.
- The placeholder picture goes from `rect.top() + lift + CARD_GAP` with height `amount * H`. While closing it overflows the layout rect, but the clip is the ScrollArea's.
- The new card frame (`<Frame>` element) and "nothing here" do not move. The former only shifts "when a drag starts while typing". The latter has no gap above it, so `lift` = 0.
- The plain version does the same accumulation in the `column()` loop, passes `amount` / `lift` to `drop_gap`, `lift` to `card`, and uses the same `with_visual_transform`.

Test B-13: with a 60fps harness (`with_step_dt(1/60)`), advance the clock by hand and `step()`, and check that `output().platform_output.num_completed_passes` is 1 on each frame while the gap is opening. With kittest's default step (0.25s), egui counts predicted_dt as already elapsed, so the 120ms animation finishes in one frame and the old implementation would pass too. Confirmed that B-13 fails on the old implementation.
