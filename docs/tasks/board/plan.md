# Plan: board

> Reworked in v2. The diff is in [board-v2/plan.md](../board-v2/plan.md). The text below is the record from that time, so do not rewrite it.

The task definition is in [task.md](task.md). The design rationale is in [docs/ARCHITECTURE.md](../../ARCHITECTURE.md). If you decide to depart from this during implementation, update this document, and update ARCHITECTURE.md too if it matters for the design.

## 0. Overview

One PR. Touches `examples/board` (new), `examples/gallery` (registration), README, and if needed `crates/egui-react-elements` and ARCHITECTURE section 6. core is unchanged.

```
examples/board/src/
  lib.rs        App and the screen components (this is what the gallery reads)
  board.rs      data model, Msg, reduce, filtering (pure functions, with unit tests)
  hooks.rs      use_undoable / use_debounced / use_dnd / use_identity
  look.rs       colors and ghost drawing used by both versions (8.6)
  plain.rs      plain egui version of the same UI
  main.rs       bin for run(..)
  plain_main.rs bin for the plain egui version
```

Splitting `board.rs` and `hooks.rs` follows the same shape as `custom-hook` / `showcase`. The gallery shows only `lib.rs` via `include_str!("lib.rs")`, so **put all of the screen assembly in `lib.rs`**. Priority goes to letting the reader follow "building UI the React way" in one file.

## 1. Data model and reducer (`board.rs`)

```rust
pub type CardId = u64;
pub type ColumnId = u64;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Board {
    pub columns: Vec<Column>,
    next_id: u64,
    /// Revision number mixed into `use_memo` deps. Bumped by 1 only when `reduce` changed the contents.
    pub rev: u64,
}

pub struct Column { pub id: ColumnId, pub name: String, pub cards: Vec<Card> }
pub struct Card { pub id: CardId, pub title: String, pub body: String, pub label: Label }
pub enum Label { None, Red, Yellow, Green, Blue }

pub enum Msg {
    AddCard { column: ColumnId },
    EditCard { card: CardId, title: String, body: String },
    SetLabel { card: CardId, label: Label },
    RemoveCard { card: CardId },
    /// Handles both moving between columns and reordering within a column. `to_index` is
    /// the index in the target column after removing the card itself.
    MoveCard { card: CardId, to_column: ColumnId, to_index: usize },
    RenameColumn { column: ColumnId, name: String },
}

pub fn reduce(board: &mut Board, msg: Msg);
/// Ids of the cards visible in that column, filtered by search term and label.
pub fn visible(column: &Column, search: &str, label: Option<Label>) -> Vec<CardId>;
```

The key point is making `MoveCard` one message. If "pull from a column and insert at a position in another column" is one step, undo is one step too, and the reducer does not need to hold the in-progress DnD state.

**Added during implementation (identity, not index)**: The drop target is `DropTarget { column: ColumnId, before: Option<CardId> }` ("before this card", or "end of the column" if `None`). `Board::drop_index(card, target) -> usize` converts it into `MoveCard`'s `to_index` (the index after removing the card itself). If the UI built the index, the caller would have to absorb two things: (a) the index shifts the moment the grabbed card leaves the column, (b) when search hides some cards, the on-screen order does not match the `Vec` index. `drop_index` is a pure function in `board.rs`, so it can be unit tested and shared with the plain egui version.

The initial data is `Board::demo()`, hardcoded with 4 columns and about 12 cards. Something must be visible right after opening it in the gallery.

## 2. Component structure (`lib.rs`)

```
App
└ BoardProvider            provide_context for the theme and Dispatch (same shape as the theme example)
  └ View column
    ├ Toolbar              search TextEdit / label filter / undo / redo / dark
    └ View row grow        columns side by side
      └ Column (key=id)    inline rename, count, "+ card", ScrollArea
        └ Card (key=id)    ★ the body that holds local state
          ├ Chip           small piece of label color (a leaf that only takes style)
          └ IconButton     × and ▸ (the smallest own element with children and on_click)
```

- **Data goes down via props. `Dispatch` and the theme go out via context.** The `State<'s, Board>` returned by `use_reducer` carries `'s`, so it cannot go into a `Handle<T>` (ARCHITECTURE section 6). `Dispatch`, on the other hand, is `Clone + Send + 'static`, so it can be put in `use_handle(cx, || ctx)` and handed out. The result is exactly the usual React shape (state via props, update function via context). Write this reason in a comment in `lib.rs`.
- The local state `<Card>` holds is the very claim of this example, so it must live in the card's own `use_state`.

```rust
#[component]
fn Card(cx: &mut Cx, card: &Card, #[prop(default)] style: ItemStyle, #[event] on_move: DropTarget) {
    let mut editing = use_state(cx, || false);
    let mut draft_title = use_state(cx, || card.title.clone());
    let mut draft_body = use_state(cx, || card.body.clone());
    let mut expanded = use_state(cx, || false);
    ...
}
```

- `<Card key={card.id} .. />` / `<Column key={column.id} .. />` inside `for` need a key (ARCHITECTURE 3.2).
- **Correction (found during implementation)**: key alone does not pass B-2. A hook's slot Id is "the scope chain + the call site", and `key` only tells *siblings under the same parent* apart. Cards are drawn inside columns, so `<Card key={7}/>` in `doing` and `<Card key={7}/>` in `review` are different slots. Move across columns and the draft stays behind in the old column. So the card's own state goes in `hooks::use_identity(cx, (card.id, "draft"), ..)`. This is a 3-line hook that re-roots the scope on the card's identity with `Cx::new(store, cx.ui(), Id::new(("board/identity", key)))` and then calls `use_state`. core is untouched (section 8). **This hook is why B-2 / B-3 pass. `key` only guarantees "two cards in the same column are not mixed up".**
- **What goes down via props / what goes out via context**: "What happened to this component" goes one level up via `#[event]` (`<Card>` sends edit / remove / label change to `<Column>`, `<Column>` sends "want to add one card" to `<BoardView>`). "What the whole tree shares" goes via context (theme, drag session, `Dispatch`). `<Column>` turns the card events it receives into messages using the `Dispatch` it took from context. That is the innermost place that knows the column id, and it saves the `for` that lays out 4 `<Column>`s from carrying 4 callbacks.
- Passing an `on_change` that touches the same state to an element with `bind` (`TextEdit bind={draft_title.bind()}`) gives E0502 (ARCHITECTURE section 6). Commit the edit with `on_submit` or from the button side with `dispatch.send(Msg::EditCard { .. })`.
- Columns share width equally with `<View grow={1.0} w={0.0}>`. The card list inside is `<ScrollArea grow={1.0}>` (it is `leaf_fill`, so give it a height).
- Build the visible cards per column with `use_memo(cx, (column.id, board.rev, &*search, label), || visible(..))`. `board.rev` goes in the deps so we do not hash the whole `Board`.

## 3. Drag and drop

The shape to try first (A):

- Build the card from `<View>` (taffy) and make **the header row the grab area**. The header lands inside `cx.leaf`, so there is a `&mut egui::Ui` and `ui.dnd_drag_source(id, CardId, ..)` works as-is. egui also draws the ghost.
- Drop target detection is our own. Register each card header's `Response::rect` and the column rect, for that frame only, on the handle `use_dnd` returns (`dnd.slot(rect, DropTarget { column, index })`). On the frame the pointer is released, pick the slot that contains the pointer position and fire `on_move`.
- The insert position indicator (a thin line between cards) is drawn with `cx.ui().painter()` from the rect of the slot selected during the drag.

If `dnd_drag_source` does not work cleanly inside a taffy leaf (B):

- Fall back to `DragAndDrop::set_payload` + `ui.interact(rect, id, Sense::drag())`, and draw the ghost ourselves in one `egui::Area`. The detection side (slot) is the same as A, so the impact is local.

Either way, DnD is **written as a custom hook + own components inside the example, without touching the library**. Only if this breaks (e.g. it cannot work without the `<View>` rect) consider a minimal addition to elements or `Cx::container`, record it in section 8, and decide separately.

**The implementation settled close to (B).**

- The grab area is the card title: one `egui::Label::new(..).sense(Sense::click_and_drag())` inside `cx.leaf`. `dnd_drag_source` is not used. It redraws the contents on the `Order::Tooltip` layer during a drag, so inside a taffy leaf "the measured contents" and "the drawn contents" would swap every frame. Grabbing, dropping, and drawing the line only need rectangles, so holding one `Response` is enough.
- The ghost is drawn directly on `ctx.layer_painter(LayerId::new(Order::Tooltip, ..))`, not in an `Area`. Placing a widget would put the same title in the accessibility tree twice, and both screen readers and kittest would see "two of the same label".
- Slots: split the rect of the card's title row top and bottom into two ("before me", "before the next card"), plus one for the column footer (`+ card`) as "the end". `<View>` does not return a `Response`, so anything that needs a rect has to be a leaf (section 8).
- `Dnd<P, T>` separates the payload and drop target types (`Dnd<CardId, DropTarget>`). Inside is `Rc<RefCell<..>>`, written through `Handle::with` (which does not request a repaint). `Handle::set` / `update` always `request_repaint`, so registering slots every frame would mean **the app never goes idle and `egui_kittest::Harness::run` fails with `ExceededMaxSteps`** (ARCHITECTURE 5.6). Section 8.

The shape of `use_dnd`:

```rust
/// What is being carried, and the drop targets registered this frame. It is a `Handle`, so
/// it can go on context and columns and cards see the same one.
pub struct Dnd<T> { dragging: Option<T>, slots: Vec<(egui::Rect, T)>, .. }
#[hook] pub fn use_dnd<T: 'static>(cx: &mut Cx) -> Handle<'_, Dnd<T>>;
```

## 4. custom hooks (`hooks.rs`)

```rust
/// Wraps `use_reducer` with history. `Undoable::Do(msg)` pushes present onto past and
/// drops future. Undo / Redo just rotate the three stacks.
pub enum Undoable<M> { Do(M), Undo, Redo }
#[hook] pub fn use_undoable<S: Clone + 'static, M: 'static>(
    cx: &mut Cx, reduce: impl Fn(&mut S, M), init: impl FnOnce() -> S,
) -> (State<'_, History<S>>, Dispatch<Undoable<M>>);

/// Returns the value once the input has been quiet for `delay` seconds. Written with only
/// `ctx.input(|i| i.time)` and `request_repaint_after` (no `Instant`, because of wasm).
#[hook] pub fn use_debounced(cx: &mut Cx, value: &str, delay: f64) -> String;
```

That `use_undoable` sits cleanly on top of `use_reducer` is the basis for "undo does not need to be a library feature".

**Difference in implementation**: The bounds are `S: Clone + PartialEq` (`M: Send + 'static` is what `use_reducer` requires). `PartialEq` is used so that "a message that changed nothing is not pushed onto history". Without it, you would have to press undo twice after an `EditCard` that did not change the value. `History<S>` is not `Serialize`. As in `showcase`, `present` is only mirrored into the `use_persisted` slot every frame, and history is not saved. The name is `use_debounced`.

`use_reducer` applies messages "on the next visit to the hook" (ARCHITECTURE section 4), so reading `*state` right after `send` gives the old value. Write handlers so they do not read it back.

## 5. Plain egui version (`plain.rs`)

```rust
pub struct PlainState {
    board: Board,                       // uses the same board.rs
    ui: HashMap<CardId, CardUi>,        // <- the difference is here
    history: Vec<Board>, future: Vec<Board>,
    search: String, search_debounce: Option<f64>, label: Option<Label>,
    drag: Option<(CardId, egui::Vec2)>, slots: Vec<(egui::Rect, DropTarget)>,
    dark: bool,
}
struct CardUi { editing: bool, draft_title: String, draft_body: String, expanded: bool }
pub fn ui(ui: &mut egui::Ui, state: &mut PlainState);
```

- Write it fairly. Key by `CardId`, not by index, and also write the line that **cleans `ui` when a card is gone** (without this line it leaks; that is what sweep takes care of in the egui-react version).
- Layout: `ui.columns(4, ..)` + `ScrollArea`, to get the same picture as the taffy version (same policy as the plain version of the `layout` example).
- DnD and undo behave the same as the react version. The logic is shared via `board.rs`, so the only difference is "where the state lives". That is the showpiece of this example, so write "what is shared / what is not" in the header comment of `plain.rs`.
- **Difference in implementation**: The draft for an in-progress column rename is `renaming: Option<(ColumnId, String)>` (only one column at a time). The react version uses `use_state` per `<Column>`, so two columns can be open at once. Holding one more map would match it, but that is one more thing to clean up for a feature nobody asked for, so it stays at one column, with the reason written in a comment in `plain.rs`. That "when the whole holds the parts' state, the caller is forced into choices like this" is itself the difference.
- Cleanup is the one line `state.ui.retain(|id, _| board.card(*id).is_some())`. Note that the react version's sweep is stricter than this: a card hidden by search is unmounted, so its draft is gone too. The plain version uses "still on the board" as the criterion, so in that case it stays. Both are consistent, but to make them the same you would write one more rule in the plain version.

## 6. Tests (`tests/board.rs`)

As in `todo` / `form`, build two harnesses, react and plain, and drive both with the same helpers.

- **B-1** Adding a card increases that column's count.
- **B-2 (highlight)** Open a card's editor, type a draft, and move that card to another column. After the move that card is still being edited and the draft is still there. The neighbor card at the destination is not in edit mode.
- **B-3 (highlight)** Move the second card to the top within a column. Only the card that was expanded stays expanded. Confirm the state is attached to the card, not the position.
- **B-4** undo / redo. Add -> move -> undo x2 -> redo returns to the original.
- **B-5** Search. Pass the debounce wait by advancing time in the harness. The filter works across columns.
- **B-6** Persistence. `Store::save_persisted` -> `load_persisted` into a new `Store` -> the same board appears (following the `showcase` test).
- **B-7** Run the same operations on react / plain and get the same result. **In the implementation the plain version does not read `CardUi` directly either. B-1 to B-5 / B-8 run through the UI as-is** (positions are checked in screen coordinates, so the same helpers work for both versions). This shows from the UI side that "plain can do the same if written correctly". The difference is whether you write the cleanup yourself, not can / cannot.
- **B-8 (added)** Inline column rename. `rename` -> type -> Enter changes the name, and that too is one history step, so undo reverts it. Run on both react / plain.
- Unit tests in `board.rs`: `reduce`'s `MoveCard` (forward / backward within the same column, between columns, to the end), `visible`, `drop_index`, and label cycling.

How to drive DnD: **kittest pointer operations were enough.** `harness.hover_at` -> `drag_at` (press) -> `hover_at` x2 -> `drop_at` (release), and egui treats this as a drag, not a click (`drag_started` means "`dragged` became true this frame", so the press frame and the move frame may be separate). None of the fallbacks (pushing directly into `input_mut().events` / adding another way to move in the UI) were needed.

Which column a card is in is checked by **the x coordinate on screen** (which quarter), and the order within a column by y. Both versions lay out 4 equal-width columns, so the same helpers work on both as-is, and we look at "the visible position" rather than "what the reducer did".

B-3 moves twice, dropping on the top half to go before and on the bottom half to go after, and checks that the expanded state stays with the same card either way.

## 7. gallery / README / CI

- Add the dependency to `examples/gallery/Cargo.toml`, put `board::META` in `EXAMPLES` (after `showcase`), and add both `Running` `match` arms (react and plain).
- `Meta`: `name: "board"`, `hooks: [.. + "#[hook]"]` (there are 4 own hooks, so the same tag as `custom-hook` was added), `elements: ["View", "Text", "TextEdit", "Button", "ScrollArea", "Frame", "Separator"]`, `plain: Some(include_str!("plain.rs"))`.
- snapshot: add a react / plain pair to `examples/gallery/tests/snapshots.rs`. **They cannot be made to match in one image, so it is two images like `list_10k`.** Columns are laid out by taffy's `gap` in the react version and by `ui.columns` and `ui.horizontal` item_spacing in the plain version, so even though the same things are drawn, positions differ by a few points. Matching them would mean writing `plain.rs` "to reproduce taffy's math" rather than "to be read", and that is on the far side of the line section 5 drew. Note that snapshots need a GPU, so none were generated in this environment (section 8).
- One row in the README table. Copy `Trunk.toml` and `index.html` from an existing example. The CI trunk loop walks `examples/*`, so no extra config is needed (to be confirmed).

## 8. Differences found during implementation

Neither core (`egui-react` / `egui-react-macros`) nor `egui-react-elements` was touched. Below is "what could not be written / how it was worked around / what to add if anything".

### 8.1 State is attached to a position in the tree, not to identity (the very theme of this example)

**What could not be written**: `use_state` inside `<Card key={card.id}/>` becomes a different slot when the card moves to another column. A hook's Id is "the scope chain + the call site", and `key` only tells siblings under the same parent apart (ARCHITECTURE 3.2 / 3.4). React is the same: when the parent changes, it unmounts and mounts again. But what this example wants to show is "state that follows the card", so B-2 does not hold this way.

**How it was worked around**: `hooks::use_identity(cx, key, init)`. Re-root the scope on the identity with `Cx::new(store, cx.ui(), Id::new(("board/identity", key)))` and call `use_state` on that `Cx`. The returned guard only borrows the store (`'s`), so it survives even after this one-line `Cx` is dropped, and the caller keeps drawing as usual. Sweep and collision detection still work. But the inner `use_state` has one call site, so the key must include not only "whose" but also "what" (`(card.id, "draft")`).

**What to add**: `use_keyed(cx, key, init) -> State<T>` (the position-independent key of `use_persisted`, minus the persistence). `Store::slot` is `pub(crate)`, so the only ways to create a slot with an arbitrary Id from user land today are "re-root with `Cx::new`" or "pass a string key to `use_persisted`". The latter writes to eframe's storage and is serialized and kept in the map at sweep time, so it cannot be used for a temporary value like a draft being edited. Adding to core would be just two things, `Store::keyed_slot` and `use_keyed`, and `use_persisted` could be rewritten on top of them.

### 8.2 `Handle` has no "write without marking dirty"

**What could not be written**: DnD slots are re-registered every frame. `Handle::set` / `update` / `update_later` all `request_repaint`, so calling them every frame means the app never goes idle (ARCHITECTURE 5.6). It keeps burning CPU in the gallery too, and `egui_kittest::Harness::run` fails with `ExceededMaxSteps`.

**How it was worked around**: Put `Rc<RefCell<..>>` on the value side and write through `Handle::with` (which only passes `&T` and does not repaint). This is why `Dnd` is `Clone` and shares its contents. It is not decoration.

**What to add**: A `Handle` version of `State::bind()` (`Handle::with_mut` / `peek_mut`). Same semantics as `bind`: "a widget or per-frame record writes directly, and the value does not change without input". That would avoid pushing interior mutability out to user land.

### 8.3 `leaf_fill` measures "everything", not "the remainder"

**What happened**: `<ScrollArea grow={1.0}>` is `leaf_fill`, so the max-content it reports to taffy is **the height of the root rect itself** (`egui_taffy`'s measure reads `infinite` as the root_rect size). The runner's root item style is `min_h: 100%` with auto height, so putting "toolbar + ScrollArea" in a `<View column>` makes the column height (toolbar + window height), and it overflows the window. Neither `grow` nor `basis={0}` helps. Flex grow / shrink only applies when the parent height is fixed, and nowhere in this chain is there a fixed height. `h="100%"` also falls to auto when the parent is auto.

**How it was worked around**: The column footer (`+ card` and the "drop at the end" zone) was put **at the end inside** the `ScrollArea`, not outside. Only the margin at the bottom of the column overflows, and everything that can be pressed or dropped on stays visible. As UI this is also closer to Trello.

**What to add**: A way to give the runner root "the window height itself". `root_style()`'s `min_h: 100%` exists to "stretch when the contents are tall", but because of it you cannot write "fill the window and give the rest to the ScrollArea". Either add a switch to `Options` to "pin the root to the window height", or give `ScrollArea` an item style meaning "take the remaining height". The `showcase` sidebar and the gallery's left column have the same shape, so those two probably also overflow the window slightly right now.

### 8.4 `<View>` does not return a rect

The grab area, the drop area, and the insert line all need a rect. `<View>` does not return a `Response`, so the card title and the column footer were written with `cx.leaf` / `cx.leaf_fill` (this example also wants to show the escape hatch, so nothing is lost). Making the whole card a drop target needs "the rect after the card has been drawn", and that cannot be obtained today. As a result, the area above the body of an open card is not a drop target. What to add: `#[event] on_rect: egui::Rect` on `<View>` (same nature as `Canvas`'s `paint`).

### 8.5 Small things

- An argument of type `Option<T>` is "an optional prop", so a prop that really wants to receive an `Option` must be `&Option<T>` (same as `selected` in `showcase`). That is `next: &Option<CardId>` and `label: &Option<Label>`.
- `<Toolbar label={&*label} on_label={|..| *label = ..}/>` is E0502 (the second one in ARCHITECTURE 3.7). Copy the value first and use `label={&filter}`. Props staying as borrows is by design, so this only calls for a one-line note.
- `egui::Id::new` requires `Hash + Debug` (`AsId`), so the `use_identity` key is also `Hash + Debug`. Same condition as `rsx!`'s `key=`, so they line up.
- snapshot (`cargo test -p gallery --features snapshot`) does not run in this container (no GPU and no software Vulkan). The `board_react` / `board_plain` images are not generated. The first person to run `UPDATE_SNAPSHOTS=1` on a machine with a GPU will add them.

### 8.6 Line counts do not differ as much as expected

A fairly written plain egui version hardly loses on line count. Excluding comments and blank lines, **react 435 lines / plain 460 lines**. For whole files (which is what the gallery shows), **631 / 582, so the react version is longer** (more prose is the example's job, so that itself is fine). `todo` is also 143 / 143 (code only 112 / 104), so this has been the case in this repo from before.

There is no difference because the plain egui version only has to "draw everything every frame", while the react version writes the `#[component]` signatures (props and events) for 6 components. **This example cannot base its claim on "fewer lines". It should base it on "which lines do what".** That is, the lines the plain egui version has and the react version does not are `ui: HashMap<CardId, CardUi>` and its `retain`, reads and writes via `card_ui(id)`, and the `renaming: Option<(ColumnId, String)>` compromise. All of these come from "the whole holds the parts' state". Conversely, what the react version has and the plain one does not are the prop and event declarations, and those are the price for making components usable elsewhere. task.md's "show it in line counts" did not hold as written, so read the gallery's line count display as showing "same screen, about the same amount of code, different places".

Note that `Theme` and ghost drawing are used by both versions, so they were moved out to `look.rs` (an addition to the file list in plan section 0). Left in `lib.rs`, the shared parts would count only toward the react version's lines and skew the comparison.
