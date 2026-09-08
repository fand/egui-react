//! Plan tests S-1 .. S-10: what the grid does, driven entirely through the UI.
//!
//! Two of them are why the example exists.
//!
//! **S-4** reads the two counters out of the status bar and shows the memo
//! stages coming apart: typing a number into a literal cell re-runs the
//! evaluation and *not* the parse, typing a formula re-runs both. Nothing in
//! the screen watches for that; it falls out of which revision counter
//! `sheet::reduce` bumps and which one each `use_memo` hashes.
//!
//! **S-7** is the other half. A `<VirtualList>` row that scrolls out of view is
//! unmounted and its hooks are swept, so a half-typed formula would be lost if
//! the draft lived in the cell. It lives in the reducer above the list, and
//! this test scrolls the row away, checks that the cell really is gone from the
//! tree, scrolls back, and finds the edit still in progress.
//!
//! Headless is enough for all ten. Everything these scenarios claim is a value,
//! a name or a rectangle: cell values are on the cells' accessibility nodes,
//! the selection is the name box's placeholder, the counters are in the status
//! text, and a column width is a cell's rect. Nothing here is about colour —
//! the tint, the flash and the one-pixel grid have no assertion and no
//! snapshot, by design (the plan's "the look stays plain").
//!
//! **Every scenario works below row 14.** The preset fills `A1:H12`, and it is
//! loaded when the persisted sheet is empty, so a test that used `A1` would be
//! arguing with it. Rows 15 and down are blank in the preset and on screen, so
//! nothing has to be cleared first.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use egui_react::prelude::*;
use egui_react_app::{root_id, root_style};
use spreadsheet::{App, ROW_H};

/// Wide enough for the row gutter and eight columns, tall enough for a bit
/// over twenty rows — so `A15` is on screen without scrolling.
const SIZE: egui::Vec2 = egui::vec2(760.0, 640.0);

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

fn harness() -> Harness<'static, Store> {
    Harness::builder()
        .with_size(SIZE)
        .build_ui_state(run_app, Store::new())
}

/// A few passes: one for the reducer to apply what was sent, one to draw with
/// it, and a couple more for the second reducer downstream of the first.
fn settle(harness: &mut Harness<'static, Store>) {
    for _ in 0..6 {
        harness.step();
    }
}

/// A harness with the preset loaded and settled.
fn loaded() -> Harness<'static, Store> {
    let mut harness = harness();
    settle(&mut harness);
    harness
}

fn click(harness: &mut Harness<'static, Store>, cell: &str) {
    harness.get_by_label(cell).click();
    settle(harness);
}

/// One `Event::Text` per character, a frame apart.
///
/// The editor is opened by the reducer, which applies on the next visit, so
/// the first character lands in the grid's key handler and the rest land in
/// the field it opened. A burst inside one frame is not lost either (they are
/// concatenated into the initial draft), but typing a character a frame is what
/// a person does and what this file wants to exercise.
fn type_text(harness: &mut Harness<'static, Store>, text: &str) {
    for c in text.chars() {
        harness
            .input_mut()
            .events
            .push(egui::Event::Text(c.to_string()));
        settle(harness);
    }
}

fn key(harness: &mut Harness<'static, Store>, key: egui::Key, modifiers: egui::Modifiers) {
    harness.input_mut().events.push(egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    });
    settle(harness);
}

fn press(harness: &mut Harness<'static, Store>, k: egui::Key) {
    key(harness, k, egui::Modifiers::NONE);
}

/// What a cell shows, from its accessibility node's value.
///
/// The text is painted rather than drawn as a widget, so the cell puts the
/// value it shows on its node next to its name; see `Cell` in `lib.rs`.
fn cell_value(harness: &Harness<'static, Store>, cell: &str) -> String {
    harness
        .get_by_label(cell)
        .accesskit_node()
        .value()
        .unwrap_or_default()
}

/// What the name box says the selection is, from its placeholder.
fn name_box(harness: &Harness<'static, Store>) -> String {
    harness
        .root()
        .children_recursive()
        .find_map(|node| {
            let node = node.accesskit_node();
            (node.role() == egui::accesskit::Role::TextInput)
                .then(|| node.placeholder().map(str::to_owned))
                .flatten()
        })
        .expect("the name box's placeholder")
}

/// Every string the accessibility tree carries as a value, in tree order.
fn values(harness: &Harness<'static, Store>) -> Vec<String> {
    harness
        .root()
        .children_recursive()
        .filter_map(|node| node.accesskit_node().value())
        .collect()
}

/// `(compiled, evaluated)`, out of the status line.
fn status(harness: &Harness<'static, Store>) -> (u32, u32) {
    let line = values(harness)
        .into_iter()
        .find(|text| text.contains("compiled "))
        .expect("the status line");
    let number = |after: &str| -> u32 {
        line.split(after)
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|word| word.parse().ok())
            .unwrap_or_else(|| panic!("no number after {after:?} in {line:?}"))
    };
    (number("compiled "), number("evaluated "))
}

/// The line that says what the cursor cell is made of.
fn inspector(harness: &Harness<'static, Store>) -> String {
    values(harness)
        .into_iter()
        .find(|text| text.contains(" = ") || text.contains(" is empty"))
        .expect("the inspector line")
}

fn enabled(harness: &Harness<'static, Store>, label: &str) -> bool {
    !harness.get_by_label(label).accesskit_node().is_disabled()
}

/// Scroll the body by `points`, positive downwards.
///
/// Wheel events rather than `Node::scroll_down`, so the distance is a number
/// this file picks; `TouchPhase::Start` turns off egui's smoothing, so one call
/// moves exactly this far. Copied from
/// `crates/egui-react-elements/tests/virtual_list.rs`, with the pointer put
/// inside the body first.
fn scroll(harness: &mut Harness<'static, Store>, points: f32) {
    {
        let input = harness.input_mut();
        input
            .events
            .push(egui::Event::PointerMoved(egui::pos2(400.0, 400.0)));
        for (phase, delta) in [
            (egui::TouchPhase::Start, egui::Vec2::ZERO),
            (egui::TouchPhase::Move, egui::vec2(0.0, -points)),
        ] {
            input.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase,
                delta,
                modifiers: egui::Modifiers::NONE,
            });
        }
    }
    settle(harness);
}

/// Write `text` into `cell` and leave the cursor where Enter put it.
fn write(harness: &mut Harness<'static, Store>, cell: &str, text: &str) {
    click(harness, cell);
    type_text(harness, text);
    press(harness, egui::Key::Enter);
}

// ---------------------------------------------------------------------------
// S-1 .. S-3: the sheet
// ---------------------------------------------------------------------------

/// S-1: a number is a number, anything else is text, and Delete empties.
#[test]
fn typing_a_number_a_word_and_deleting_them_again() {
    let mut harness = loaded();

    write(&mut harness, "A15", "12");
    assert_eq!(cell_value(&harness, "A15"), "12");

    write(&mut harness, "B15", "hello");
    assert_eq!(cell_value(&harness, "B15"), "hello");

    click(&mut harness, "A15");
    press(&mut harness, egui::Key::Delete);
    assert_eq!(cell_value(&harness, "A15"), "");
    assert_eq!(
        cell_value(&harness, "B15"),
        "hello",
        "and only the selection"
    );
}

/// S-2: a formula shows its result, and follows the cells it reads.
#[test]
fn a_formula_follows_the_cells_it_reads() {
    let mut harness = loaded();

    write(&mut harness, "A15", "2");
    write(&mut harness, "B15", "3");
    write(&mut harness, "C15", "=A15*B15");
    assert_eq!(cell_value(&harness, "C15"), "6");

    write(&mut harness, "A15", "4");
    assert_eq!(cell_value(&harness, "C15"), "12");
}

/// S-3: a cycle says so in both cells, leaves the rest of the sheet alone, and
/// clears the moment one end of it stops being a formula.
#[test]
fn a_cycle_is_reported_in_both_cells_and_clears_when_it_is_broken() {
    let mut harness = loaded();

    write(&mut harness, "A17", "=B17");
    write(&mut harness, "B17", "=A17");
    assert_eq!(cell_value(&harness, "A17"), "#CYCLE!");
    assert_eq!(cell_value(&harness, "B17"), "#CYCLE!");

    write(&mut harness, "C17", "=1+1");
    assert_eq!(
        cell_value(&harness, "C17"),
        "2",
        "a cycle is not contagious"
    );
    assert_eq!(
        cell_value(&harness, "H2"),
        "7350",
        "and the preset above it is untouched"
    );

    write(&mut harness, "B17", "5");
    assert_eq!(cell_value(&harness, "A17"), "5");
    assert_eq!(cell_value(&harness, "B17"), "5");
}

// ---------------------------------------------------------------------------
// S-4: the highlight
// ---------------------------------------------------------------------------

/// S-4: a literal edit re-runs the evaluation and not the parse; a formula
/// re-runs both.
///
/// This is the example's thesis, read off the status bar. `compiled` is the
/// memo keyed on `structure_rev`, `evaluated` the one keyed on `value_rev`, and
/// the only thing that decides which moves is which counter `sheet::reduce`
/// bumped.
#[test]
fn a_literal_edit_evaluates_without_recompiling() {
    let mut harness = loaded();
    let (compiled, evaluated) = status(&harness);

    // A number into an empty cell: nothing to parse.
    write(&mut harness, "A15", "1");
    assert_eq!(
        status(&harness),
        (compiled, evaluated + 1),
        "a literal must not re-parse the sheet"
    );

    // A formula: the parse and the ordering have to be redone as well.
    write(&mut harness, "B15", "=A15+1");
    assert_eq!(status(&harness), (compiled + 1, evaluated + 2));

    // Changing the formula's text is another parse.
    write(&mut harness, "B15", "=A15+2");
    assert_eq!(status(&harness), (compiled + 2, evaluated + 3));

    // And a second number into the same literal cell still does not.
    write(&mut harness, "A15", "7");
    assert_eq!(
        status(&harness),
        (compiled + 2, evaluated + 4),
        "the formula above did not make the literal edit structural"
    );
    assert_eq!(cell_value(&harness, "B15"), "9", "and it did recalculate");
}

// ---------------------------------------------------------------------------
// S-5: the keyboard
// ---------------------------------------------------------------------------

/// S-5: arrows move, Shift extends, Enter and F2 open the editor, Enter and Tab
/// commit and step, Escape puts it back, and typing replaces.
#[test]
fn the_grid_is_driven_from_the_keyboard() {
    let mut harness = loaded();

    click(&mut harness, "A15");
    press(&mut harness, egui::Key::ArrowDown);
    assert_eq!(name_box(&harness), "A16", "the cursor moved down");

    key(&mut harness, egui::Key::ArrowRight, egui::Modifiers::SHIFT);
    assert_eq!(
        name_box(&harness),
        "A16:B16",
        "Shift keeps the anchor where it was"
    );

    // Enter opens the editor on the cursor cell, which Shift+Right left on B16.
    press(&mut harness, egui::Key::Enter);
    assert!(harness.query_by_label("edit B16").is_some());

    // Typing into it and pressing Enter commits and steps down.
    type_text(&mut harness, "12");
    press(&mut harness, egui::Key::Enter);
    assert_eq!(cell_value(&harness, "B16"), "12");
    assert_eq!(name_box(&harness), "B17", "Enter steps down");

    // Tab commits and steps right instead.
    click(&mut harness, "A18");
    type_text(&mut harness, "3");
    press(&mut harness, egui::Key::Tab);
    assert_eq!(cell_value(&harness, "A18"), "3");
    assert_eq!(name_box(&harness), "B18", "Tab steps right");

    // Escape leaves the cell as it was.
    click(&mut harness, "A18");
    type_text(&mut harness, "999");
    press(&mut harness, egui::Key::Escape);
    assert_eq!(
        cell_value(&harness, "A18"),
        "3",
        "Escape throws the draft away"
    );
    assert!(harness.query_by_label("edit A18").is_none());

    // F2 opens the editor keeping what is there; typing at the grid replaces it.
    press(&mut harness, egui::Key::F2);
    let editor = harness.get_by_label("edit A18").accesskit_node();
    assert_eq!(editor.value().unwrap_or_default(), "3", "F2 keeps the text");
    press(&mut harness, egui::Key::Escape);

    type_text(&mut harness, "8");
    press(&mut harness, egui::Key::Enter);
    assert_eq!(cell_value(&harness, "A18"), "8", "typing replaces");
}

// ---------------------------------------------------------------------------
// S-6: the clipboard
// ---------------------------------------------------------------------------

/// S-6: a paste moves every reference by the distance it travelled, and a
/// rectangle of them is one undo step.
#[test]
fn copy_and_paste_shift_the_references_they_carry() {
    let mut harness = loaded();

    write(&mut harness, "A15", "2");
    write(&mut harness, "B15", "3");
    write(&mut harness, "C15", "=A15*B15");
    write(&mut harness, "A16", "4");
    write(&mut harness, "B16", "5");

    click(&mut harness, "C15");
    key(&mut harness, egui::Key::C, egui::Modifiers::COMMAND);
    click(&mut harness, "C16");
    key(&mut harness, egui::Key::V, egui::Modifiers::COMMAND);
    assert!(
        inspector(&harness).contains("C16 = A16*B16"),
        "the pasted formula reads its own row: {}",
        inspector(&harness)
    );
    assert_eq!(cell_value(&harness, "C16"), "20");

    // A 2 x 2 rectangle, and one undo takes all four back.
    write(&mut harness, "A19", "1");
    write(&mut harness, "B19", "2");
    write(&mut harness, "A20", "3");
    write(&mut harness, "B20", "4");
    click(&mut harness, "A19");
    key(&mut harness, egui::Key::ArrowRight, egui::Modifiers::SHIFT);
    key(&mut harness, egui::Key::ArrowDown, egui::Modifiers::SHIFT);
    assert_eq!(name_box(&harness), "A19:B20");
    key(&mut harness, egui::Key::C, egui::Modifiers::COMMAND);

    click(&mut harness, "E19");
    key(&mut harness, egui::Key::V, egui::Modifiers::COMMAND);
    for (cell, want) in [("E19", "1"), ("F19", "2"), ("E20", "3"), ("F20", "4")] {
        assert_eq!(cell_value(&harness, cell), want);
    }

    key(&mut harness, egui::Key::Z, egui::Modifiers::COMMAND);
    for cell in ["E19", "F19", "E20", "F20"] {
        assert_eq!(cell_value(&harness, cell), "", "one paste, one undo step");
    }
}

// ---------------------------------------------------------------------------
// S-7 and S-9: the highlight, and what virtualization costs
// ---------------------------------------------------------------------------

/// S-7: an edit in progress survives its row scrolling out of view and back.
///
/// The row really is unmounted in between — the assertion that `A41` is not in
/// the tree at all is the point of the middle of this test — so the draft
/// cannot be in the cell. It is in the reducer at `App`, and the cell is only
/// told "you are being edited, here is the draft".
#[test]
fn an_edit_survives_scrolling_out_of_view_and_back() {
    let mut harness = loaded();

    scroll(&mut harness, ROW_H * 30.0);
    click(&mut harness, "A41");
    type_text(&mut harness, "abc");
    assert!(
        harness.query_by_label("edit A41").is_some(),
        "the editor is open on A41"
    );

    scroll(&mut harness, -ROW_H * 60.0);
    assert!(
        harness.query_by_label("A41").is_none(),
        "the row is unmounted, not hidden"
    );
    assert!(harness.query_by_label("edit A41").is_none());

    scroll(&mut harness, ROW_H * 30.0);
    assert!(
        harness.query_by_label("edit A41").is_some(),
        "and the edit is still in progress"
    );
    press(&mut harness, egui::Key::Enter);
    assert_eq!(cell_value(&harness, "A41"), "abc");
}

/// S-9: a long scroll leaves no hook id collisions behind.
///
/// This is the first example with a thousand hooks in view at once. The row
/// index is the hook scope and the column is the cell's, both written by hand
/// rather than by `key=`, so "no two hooks claimed the same id" is worth an
/// assertion of its own.
#[test]
fn scrolling_the_grid_collides_with_nothing() {
    let mut harness = loaded();
    for _ in 0..20 {
        scroll(&mut harness, ROW_H * 4.0);
    }
    click(&mut harness, "A81");
    type_text(&mut harness, "x");
    for _ in 0..20 {
        scroll(&mut harness, -ROW_H * 4.0);
    }
    assert!(
        harness.state().collisions().is_empty(),
        "hook id collisions: {:?}",
        harness.state().collisions()
    );
}

// ---------------------------------------------------------------------------
// S-8: the history
// ---------------------------------------------------------------------------

/// S-8: undo walks back one edit at a time, redo replays them, and doing
/// something new throws the redo branch away.
#[test]
fn undo_and_redo_walk_the_history_one_edit_at_a_time() {
    let mut harness = loaded();
    write(&mut harness, "A15", "1");
    write(&mut harness, "A16", "2");

    click(&mut harness, "undo");
    assert_eq!(cell_value(&harness, "A16"), "");
    assert_eq!(cell_value(&harness, "A15"), "1");

    click(&mut harness, "undo");
    assert_eq!(cell_value(&harness, "A15"), "");

    assert!(enabled(&harness, "redo"));
    click(&mut harness, "redo");
    assert_eq!(cell_value(&harness, "A15"), "1");
    click(&mut harness, "redo");
    assert_eq!(cell_value(&harness, "A16"), "2");

    // A new edit after an undo is a new branch: there is nothing to redo.
    click(&mut harness, "undo");
    assert!(enabled(&harness, "redo"));
    write(&mut harness, "A17", "3");
    assert!(
        !enabled(&harness, "redo"),
        "the redo branch should have been dropped"
    );
}

// ---------------------------------------------------------------------------
// S-10: the column header
// ---------------------------------------------------------------------------

/// S-10: dragging a column edge widens the column, moves everything to the
/// right of it, and is one undo step.
#[test]
fn dragging_a_column_edge_resizes_it_in_one_step() {
    let mut harness = loaded();
    let before = harness.get_by_label("A15").rect();
    let neighbour = harness.get_by_label("B15").rect();

    // The header sits directly above the first row, so the edge between A and B
    // is half a row above the top of the grid at A's right-hand side.
    let edge = egui::pos2(
        before.right(),
        harness.get_by_label("A1").rect().top() - ROW_H / 2.0,
    );
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(edge));
    harness.step();
    harness.input_mut().events.push(egui::Event::PointerButton {
        pos: edge,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    for step in 1..=4i16 {
        let to = edge + egui::vec2(10.0 * f32::from(step), 0.0);
        harness
            .input_mut()
            .events
            .push(egui::Event::PointerMoved(to));
        harness.step();
    }
    harness.input_mut().events.push(egui::Event::PointerButton {
        pos: edge + egui::vec2(40.0, 0.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    settle(&mut harness);

    let after = harness.get_by_label("A15").rect();
    assert!(
        (after.width() - before.width() - 40.0).abs() < 1.0,
        "A should be 40 wider: {} -> {}",
        before.width(),
        after.width()
    );
    assert!(
        (harness.get_by_label("B15").rect().left() - neighbour.left() - 40.0).abs() < 1.0,
        "and B should have moved right by the same"
    );

    click(&mut harness, "undo");
    assert!(
        (harness.get_by_label("A15").rect().width() - before.width()).abs() < 1.0,
        "one drag, one undo step"
    );
}
