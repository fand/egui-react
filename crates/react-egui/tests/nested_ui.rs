//! ARCHITECTURE.md 10, item 8: a new `Cx` can be built inside an egui container
//! closure (`ui.vertical`), hooks work there, and inner / outer state are
//! independent and survive across frames.

mod common;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use react_egui::prelude::*;

fn app(cx: &mut Cx<'_, '_>) {
    let mut outer = use_state(cx, || 0i32);
    cx.ui().label(format!("outer: {}", *outer));
    if cx.ui().button("outer +").clicked() {
        *outer += 1;
    }

    // The shape a container element will expand to: copy `store` and the scope
    // id out of `cx`, then rebuild a `Cx` around the inner `Ui`.
    let (store, scope) = (cx.store, cx.scope_id());
    cx.ui().vertical(|ui| {
        let mut cx = Cx::new(store, ui, scope);
        cx.scope("inner", |cx| {
            let mut inner = use_state(cx, || 0i32);
            cx.ui().label(format!("inner: {}", *inner));
            if cx.ui().button("inner +").clicked() {
                *inner += 1;
            }
        });
    });
}

#[test]
fn hooks_work_inside_a_container_closure() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, app);
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("outer: 0").is_some());
    assert!(harness.query_by_label("inner: 0").is_some());

    harness.get_by_label("inner +").click();
    harness.run();
    assert!(harness.query_by_label("outer: 0").is_some());
    assert!(harness.query_by_label("inner: 1").is_some());

    harness.get_by_label("outer +").click();
    harness.run();
    assert!(harness.query_by_label("outer: 1").is_some());
    assert!(harness.query_by_label("inner: 1").is_some());

    // Nothing clicked: both values must survive the frame unchanged.
    harness.run();
    assert!(harness.query_by_label("outer: 1").is_some());
    assert!(harness.query_by_label("inner: 1").is_some());
}
