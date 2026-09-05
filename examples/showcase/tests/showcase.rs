//! The notes app does what a notes app does.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use react_egui::prelude::*;
use react_egui_app::{root_id, root_style};
use showcase::App;

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
        .map(|i| format!("untitled {i}"))
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

    assert_eq!(titles(&harness), ["untitled 1", "untitled 2"]);
    assert!(harness.query_by_label("no notes").is_none());
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

    type_search(&mut harness, "led 1");
    assert_eq!(titles(&harness), ["untitled 1"]);

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

    assert_eq!(titles(&harness), ["untitled 1"]);
    let open = harness
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .nth(1)
        .and_then(|node| node.accesskit_node().value());
    assert_eq!(open.as_deref(), Some("untitled 1"), "the title field");

    harness.get_by_label("delete").click_accesskit();
    settle(&mut harness);
    assert!(
        harness
            .query_by_label("nothing open. make a note.")
            .is_some()
    );
}

#[test]
fn clearing_everything_takes_two_clicks() {
    let mut harness = harness();
    harness.run();
    new_note(&mut harness);
    new_note(&mut harness);

    harness.get_by_label("settings").click_accesskit();
    settle(&mut harness);
    assert!(harness.query_by_label("compact rows").is_some());

    // One click asks, and changes nothing.
    harness.get_by_label("clear all").click_accesskit();
    settle(&mut harness);
    assert!(harness.query_by_label("delete all 2 notes?").is_some());
    assert_eq!(titles(&harness).len(), 2);

    // Backing out changes nothing either.
    harness.get_by_label("cancel").click_accesskit();
    settle(&mut harness);
    assert_eq!(titles(&harness).len(), 2);

    harness.get_by_label("clear all").click_accesskit();
    settle(&mut harness);
    harness.get_by_label("yes, clear all").click_accesskit();
    settle(&mut harness);
    assert!(titles(&harness).is_empty());

    // And the window closes on its own button.
    harness.get_by_label("Close window").click_accesskit();
    settle(&mut harness);
    assert!(harness.query_by_label("compact rows").is_none());
}
