//! ARCHITECTURE.md 10, item 2: a `#[hook]`-style custom hook can return the
//! guard (the store lifetime `'s` is independent of the `&mut Cx` borrow), and
//! two calls in one component get independent state.

mod common;

use common::{run_app, use_counter};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use react_egui::prelude::*;

fn app(cx: &mut Cx<'_, '_>) {
    let mut left = use_counter(cx);
    let mut right = use_counter(cx);

    cx.ui().label(format!("left: {}", *left));
    if cx.ui().button("left +").clicked() {
        *left += 1;
    }
    cx.ui().label(format!("right: {}", *right));
    if cx.ui().button("right +").clicked() {
        *right += 1;
    }
}

#[test]
fn two_calls_of_one_custom_hook_are_independent() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, app);
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("left: 0").is_some());
    assert!(harness.query_by_label("right: 0").is_some());

    harness.get_by_label("left +").click();
    harness.run();
    harness.get_by_label("left +").click();
    harness.run();
    assert!(harness.query_by_label("left: 2").is_some());
    assert!(harness.query_by_label("right: 0").is_some());

    harness.get_by_label("right +").click();
    harness.run();
    assert!(harness.query_by_label("left: 2").is_some());
    assert!(harness.query_by_label("right: 1").is_some());
}
