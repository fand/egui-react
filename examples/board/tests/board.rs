//! Plan tests B-1 .. B-13: what a card carries with it when it moves.
//!
//! B-2 and B-3 are the two the example exists for, and both are driven through
//! the UI — pointer down on a card, pointer over another column, pointer up —
//! because "the reducer moved the card" is not the claim. The claim is that the
//! title someone was halfway through typing is still on the card afterwards.
//!
//! Cards are grabbed by their tick box and found by its name. A card whose
//! title has become an editor has no title label left to look up, and the whole
//! card is the drag handle anyway: pressing on the box and moving drags the
//! card, because the background that senses the drag is registered under every
//! widget on it.
//!
//! Every helper is generic over the harness state, so the same script runs
//! against the egui-react version and the plain egui one (B-7).

use board::App;
use board::plain::{self, PlainState};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use egui_react::prelude::*;
use egui_react_app::{root_id, root_style};

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

/// A egui-react harness that starts from saved storage, as a restart would.
fn react_from<'a>(json: &str) -> Harness<'a, Store> {
    let mut store = Store::new();
    store.load_persisted(json);
    steady(
        Harness::builder()
            .with_size(SIZE)
            .build_ui_state(run_app, store),
    )
}

fn plain<'a>() -> Harness<'a, PlainState> {
    plain_from(PlainState::default())
}

/// A frame every sixtieth of a second, for the one test that watches an
/// animation. kittest's default step is a quarter of a second, and egui's
/// animations count the predicted frame time as already elapsed, so at that
/// rate a gap is fully open on the frame it starts opening.
const FRAME: f32 = 1.0 / 60.0;

fn react_at_60fps<'a>() -> Harness<'a, Store> {
    steady(
        Harness::builder()
            .with_size(SIZE)
            .with_step_dt(FRAME)
            .build_ui_state(run_app, Store::new()),
    )
}

fn plain_at_60fps<'a>() -> Harness<'a, PlainState> {
    steady(
        Harness::builder()
            .with_size(SIZE)
            .with_step_dt(FRAME)
            .build_ui_state(
                |ui, state: &mut PlainState| plain::ui(ui, state),
                PlainState::default(),
            ),
    )
}

fn plain_from<'a>(state: PlainState) -> Harness<'a, PlainState> {
    steady(
        Harness::builder()
            .with_size(SIZE)
            .build_ui_state(|ui, state: &mut PlainState| plain::ui(ui, state), state),
    )
}

/// Stop the caret blinking.
///
/// These tests open editors and leave them open, and a focused field asks for
/// a repaint every time the caret is due to change — so `Harness::run`, which
/// runs until the app stops asking, would never finish (ARCHITECTURE 5.6).
/// Nothing here is watching it blink.
fn steady<S>(harness: Harness<'_, S>) -> Harness<'_, S> {
    harness
        .ctx
        .all_styles_mut(|style| style.visuals.text_cursor.blink = false);
    harness
}

/// Two passes: one to apply the write, one to draw with it.
fn settle<S>(harness: &mut Harness<'_, S>) {
    harness.run();
    harness.run();
}

/// A card's tick box, which is the one part of a card that is named the same
/// whether or not its title is being edited.
fn box_of(title: &str) -> String {
    format!("done: {title}")
}

/// Which of the four columns a card is drawn in.
///
/// Both versions divide the width into four equal columns, so where a card is
/// on screen is the honest way to ask which column it is in — and it is the
/// same question for both.
fn column_of<S>(harness: &Harness<'_, S>, title: &str) -> usize {
    let x = harness.get_by_label(&box_of(title)).rect().center().x;
    ((x / SIZE.x * 4.0).floor() as usize).min(3)
}

/// How far down the screen a card is.
fn y_of<S>(harness: &Harness<'_, S>, title: &str) -> f32 {
    harness.get_by_label(&box_of(title)).rect().center().y
}

/// Which half of a card the drop lands on: in front of it, or behind it.
#[derive(Clone, Copy)]
enum Half {
    Above,
    Below,
}

/// Where on the card called `onto` a drop lands in that half.
fn half_of<S>(harness: &Harness<'_, S>, onto: &str, half: Half) -> egui::Pos2 {
    let target = harness.get_by_label(onto).rect();
    match half {
        Half::Above => egui::pos2(target.center().x, target.top() + 1.0),
        Half::Below => egui::pos2(target.center().x, target.bottom() - 1.0),
    }
}

/// Pick a card up by its tick box and drop it on another card.
///
/// Three phases, as a real pointer does it: press, move (which is what makes
/// egui call it a drag rather than a click), release.
fn drag<S>(harness: &mut Harness<'_, S>, title: &str, onto: &str, half: Half) {
    let from = harness.get_by_label(&box_of(title)).rect().center();
    let to = half_of(harness, onto, half);
    drag_to(harness, from, to);
}

/// The same, ending on a column's footer: "at the end of this column".
fn drag_to_end<S>(harness: &mut Harness<'_, S>, title: &str, column: usize) {
    let from = harness.get_by_label(&box_of(title)).rect().center();
    let to = harness
        .get_all_by_label("+ card")
        .nth(column)
        .expect("a footer for every column")
        .rect()
        .center();
    drag_to(harness, from, to);
}

fn drag_to<S>(harness: &mut Harness<'_, S>, from: egui::Pos2, to: egui::Pos2) {
    hold(harness, from, to);
    harness.drop_at(to);
    settle(harness);
    settle(harness);
}

/// The first half of a drag: pick the card up and hold it over `to` without
/// letting go, which is when the gap is open.
fn hold<S>(harness: &mut Harness<'_, S>, from: egui::Pos2, to: egui::Pos2) {
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
}

/// Open a card's editor with the pen, and check that it is ready to type into.
fn open_editor<S>(harness: &mut Harness<'_, S>, title: &str) {
    let y = harness.get_by_label(&box_of(title)).rect().center().y;
    // "edit" is on every card; the one on this card's row is the one whose
    // rectangle sits beside it.
    harness
        .get_all_by_label("edit")
        .min_by(|a, b| {
            let a = (a.rect().center().y - y).abs();
            let b = (b.rect().center().y - y).abs();
            a.total_cmp(&b)
        })
        .expect("an edit button on that card")
        .click();
    settle(harness);
    assert!(
        draft_field(harness).is_focused(),
        "the editor opens ready to type into"
    );
}

/// Open a card's editor and add `text` to the end of what is already there.
fn edit<S>(harness: &mut Harness<'_, S>, title: &str, text: &str) {
    open_editor(harness, title);
    // The field opens with everything selected, so a key that moves the caret
    // is what turns typing from "replace it" into "add to it".
    harness.key_press(egui::Key::End);
    draft_field(harness).type_text(text);
    settle(harness);
}

/// The title field of the card that is being edited.
fn draft_field<'h, S>(harness: &'h Harness<'_, S>) -> egui_kittest::Node<'h> {
    harness.get_by_label("title")
}

/// What the open editor's title field says.
fn draft<S>(harness: &Harness<'_, S>) -> Option<String> {
    harness.query_by_label("title")?.accesskit_node().value()
}

/// How many cards are open for editing.
fn editors<S>(harness: &Harness<'_, S>) -> usize {
    harness.query_all_by_label("title").count()
}

/// Whether a card's box is ticked.
fn ticked<S>(harness: &Harness<'_, S>, title: &str) -> Option<egui::accesskit::Toggled> {
    harness
        .get_by_label(&box_of(title))
        .accesskit_node()
        .toggled()
}

/// The toolbar's chip of that name. The last column is called "done" too, and
/// the toolbar is drawn first.
fn chip<S>(harness: &Harness<'_, S>, name: &str) {
    harness
        .get_all_by_label(name)
        .next()
        .expect("a filter chip")
        .click();
}

/// Add a card to the first column the way a user does: the footer opens an
/// empty editor, and what is typed into it is the card.
fn add_card<S>(harness: &mut Harness<'_, S>, title: &str) {
    harness
        .get_all_by_label("+ card")
        .next()
        .expect("the backlog footer")
        .click();
    settle(harness);
    assert!(
        harness.get_by_label("new card").is_focused(),
        "the new card opens ready to type into"
    );
    harness.get_by_label("new card").type_text(title);
    settle(harness);
    harness.key_press(egui::Key::Enter);
    settle(harness);
}

/// B-1: the footer adds a card to its column, and only if it was given a name.
fn adding_a_card<S>(harness: &mut Harness<'_, S>) {
    assert!(
        harness.query_by_label("12 cards").is_some(),
        "twelve to start"
    );
    assert!(
        harness.query_by_label("4/4").is_some(),
        "backlog holds four"
    );

    add_card(harness, "new card");

    assert!(harness.query_by_label("13 cards").is_some());
    assert!(
        harness.query_by_label("5/5").is_some(),
        "backlog holds five"
    );
    assert_eq!(column_of(harness, "new card"), 0);

    // The editor is the card: cancel it and there is nothing to keep.
    harness
        .get_all_by_label("+ card")
        .next()
        .expect("the backlog footer")
        .click();
    settle(harness);
    harness.key_press(egui::Key::Escape);
    settle(harness);
    assert!(
        harness.query_by_label("13 cards").is_some(),
        "a cancelled card was never added"
    );

    // And confirming an empty one adds nothing either.
    harness
        .get_all_by_label("+ card")
        .next()
        .expect("the backlog footer")
        .click();
    settle(harness);
    harness.key_press(egui::Key::Enter);
    settle(harness);
    assert!(
        harness.query_by_label("13 cards").is_some(),
        "and neither was a nameless one"
    );
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
    // The tick box is named after what the board says, not what is being typed.
    assert!(harness.query_by_label(&box_of("buy milk")).is_some());
}

/// B-3: reordering a column moves the state with the cards, not with the
/// positions they were in.
fn a_draft_survives_a_reorder<S>(harness: &mut Harness<'_, S>) {
    edit(harness, "read the plan", "!");
    assert_eq!(draft(harness).as_deref(), Some("read the plan!"));

    drag(harness, "read the plan", "write the plan", Half::Above);

    assert!(
        y_of(harness, "read the plan") < y_of(harness, "write the plan"),
        "the second card is now the first"
    );
    assert_eq!(editors(harness), 1, "one card is open, as before");
    assert_eq!(
        draft(harness).as_deref(),
        Some("read the plan!"),
        "and it is the same card, still holding the same draft"
    );
    assert!(
        harness.query_by_label("write the plan").is_some(),
        "the card it swapped with did not inherit the editor"
    );

    // And back: dropped on the lower half of a card, it lands behind it.
    drag(harness, "read the plan", "write the plan", Half::Below);

    assert!(
        y_of(harness, "read the plan") > y_of(harness, "write the plan"),
        "the first card is the second again"
    );
    assert_eq!(draft(harness).as_deref(), Some("read the plan!"));
}

/// B-4: undo and redo walk the history one user action at a time.
fn undo_and_redo<S>(harness: &mut Harness<'_, S>) {
    add_card(harness, "new card");
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
/// like any other. Escape puts it back.
fn renaming_a_column<S>(harness: &mut Harness<'_, S>) {
    let rename = |harness: &mut Harness<'_, S>| {
        harness
            .get_all_by_label("rename")
            .next()
            .expect("the backlog's rename button")
            .click();
        settle(harness);
    };

    rename(harness);
    assert!(harness.get_by_label("column name").is_focused());
    harness.key_press(egui::Key::End);
    harness
        .get_by_label("column name")
        .type_text(" (was backlog)");
    settle(harness);
    harness.key_press(egui::Key::Enter);
    settle(harness);
    settle(harness);

    assert!(harness.query_by_label("backlog (was backlog)").is_some());
    // And it is one step of the history, like everything else.
    harness.get_by_label("undo").click();
    settle(harness);
    assert!(harness.query_by_label("backlog").is_some());

    // Opened and escaped, the name is what it was and the field is gone.
    rename(harness);
    harness.get_by_label("column name").type_text("nonsense");
    settle(harness);
    harness.key_press(egui::Key::Escape);
    settle(harness);
    assert_eq!(harness.query_all_by_label("column name").count(), 0);
    assert!(harness.query_by_label("backlog").is_some());
}

/// B-9: the pen edits the title in place — selected on open, Enter to keep it,
/// Escape to put it back.
fn editing_a_title<S>(harness: &mut Harness<'_, S>) {
    open_editor(harness, "buy milk");
    // Everything is selected, so the first key replaces the lot.
    draft_field(harness).type_text("z");
    settle(harness);
    assert_eq!(draft(harness).as_deref(), Some("z"));

    harness.key_press(egui::Key::Escape);
    settle(harness);
    assert_eq!(editors(harness), 0, "the editor closed");
    assert!(
        harness.query_by_label("buy milk").is_some(),
        "and the title is what it was"
    );

    edit(harness, "buy milk", "!");
    harness.key_press(egui::Key::Enter);
    settle(harness);
    assert_eq!(editors(harness), 0);
    assert!(harness.query_by_label("buy milk!").is_some());

    harness.get_by_label("undo").click();
    settle(harness);
    assert!(
        harness.query_by_label("buy milk").is_some(),
        "one edit, one step of the history"
    );
}

/// B-10: the tick box marks a card done, and the toolbar filters on it.
fn ticking_a_card<S>(harness: &mut Harness<'_, S>) {
    use egui::accesskit::Toggled;

    assert_eq!(ticked(harness, "buy milk"), Some(Toggled::False));
    harness.get_by_label(&box_of("buy milk")).click();
    settle(harness);
    assert_eq!(ticked(harness, "buy milk"), Some(Toggled::True));
    harness.get_by_label("undo").click();
    settle(harness);
    assert_eq!(
        ticked(harness, "buy milk"),
        Some(Toggled::False),
        "one tick, one step of the history"
    );

    chip(harness, "done");
    settle(harness);
    assert!(
        harness.query_by_label("3/3").is_some(),
        "the done column is all done"
    );
    assert!(
        harness.query_by_label("0/4").is_some(),
        "and the backlog is none of it"
    );

    // Pressing the chip that is already on clears the filter.
    chip(harness, "done");
    settle(harness);
    assert!(harness.query_by_label("4/4").is_some());
}

/// B-11: a card held over another one opens a gap where it would land, and the
/// cards below it move down to make room.
fn a_held_card_opens_a_gap<S>(harness: &mut Harness<'_, S>) {
    let before = y_of(harness, "name the hooks");
    let from = harness.get_by_label(&box_of("buy milk")).rect().center();
    let to = half_of(harness, "wire the drag", Half::Below);
    hold(harness, from, to);

    assert!(
        harness.query_by_label("drop here").is_some(),
        "the gap is open while the card is in hand"
    );
    assert!(
        y_of(harness, "name the hooks") > before,
        "and the cards after it made room"
    );

    harness.drop_at(to);
    settle(harness);
    settle(harness);
    assert!(
        harness.query_by_label("drop here").is_none(),
        "the gap closes when the card is let go"
    );
    assert_eq!(column_of(harness, "buy milk"), 1);
}

/// B-12: dragging across the board does not select the text it passes over.
fn dragging_selects_no_text<S>(harness: &mut Harness<'_, S>) {
    drag(harness, "buy milk", "wire the drag", Half::Below);
    assert!(
        !harness
            .ctx
            .plugin::<egui::text_selection::LabelSelectionState>()
            .lock()
            .has_selection(),
        "a card's title is not selectable, so a drag over it selects nothing"
    );
}

/// B-13: the gap slides open without the column being laid out twice a frame.
///
/// The animation is in the picture, not in the layout: a taffy node whose
/// height moved would make egui_taffy relayout and ask egui for a second pass,
/// every frame the gap was moving, which egui flags as a performance bug on
/// screen. So while the gap is opening, each frame is one pass — except the
/// frame the layout itself changes, which is one relayout and is allowed.
///
/// Driven a frame at a time on a pinned clock, on a harness that steps at
/// sixty frames a second: `Harness::run` would stop when the animation stops
/// asking for frames, which is after it is over.
fn a_gap_opens_in_one_pass<S>(harness: &mut Harness<'_, S>) {
    let mut now = 0.0;
    let mut frame = |harness: &mut Harness<'_, S>| {
        now += f64::from(FRAME);
        harness.input_mut().time = Some(now);
        harness.step();
        harness.output().platform_output.num_completed_passes
    };

    let from = harness.get_by_label(&box_of("buy milk")).rect().center();
    let to = half_of(harness, "wire the drag", Half::Below);
    harness.hover_at(from);
    frame(harness);
    harness.drag_at(from);
    frame(harness);
    // The first move is the one egui decides is a drag, the second offers the
    // slot, the third is the frame the gap opens in the layout.
    for _ in 0..3 {
        harness.hover_at(to);
        frame(harness);
    }
    assert!(harness.query_by_label("drop here").is_some());

    // The rest of the opening, and some frames after it, one pass each.
    let second_passes = (0..12)
        .filter(|_| {
            harness.hover_at(to);
            frame(harness) > 1
        })
        .count();
    assert_eq!(
        second_passes, 0,
        "frames took a second pass while the gap was sliding open"
    );

    harness.drop_at(to);
    settle(harness);
    assert_eq!(column_of(harness, "buy milk"), 1);
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
fn b3_react_keeps_a_draft_through_a_reorder() {
    let mut harness = react();
    harness.run();
    a_draft_survives_a_reorder(&mut harness);
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
    searching(&mut harness, past_the_debounce);
}

/// Past the debounce. The clock is pinned so that the test settles when it says
/// so and not when the machine happens to be slow.
fn past_the_debounce<S>(harness: &mut Harness<'_, S>) {
    harness.input_mut().time = Some(10.0);
    harness.step();
    harness.step();
}

/// B-6: the board is saved and restored; the drafts are not, and should not be.
#[test]
fn b6_react_keeps_the_board_across_a_restart() {
    let mut harness = react();
    harness.run();
    add_card(&mut harness, "new card");
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
fn b9_react_edits_a_title() {
    let mut harness = react();
    harness.run();
    editing_a_title(&mut harness);
}

#[test]
fn b10_react_ticks_a_card() {
    let mut harness = react();
    harness.run();
    ticking_a_card(&mut harness);
}

#[test]
fn b11_react_opens_a_gap() {
    let mut harness = react();
    harness.run();
    a_held_card_opens_a_gap(&mut harness);
}

#[test]
fn b12_react_selects_no_text_while_dragging() {
    let mut harness = react();
    harness.run();
    dragging_selects_no_text(&mut harness);
}

#[test]
fn b13_react_opens_a_gap_in_one_pass() {
    let mut harness = react_at_60fps();
    harness.run();
    a_gap_opens_in_one_pass(&mut harness);
}

/// B-7: the plain egui version answers every one of them the same way.
///
/// A fresh harness per script, because each starts from the demo board.
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
    a_draft_survives_a_reorder(&mut harness);

    let mut harness = plain();
    harness.run();
    undo_and_redo(&mut harness);

    let mut harness = plain();
    harness.input_mut().time = Some(0.0);
    harness.run();
    searching(&mut harness, past_the_debounce);

    let mut harness = plain();
    harness.run();
    renaming_a_column(&mut harness);

    let mut harness = plain();
    harness.run();
    editing_a_title(&mut harness);

    let mut harness = plain();
    harness.run();
    ticking_a_card(&mut harness);

    let mut harness = plain();
    harness.run();
    a_held_card_opens_a_gap(&mut harness);

    let mut harness = plain();
    harness.run();
    dragging_selects_no_text(&mut harness);

    let mut harness = plain_at_60fps();
    harness.run();
    a_gap_opens_in_one_pass(&mut harness);

    // B-6 for this side: the board is a `Board` either way, so a restart is
    // the same round trip through the same JSON.
    let mut harness = plain();
    harness.run();
    add_card(&mut harness, "new card");
    let saved = harness.state().save();
    assert!(saved.contains("new card"), "saved: {saved}");

    let mut restarted = plain_from(PlainState::load(&saved));
    restarted.run();
    assert!(restarted.query_by_label("13 cards").is_some());
    assert_eq!(column_of(&restarted, "new card"), 0);
}
