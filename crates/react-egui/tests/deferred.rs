//! Plan 1.4 / test 2-4: `update_later` is the way out of "the loop borrows the
//! state", `cx.defer` runs once at the end of the pass, and a write queued for
//! a slot that disappears in the same pass is harmless.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use react_egui::prelude::*;

#[test]
fn update_later_removes_from_a_list_being_iterated() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                let todos = use_state(cx, || vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
                // `todos` is borrowed by the loop, so a handler cannot write to
                // it directly. `update_later` queues the write instead, and the
                // closure is `'static`, hence `move`.
                for (i, todo) in todos.iter().enumerate() {
                    if cx.ui().button(format!("del {todo}")).clicked() {
                        todos.update_later(move |t| {
                            t.remove(i);
                        });
                    }
                }
                // Drawn after the loop, so within this pass it still sees the
                // old value (ARCHITECTURE.md 5.7).
                cx.ui().label(format!("len: {}", todos.len()));
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("len: 3").is_some());

    harness.get_by_label("del b").click();
    harness.step();
    assert!(
        harness.query_by_label("len: 3").is_some(),
        "the label was drawn before the queue was applied"
    );

    harness.run();
    assert!(harness.query_by_label("len: 2").is_some());
    assert!(harness.query_by_label("del b").is_none());
    assert!(harness.query_by_label("del a").is_some());
    assert!(harness.query_by_label("del c").is_some());
}

#[test]
fn defer_runs_once_per_pass_and_does_not_repaint() {
    let passes = Rc::new(Cell::new(0u32));
    let deferred = Rc::new(Cell::new(0u32));

    let passes_in_app = Rc::clone(&passes);
    let deferred_in_app = Rc::clone(&deferred);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let passes = Rc::clone(&passes_in_app);
            let deferred = Rc::clone(&deferred_in_app);
            run_app(ui, store, move |cx| {
                passes.set(passes.get() + 1);
                let deferred = Rc::clone(&deferred);
                cx.defer(move || deferred.set(deferred.get() + 1));
                cx.ui().label("deferred");
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(passes.get() > 0);
    assert_eq!(deferred.get(), passes.get());

    // `defer` touches no state, so it must not keep the app awake.
    assert!(!harness.ctx.has_requested_repaint());
    harness.step();
    assert_eq!(deferred.get(), passes.get());
    assert!(!harness.ctx.has_requested_repaint());
}

#[test]
fn update_later_on_a_slot_that_unmounts_in_the_same_pass_is_dropped() {
    let show = Rc::new(Cell::new(true));

    let show_in_app = Rc::clone(&show);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let show = Rc::clone(&show_in_app);
            run_app(ui, store, move |cx| {
                if show.get() {
                    let count = use_state(cx, || 0i32);
                    if cx.ui().button("queue then hide").clicked() {
                        count.update_later(|c| *c += 1);
                        show.set(false);
                    }
                    cx.ui().label(format!("count: {}", *count));
                }
                cx.ui().label("root");
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("count: 0").is_some());

    // The click hides the component; the queued write is applied to a slot the
    // sweep is about to drop, and the remount starts from the initial value.
    harness.get_by_label("queue then hide").click();
    harness.run();
    assert!(harness.query_by_label("root").is_some());
    assert_eq!(harness.state().len(), 0);

    show.set(true);
    harness.run();
    assert!(harness.query_by_label("count: 0").is_some());
}

#[test]
fn handle_update_later_repaints() {
    let queue = Rc::new(Cell::new(false));

    let queue_in_app = Rc::clone(&queue);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let queue = Rc::clone(&queue_in_app);
            run_app(ui, store, move |cx| {
                let count = use_handle(cx, || 0i32);
                if queue.get() {
                    queue.set(false);
                    count.update_later(|c| *c += 1);
                }
                cx.ui().label(format!("handle: {}", count.get()));
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("handle: 0").is_some());

    queue.set(true);
    harness.step();
    assert!(harness.ctx.has_requested_repaint());
    harness.run();
    assert!(harness.query_by_label("handle: 1").is_some());
}
