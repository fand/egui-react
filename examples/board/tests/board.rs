//! Plan tests B-1 .. B-7: what a card carries with it when it moves.
//!
//! B-2 and B-3 are the two the example exists for, and both are driven through
//! the UI — pointer down on a card's title, pointer over another column,
//! pointer up — because "the reducer moved the card" is not the claim. The
//! claim is that the draft someone was typing is still on the card afterwards.
//!
//! Every helper is generic over the harness state, so the same script runs
//! against the react-egui version and the plain egui one (B-7).

use board::App;
use board::plain::{self, PlainState};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use react_egui::prelude::*;
use react_egui_app::{root_id, root_style};

/// Wide enough for four columns of cards, tall enough that no column scrolls:
/// a `ScrollArea` culls what it does not draw, and a culled widget is not in
/// the tree to be found.
const SIZE: egui::Vec2 = egui::vec2(1000.0, 700.0);

/// The runner's frame, minus eframe.
fn run_app(ui: &mut egui::Ui, store: &mut Store) {
    store.begin_pass(ui.ctx());
    {
        let store: &Store = store;
        let mut cx = Cx::new(store, ui, root_id());
        let view = rsx! { <App/> };
        cx.root_container(root_id(), root_style(), |cx| view.show(cx));
    }
    store.end_pass();
}

fn react<'a>() -> Harness<'a, Store> {
    react_from("{}")
}

/// A react-egui harness that starts from saved storage, as a restart would.
fn react_from<'a>(json: &str) -> Harness<'a, Store> {
    let mut store = Store::new();
    store.load_persisted(json);
    Harness::builder()
        .with_size(SIZE)
        .build_ui_state(run_app, store)
}

fn plain<'a>() -> Harness<'a, PlainState> {
    Harness::builder().with_size(SIZE).build_ui_state(
        |ui, state: &mut PlainState| plain::ui(ui, state),
        PlainState::default(),
    )
}

/// Two passes: one to apply the write, one to draw with it.
fn settle<S>(harness: &mut Harness<'_, S>) {
    harness.run();
    harness.run();
}

/// Which of the four columns a card is drawn in.
///
/// Both versions divide the width into four equal columns, so where a card is
/// on screen is the honest way to ask which column it is in — and it is the
/// same question for both.
fn column_of<S>(harness: &Harness<'_, S>, title: &str) -> usize {
    let x = harness.get_by_label(title).rect().center().x;
    ((x / SIZE.x * 4.0).floor() as usize).min(3)
}

/// How far down the screen a card is.
fn y_of<S>(harness: &Harness<'_, S>, title: &str) -> f32 {
    harness.get_by_label(title).rect().center().y
}

/// Which half of a card the drop lands on: in front of it, or behind it.
#[derive(Clone, Copy)]
enum Half {
    Above,
    Below,
}

/// Pick a card up by its title and drop it on another card's.
///
/// Three phases, as a real pointer does it: press, move (which is what makes
/// egui call it a drag rather than a click), release.
fn drag<S>(harness: &mut Harness<'_, S>, title: &str, onto: &str, half: Half) {
    let from = harness.get_by_label(title).rect().center();
    let target = harness.get_by_label(onto).rect();
    let to = match half {
        Half::Above => egui::pos2(target.center().x, target.top() + 1.0),
        Half::Below => egui::pos2(target.center().x, target.bottom() - 1.0),
    };
    drag_to(harness, from, to);
}

/// The same, ending on a column's footer: "at the end of this column".
fn drag_to_end<S>(harness: &mut Harness<'_, S>, title: &str, column: usize) {
    let from = harness.get_by_label(title).rect().center();
    let to = harness
        .get_all_by_label("+ card")
        .nth(column)
        .expect("a footer for every column")
        .rect()
        .center();
    drag_to(harness, from, to);
}

fn drag_to<S>(harness: &mut Harness<'_, S>, from: egui::Pos2, to: egui::Pos2) {
    harness.hover_at(from);
    harness.run();
    harness.drag_at(from);
    harness.run();
    // Twice: the first move is the one egui decides is a drag, the second is
    // the frame that offers the slot under the pointer to the drag session.
    harness.hover_at(to);
    harness.run();
    harness.hover_at(to);
    harness.run();
    harness.drop_at(to);
    settle(harness);
    settle(harness);
}

/// Open a card's editor and type `text` into the title field.
fn edit<S>(harness: &mut Harness<'_, S>, title: &str, text: &str) {
    open_editor(harness, title);
    // The editor is the only thing on screen with a "save" next to it, so its
    // title field is the one that comes before that.
    draft_field(harness).focus();
    harness.run();
    draft_field(harness).type_text(text);
    settle(harness);
}

fn open_editor<S>(harness: &mut Harness<'_, S>, title: &str) {
    let card = harness.get_by_label(title).rect();
    // "edit" is on every card; the one on this card's row is the one whose
    // rectangle sits beside it.
    let button = harness
        .get_all_by_label("edit")
        .find(|node| (node.rect().center().y - card.center().y).abs() < 4.0)
        .expect("an edit button on that card")
        .rect()
        .center();
    harness.hover_at(button);
    harness.run();
    harness.get_all_by_label("edit").for_each(drop);
    click_at(harness, button);
}

fn click_at<S>(harness: &mut Harness<'_, S>, at: egui::Pos2) {
    harness.hover_at(at);
    harness.run();
    harness.drag_at(at);
    harness.run();
    harness.drop_at(at);
    settle(harness);
}

/// The title field of the card that is being edited, if any.
fn draft_field<'h, S>(harness: &'h Harness<'_, S>) -> egui_kittest::Node<'h> {
    // Single-line fields, in tree order: the search box, a column's rename box
    // if one is open, then the open card's title. (Its body is a multiline
    // field, which is a role of its own.) So the last one is the draft title.
    let inputs: Vec<_> = harness
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .collect();
    inputs.into_iter().last().expect("the draft title field")
}

/// What the open editor's title field says.
fn draft<S>(harness: &Harness<'_, S>) -> Option<String> {
    harness
        .query_by_label("save")
        .is_some()
        .then(|| draft_field(harness).accesskit_node().value())
        .flatten()
}

/// How many cards are open for editing.
fn editors<S>(harness: &Harness<'_, S>) -> usize {
    harness.query_all_by_label("save").count()
}

/// B-1: a column's footer adds a card to that column.
fn adding_a_card<S>(harness: &mut Harness<'_, S>) {
    assert!(
        harness.query_by_label("12 cards").is_some(),
        "twelve to start"
    );
    assert!(
        harness.query_by_label("4/4").is_some(),
        "backlog holds four"
    );

    harness
        .get_all_by_label("+ card")
        .next()
        .expect("the backlog footer")
        .click();
    settle(harness);

    assert!(harness.query_by_label("13 cards").is_some());
    assert!(
        harness.query_by_label("5/5").is_some(),
        "backlog holds five"
    );
    assert_eq!(column_of(harness, "new card"), 0);
}

/// B-2: the draft of a card that is being edited follows it to another column.
fn a_draft_follows_its_card<S>(harness: &mut Harness<'_, S>) {
    edit(harness, "buy milk", " and bread");
    assert_eq!(draft(harness).as_deref(), Some("buy milk and bread"));
    assert_eq!(column_of(harness, "buy milk"), 0);

    // Into "doing", in front of the card that is already there.
    drag(harness, "buy milk", "wire the drag", Half::Above);

    assert_eq!(column_of(harness, "buy milk"), 1, "the card moved");
    assert!(
        y_of(harness, "buy milk") < y_of(harness, "wire the drag"),
        "and landed in front of the card it was dropped on"
    );
    assert_eq!(
        draft(harness).as_deref(),
        Some("buy milk and bread"),
        "the draft came with it"
    );
    assert_eq!(
        editors(harness),
        1,
        "and nothing else was opened for editing"
    );
    // The saved title is untouched: what moved is the editor, not an edit.
    assert!(harness.query_by_label("buy milk").is_some());
}

/// B-3: reordering a column moves the state with the cards, not with the
/// positions they were in.
fn expanding_survives_a_reorder<S>(harness: &mut Harness<'_, S>) {
    // The second card in the backlog, opened to show its body.
    harness.get_by_label("read the plan").click();
    settle(harness);
    assert!(
        harness
            .query_by_label("twice, then argue with it")
            .is_some()
    );

    drag(harness, "read the plan", "write the plan", Half::Above);

    assert!(
        y_of(harness, "read the plan") < y_of(harness, "write the plan"),
        "the second card is now the first"
    );
    assert!(
        harness
            .query_by_label("twice, then argue with it")
            .is_some(),
        "the card that was open is still open"
    );
    assert!(
        harness
            .query_by_label("what the example proves, and how")
            .is_none(),
        "and the card it swapped with did not inherit it"
    );

    // And back: dropped on the lower half of a card, it lands behind it.
    drag(harness, "read the plan", "write the plan", Half::Below);

    assert!(
        y_of(harness, "read the plan") > y_of(harness, "write the plan"),
        "the first card is the second again"
    );
    assert!(
        harness
            .query_by_label("twice, then argue with it")
            .is_some(),
        "still open, two moves later"
    );
}

/// B-4: undo and redo walk the history one user action at a time.
fn undo_and_redo<S>(harness: &mut Harness<'_, S>) {
    harness
        .get_all_by_label("+ card")
        .next()
        .expect("the backlog footer")
        .click();
    settle(harness);
    drag_to_end(harness, "buy milk", 3);
    assert!(harness.query_by_label("13 cards").is_some());
    assert_eq!(column_of(harness, "buy milk"), 3);

    harness.get_by_label("undo").click();
    settle(harness);
    assert_eq!(column_of(harness, "buy milk"), 0, "the drag is undone");
    assert!(harness.query_by_label("13 cards").is_some());

    harness.get_by_label("undo").click();
    settle(harness);
    assert!(harness.query_by_label("12 cards").is_some(), "and the add");

    harness.get_by_label("redo").click();
    settle(harness);
    assert!(harness.query_by_label("13 cards").is_some());
    harness.get_by_label("redo").click();
    settle(harness);
    assert_eq!(column_of(harness, "buy milk"), 3, "and back again");
}

/// B-5: the search box filters every column, once it has been still for long
/// enough.
fn searching<S>(harness: &mut Harness<'_, S>, mut wait: impl FnMut(&mut Harness<'_, S>)) {
    let search = || egui::accesskit::Role::TextInput;
    harness.get_all_by_role(search()).next().unwrap().focus();
    harness.run();
    harness
        .get_all_by_role(search())
        .next()
        .unwrap()
        .type_text("plan");
    harness.run();

    // Still inside the delay: the box has the text, the columns do not know.
    assert!(harness.query_by_label("4/4").is_some(), "not yet filtered");

    wait(harness);

    assert!(
        harness.query_by_label("2/4").is_some(),
        "backlog: two of four"
    );
    assert!(
        harness.query_by_label("0/2").is_some(),
        "review: none of two"
    );
    assert!(harness.query_by_label("write the plan").is_some());
    assert!(harness.query_by_label("buy milk").is_none());
    assert_eq!(harness.query_all_by_label("nothing here").count(), 3);
}

/// B-8: a column's name is edited in place, and the edit is a board message
/// like any other.
fn renaming_a_column<S>(harness: &mut Harness<'_, S>) {
    harness
        .get_all_by_label("rename")
        .next()
        .expect("the backlog's rename button")
        .click();
    settle(harness);

    // Two fields on screen now: the search box, then the rename box.
    fn rename<'h, S>(harness: &'h Harness<'_, S>) -> egui_kittest::Node<'h> {
        harness
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .nth(1)
            .expect("the rename field")
    }
    rename(harness).focus();
    harness.run();
    rename(harness).type_text(" (was backlog)");
    harness.run();
    harness.key_press(egui::Key::Enter);
    settle(harness);
    settle(harness);

    assert!(harness.query_by_label("backlog (was backlog)").is_some());
    // And it is one step of the history, like everything else.
    harness.get_by_label("undo").click();
    settle(harness);
    assert!(harness.query_by_label("backlog").is_some());
}

#[test]
fn b1_react_adds_a_card() {
    let mut harness = react();
    harness.run();
    adding_a_card(&mut harness);
}

#[test]
fn b2_react_carries_a_draft_across_a_move() {
    let mut harness = react();
    harness.run();
    a_draft_follows_its_card(&mut harness);
}

#[test]
fn b3_react_keeps_expansion_with_the_card() {
    let mut harness = react();
    harness.run();
    expanding_survives_a_reorder(&mut harness);
}

#[test]
fn b4_react_undoes_and_redoes() {
    let mut harness = react();
    harness.run();
    undo_and_redo(&mut harness);
}

#[test]
fn b5_react_searches() {
    let mut harness = react();
    harness.input_mut().time = Some(0.0);
    harness.run();
    searching(&mut harness, |harness| {
        // Past the debounce. The clock is pinned so that the test settles when
        // it says so and not when the machine happens to be slow.
        harness.input_mut().time = Some(10.0);
        harness.step();
        harness.step();
    });
}

/// B-6: the board is saved and restored; the drafts are not, and should not be.
#[test]
fn b6_react_keeps_the_board_across_a_restart() {
    let mut harness = react();
    harness.run();
    harness
        .get_all_by_label("+ card")
        .next()
        .expect("the backlog footer")
        .click();
    settle(&mut harness);
    edit(&mut harness, "new card", "!");
    assert!(harness.query_by_label("13 cards").is_some());

    let saved = harness.state().save_persisted();
    assert!(saved.contains("new card"), "saved: {saved}");

    let mut restarted = react_from(&saved);
    restarted.run();
    assert!(restarted.query_by_label("13 cards").is_some());
    assert_eq!(column_of(&restarted, "new card"), 0);
    assert_eq!(editors(&restarted), 0, "an editor is not part of the board");
}

#[test]
fn b8_react_renames_a_column() {
    let mut harness = react();
    harness.run();
    renaming_a_column(&mut harness);
}

#[test]
fn b7_plain_does_all_of_it_the_same_way() {
    let mut harness = plain();
    harness.run();
    adding_a_card(&mut harness);

    let mut harness = plain();
    harness.run();
    a_draft_follows_its_card(&mut harness);

    let mut harness = plain();
    harness.run();
    expanding_survives_a_reorder(&mut harness);

    let mut harness = plain();
    harness.run();
    undo_and_redo(&mut harness);

    let mut harness = plain();
    harness.input_mut().time = Some(0.0);
    harness.run();
    searching(&mut harness, |harness| {
        harness.input_mut().time = Some(10.0);
        harness.step();
        harness.step();
    });

    let mut harness = plain();
    harness.run();
    renaming_a_column(&mut harness);
}
