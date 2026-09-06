//! ARCHITECTURE.md 5.6: a frame that mutates state through `DerefMut` requests
//! a repaint; a frame that only reads does not.
//!
//! The UI is labels only (no buttons, no hover, no cursor blink), so egui has
//! no reason of its own to ask for another frame.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;

#[test]
fn repaint_is_requested_only_on_mutation() {
    let mutate = Rc::new(Cell::new(false));
    let mutate_in_app = Rc::clone(&mutate);

    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let mutate = Rc::clone(&mutate_in_app);
            run_app(ui, store, move |cx| {
                let mut n = use_state(cx, || 0i32);
                if mutate.get() {
                    *n += 1;
                }
                cx.ui().label(format!("n: {}", *n));
            });
        },
        Store::new(),
    );

    // Settle: `run` loops until nothing asks for another frame.
    harness.run();
    assert!(!harness.ctx.has_requested_repaint());

    // A read-only frame must not request a repaint.
    harness.step();
    assert!(!harness.ctx.has_requested_repaint());

    // A frame that mutates through `DerefMut` must.
    mutate.set(true);
    harness.step();
    assert!(harness.ctx.has_requested_repaint());

    mutate.set(false);
    harness.run();
    assert!(!harness.ctx.has_requested_repaint());
}

/// Plan 1.5 / test 2-5: the deferred queue and `Dispatch` request a repaint on
/// the frame that queues them; `cx.defer`, which cannot touch state, does not.
#[test]
fn repaint_is_requested_by_update_later_and_send_but_not_defer() {
    #[derive(Clone, Copy, PartialEq)]
    enum Action {
        None,
        Defer,
        UpdateLater,
        Send,
    }

    let action = Rc::new(Cell::new(Action::None));
    let action_in_app = Rc::clone(&action);

    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let action = Rc::clone(&action_in_app);
            run_app(ui, store, move |cx| {
                let (total, dispatch) = use_reducer(cx, |s: &mut i32, m: i32| *s += m, || 0i32);
                let n = use_state(cx, || 0i32);
                match action.replace(Action::None) {
                    Action::None => {}
                    Action::Defer => cx.defer(|| {}),
                    Action::UpdateLater => n.update_later(|v| *v += 1),
                    Action::Send => dispatch.send(1),
                }
                cx.ui().label(format!("n: {} total: {}", *n, *total));
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(!harness.ctx.has_requested_repaint());

    // `defer` runs a `'static` closure that cannot reach the store.
    action.set(Action::Defer);
    harness.step();
    assert!(!harness.ctx.has_requested_repaint());

    action.set(Action::UpdateLater);
    harness.step();
    assert!(harness.ctx.has_requested_repaint());

    harness.run();
    assert!(!harness.ctx.has_requested_repaint());

    action.set(Action::Send);
    harness.step();
    assert!(harness.ctx.has_requested_repaint());

    harness.run();
    assert!(harness.query_by_label("n: 1 total: 1").is_some());
    assert!(!harness.ctx.has_requested_repaint());
}
