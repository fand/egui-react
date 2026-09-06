//! The same edits give the same settings and the same log in both versions.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use form::App;
use form::plain::{self, PlainState};
use egui_react::prelude::*;
use egui_react_app::{root_id, root_style};

const SIZE: egui::Vec2 = egui::vec2(420.0, 420.0);

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

/// Type into the name field and untick `notify`, then read the summary line
/// and the log back.
fn drive<S>(harness: &mut Harness<'_, S>) {
    assert!(
        harness.query_by_label("anon, dark, volume 50").is_some(),
        "expected the default summary",
    );
    assert!(
        harness.query_by_label("log (1)").is_none(),
        "expected no log"
    );

    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .focus();
    harness.run();
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .type_text("2");
    harness.run();
    harness.run();
    assert!(
        harness.query_by_label("anon2, dark, volume 50").is_some(),
        "the summary did not follow the field",
    );

    harness
        .get_all_by_role(egui::accesskit::Role::CheckBox)
        .next()
        .expect("the notify checkbox")
        .click();
    harness.run();
    harness.run();

    // One line for the edit, one for the checkbox.
    assert!(
        harness.query_by_label("log (2)").is_some(),
        "expected two log entries",
    );
    harness.get_by_label("log (2)").click();
    harness.run();
    harness.run();
    assert!(
        harness.query_by_label("notify = false").is_some(),
        "expected the checkbox's new value in the log",
    );
}

#[test]
fn the_egui_react_version_edits_the_settings() {
    let mut harness = react();
    harness.run();
    drive(&mut harness);
}

#[test]
fn the_plain_egui_version_edits_the_settings_the_same() {
    let mut harness = plain();
    harness.run();
    drive(&mut harness);
}
