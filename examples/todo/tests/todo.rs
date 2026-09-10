//! Plan test A-2: add, toggle, remove and clear done, driven the same way
//! against both versions.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::prelude::*;
use egui_reactor_app::{root_id, root_style};
use todo::App;
use todo::plain::{self, PlainState};

const SIZE: egui::Vec2 = egui::vec2(400.0, 400.0);

fn react<'a>() -> Harness<'a, Store> {
    Harness::builder().with_size(SIZE).build_ui_state(
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

fn plain<'a>() -> Harness<'a, PlainState> {
    Harness::builder().with_size(SIZE).build_ui_state(
        |ui, state: &mut PlainState| plain::ui(ui, state),
        PlainState::default(),
    )
}

/// Type `text` into the field and press Enter.
fn add<S>(harness: &mut Harness<'_, S>, text: &str) {
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .focus();
    harness.run();
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .type_text(text);
    harness.run();
    harness.key_press(egui::Key::Enter);
    // One pass to apply the write, one to draw the new list.
    harness.run();
    harness.run();
}

/// Expand the "done" section, if it is not already showing `target`.
///
/// Whether it starts open is egui's business either way (both versions use a
/// `CollapsingHeader`), and not what this test is about.
fn open_done<S>(harness: &mut Harness<'_, S>, target: &str) {
    if harness.query_by_label(target).is_none() {
        harness.get_by_label("done (1)").click();
        harness.run();
        harness.run();
    }
}

/// Add two, tick one off, undo it, remove one, then clear the done one.
fn drive<S>(harness: &mut Harness<'_, S>) {
    add(harness, "write the plan");
    add(harness, "read the plan");
    assert!(
        harness.query_by_label("2 left").is_some(),
        "expected 2 left"
    );

    // The first checkbox in the tree is the first item's.
    harness
        .get_all_by_role(egui::accesskit::Role::CheckBox)
        .next()
        .unwrap()
        .click();
    harness.run();
    harness.run();
    assert!(
        harness.query_by_label("1 left").is_some(),
        "expected 1 left"
    );
    assert!(
        harness.query_by_label("done (1)").is_some(),
        "expected a done section"
    );

    open_done(harness, "undo");
    harness.get_by_label("undo").click();
    harness.run();
    harness.run();
    assert!(
        harness.query_by_label("2 left").is_some(),
        "expected 2 left again"
    );

    // Remove the first item.
    harness.get_all_by_label("remove").next().unwrap().click();
    harness.run();
    harness.run();
    assert!(
        harness.query_by_label("1 left").is_some(),
        "expected 1 left"
    );

    // Tick the survivor off and clear it.
    harness
        .get_all_by_role(egui::accesskit::Role::CheckBox)
        .next()
        .unwrap()
        .click();
    harness.run();
    harness.run();
    open_done(harness, "clear done");
    harness.get_by_label("clear done").click();
    harness.run();
    harness.run();
    assert!(
        harness.query_by_label("0 left").is_some(),
        "expected an empty list"
    );
    assert!(
        harness.query_by_label("done (1)").is_none(),
        "the done section should be gone"
    );
}

#[test]
fn the_egui_reactor_version_manages_the_list() {
    let mut harness = react();
    harness.run();
    drive(&mut harness);
}

#[test]
fn the_plain_egui_version_manages_the_list_the_same() {
    let mut harness = plain();
    harness.run();
    drive(&mut harness);
}
