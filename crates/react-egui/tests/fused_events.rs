//! ARCHITECTURE.md 10, item 3: the fused event closure compiles when several
//! arms capture the same `State` mutably, and `Handler` calls nullary and
//! unary handlers through one uniform expression.

mod common;

use common::{Dialog, DialogEvent, run_app};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use react_egui::prelude::*;

fn app(cx: &mut Cx<'_, '_>) {
    let mut open = use_state(cx, || true);
    let mut title = use_state(cx, || String::from("Quit?"));

    cx.ui().label(format!("state: {}", *title));
    if !*open && cx.ui().button("Reopen").clicked() {
        *open = true;
    }

    if *open {
        // The `title` prop shares `title` immutably while the fused closure
        // needs it mutably, so the value is copied out first. See the notes on
        // ARCHITECTURE.md 3.7.
        let title_text = (*title).clone();
        rsx! {
            <Dialog
                title={&title_text}
                on_ok={|| *open = false}
                on_cancel={|| *open = false}
                on_rename={|name: String| *title = name}
            />
        }
        .show(cx);
    }
}

#[test]
fn fused_event_closure_drives_several_handlers() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, app);
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("Quit?").is_some());

    // The unary handler renames through the payload.
    harness.get_by_label("Rename").click();
    harness.run();
    assert!(harness.query_by_label("Renamed?").is_some());
    assert!(harness.query_by_label("state: Renamed?").is_some());

    // A nullary handler closes the dialog.
    harness.get_by_label("OK").click();
    harness.run();
    assert!(harness.query_by_label("OK").is_none());

    harness.get_by_label("Reopen").click();
    harness.run();
    assert!(harness.query_by_label("OK").is_some());

    // The other nullary handler, capturing the very same state, also closes it.
    harness.get_by_label("Cancel").click();
    harness.run();
    assert!(harness.query_by_label("Cancel").is_none());
}

/// The shapes `rsx!` must be able to emit as `Handler::call(closure, payload)`
/// without knowing anything about the closure. All of these infer `Marker`.
#[test]
fn handler_call_shapes() {
    let mut n = 0i32;
    let mut s = String::new();

    // nullary handler, unit payload
    Handler::call(|| n += 1, ());
    // unary handler, annotated parameter
    Handler::call(|v: String| s = v, String::from("a"));
    // unary handler, parameter type left to inference
    Handler::call(|v| s = v, String::from("b"));
    // nullary handler dropping a non-unit payload
    Handler::call(|| n += 1, String::from("c"));
    // unary handler consuming the unit payload
    Handler::call(|_v: ()| n += 1, ());
    // unary handler over a borrowed payload
    Handler::call(|v: &str| s = v.to_owned(), "d");
    // handler body evaluating to something other than `()`
    Handler::call(|| n.checked_add(1).unwrap(), ());
    // a plain fn item, not a closure
    fn set_it(v: String) {
        drop(v);
    }
    Handler::call(set_it, String::from("e"));

    assert_eq!(n, 3);
    assert_eq!(s, "d");
}
