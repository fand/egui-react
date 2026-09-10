//! Plan test A-1: the clicks give the numbers they should.

use counter::App;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::prelude::*;
use egui_reactor_app::{root_id, root_style};

const SIZE: egui::Vec2 = egui::vec2(400.0, 300.0);

/// The egui-reactor version under the runner's frame.
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

/// `+`, `+`, `-`, `+` leaves 2; `reset` puts it back to 0.
fn drive<S>(harness: &mut Harness<'_, S>) {
    assert!(harness.query_by_label("0").is_some());

    for label in ["+", "+", "-", "+"] {
        harness.get_by_label(label).click();
        harness.run();
    }
    assert!(harness.query_by_label("2").is_some(), "expected 2");

    harness.get_by_label("reset").click();
    harness.run();
    assert!(harness.query_by_label("0").is_some(), "expected 0");
}

#[test]
fn the_egui_reactor_version_counts() {
    let mut harness = react();
    harness.run();
    drive(&mut harness);
}
