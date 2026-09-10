//! The provided values reach the leaves, the leaves write back, and a
//! component outside the provider sees nothing.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::prelude::*;
use egui_reactor_app::{root_id, root_style};
use theme::App;

fn harness<'a>() -> Harness<'a, Store> {
    Harness::builder()
        .with_size(egui::vec2(420.0, 420.0))
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

#[test]
fn the_locale_reaches_the_leaves() {
    let mut harness = harness();
    harness.run();

    assert!(harness.query_by_label("Hello").is_some());
    assert!(harness.query_by_label("press me").is_some());

    harness.get_by_label("français").click();
    // One pass to apply the write, one to draw with the new value.
    harness.run();
    harness.run();

    assert!(harness.query_by_label("Bonjour").is_some());
    assert!(harness.query_by_label("appuyez ici").is_some());
    assert!(harness.query_by_label("Hello").is_none());
}

#[test]
fn the_theme_reaches_the_leaves() {
    let mut harness = harness();
    harness.run();

    assert!(harness.query_by_label("mode: dark").is_some());

    harness.get_by_label("switch to light").click();
    harness.run();
    harness.run();

    assert!(harness.query_by_label("mode: light").is_some());
    assert!(harness.query_by_label("switch to dark").is_some());
}

/// The binding lasts as long as the children it was provided to, and no longer.
#[test]
fn a_component_outside_the_provider_sees_nothing() {
    let mut harness = harness();
    harness.run();

    assert!(
        harness
            .query_by_label("outside: no theme provided")
            .is_some()
    );
    // The one inside does see it, so this is scoping and not a missing value.
    assert!(harness.query_by_label("mode: dark").is_some());
}
