//! The notes app does what a notes app does.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use egui_react::prelude::*;
use egui_react_app::{root_id, root_style};
use notes::App;

fn harness<'a>() -> Harness<'a, Store> {
    Harness::builder()
        .with_size(egui::vec2(700.0, 500.0))
        .build_ui_state(
            |ui, store: &mut Store| {
                store.begin_pass(ui.ctx());
                {
                    let store: &Store = store;
                    let mut cx = Cx::new(store, ui, root_id());
                    let view = rsx! { <App/> };
                    cx.root_container(root_id(), root_style(), |cx| view.show(cx));
                }
                store.end_pass();
            },
            Store::new(),
        )
}

/// Two passes: one to apply the write, one to draw with it.
fn settle(harness: &mut Harness<'_, Store>) {
    harness.run();
    harness.run();
}

fn new_note(harness: &mut Harness<'_, Store>) {
    harness.get_by_label("new").click_accesskit();
    settle(harness);
}

/// The body field is the only multi-line one on screen.
fn type_body(harness: &mut Harness<'_, Store>, text: &str) {
    harness
        .get_by_role(egui::accesskit::Role::MultilineTextInput)
        .focus();
    harness.run();
    harness
        .get_by_role(egui::accesskit::Role::MultilineTextInput)
        .type_text(text);
    settle(harness);
}

/// The search field is the first single-line one.
fn type_search(harness: &mut Harness<'_, Store>, text: &str) {
    harness
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .next()
        .expect("the search field")
        .focus();
    harness.run();
    harness
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .next()
        .expect("the search field")
        .type_text(text);
    settle(harness);
}

/// Every note title in the sidebar.
fn titles(harness: &Harness<'_, Store>) -> Vec<String> {
    (1..=8)
        .map(|i| format!("Note {i}"))
        .filter(|title| harness.query_by_label(title).is_some())
        .collect()
}

#[test]
fn making_notes_fills_the_list() {
    let mut harness = harness();
    harness.run();
    assert!(harness.query_by_label("no notes").is_some());
    assert!(
        harness
            .query_by_label("nothing open. make a note.")
            .is_some()
    );

    new_note(&mut harness);
    new_note(&mut harness);

    assert_eq!(titles(&harness), ["Note 1", "Note 2"]);
    assert!(harness.query_by_label("no notes").is_none());
}

/// `new` puts the caret in the title with the name selected, so typing
/// replaces "Note N" outright.
#[test]
fn a_new_note_gets_its_title_typed_first() {
    let mut harness = harness();
    harness.run();
    new_note(&mut harness);
    settle(&mut harness);

    let title = || egui::accesskit::Role::TextInput;
    let field = harness
        .get_all_by_role(title())
        .nth(1)
        .expect("the title field");
    assert!(field.is_focused(), "the title takes the focus");
    field.type_text("groceries");
    settle(&mut harness);
    let open = harness
        .get_all_by_role(title())
        .nth(1)
        .and_then(|node| node.accesskit_node().value());
    assert_eq!(
        open.as_deref(),
        Some("groceries"),
        "the old name was selected"
    );
    assert!(
        harness.query_by_label("groceries").is_some(),
        "and the list follows"
    );
}

#[test]
fn typing_updates_the_word_count() {
    let mut harness = harness();
    harness.run();
    new_note(&mut harness);
    assert!(harness.query_by_label("0 words").is_some());

    type_body(&mut harness, "three little words");
    assert!(harness.query_by_label("3 words").is_some());
}

#[test]
fn search_narrows_the_list() {
    let mut harness = harness();
    harness.run();
    new_note(&mut harness);
    new_note(&mut harness);
    assert_eq!(titles(&harness).len(), 2);

    type_search(&mut harness, "te 1");
    assert_eq!(titles(&harness), ["Note 1"]);

    type_search(&mut harness, "zzz");
    assert!(harness.query_by_label("no notes").is_some());
}

#[test]
fn deleting_opens_what_is_left() {
    let mut harness = harness();
    harness.run();
    new_note(&mut harness);
    new_note(&mut harness);

    // The newest is open; delete it and the other one takes its place.
    harness.get_by_label("delete").click_accesskit();
    settle(&mut harness);

    assert_eq!(titles(&harness), ["Note 1"]);
    let open = harness
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .nth(1)
        .and_then(|node| node.accesskit_node().value());
    assert_eq!(open.as_deref(), Some("Note 1"), "the title field");

    harness.get_by_label("delete").click_accesskit();
    settle(&mut harness);
    assert!(
        harness
            .query_by_label("nothing open. make a note.")
            .is_some()
    );
}

/// The body field fills what the title row and the counts leave, and no
/// more: a filling widget measures itself as all the room there is, and
/// without `h={0}` beside `grow` the column would run off the window.
#[test]
fn the_editor_stays_inside_the_window() {
    let mut harness = harness();
    harness.run();
    new_note(&mut harness);
    settle(&mut harness);

    let body = harness
        .get_by_role(egui::accesskit::Role::MultilineTextInput)
        .rect();
    assert!(
        body.max.y <= 500.0 && body.max.x <= 700.0,
        "the body runs off the window: {body:?}"
    );
    assert!(body.height() > 200.0, "and it fills the column: {body:?}");
}
