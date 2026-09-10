//! ARCHITECTURE.md 10, item 1: the `State` guard lives only for the component
//! body, and sibling handlers can each borrow the same state mutably in turn.

mod common;

use common::{counter, run_app};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::Store;

#[test]
fn sibling_handlers_share_one_state() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| counter(cx, 1));
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("count: 1").is_some());

    harness.get_by_label("+").click();
    harness.run();
    assert!(harness.query_by_label("count: 2").is_some());

    harness.get_by_label("-").click();
    harness.run();
    assert!(harness.query_by_label("count: 1").is_some());
}
