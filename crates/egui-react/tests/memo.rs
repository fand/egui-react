//! Plan 1.2 / test 2-2: `use_memo` recomputes only when the deps hash changes,
//! its `&T` lives next to a `State` guard for the whole pass, and unmounting
//! throws the cached value away.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;

#[test]
fn memo_recomputes_only_when_deps_change() {
    let runs = Rc::new(Cell::new(0u32));
    let dep = Rc::new(Cell::new(0u32));

    let runs_in_app = Rc::clone(&runs);
    let dep_in_app = Rc::clone(&dep);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let runs = Rc::clone(&runs_in_app);
            let dep = Rc::clone(&dep_in_app);
            run_app(ui, store, move |cx| {
                let d = dep.get();
                let value: &String = use_memo(cx, d, || {
                    runs.set(runs.get() + 1);
                    format!("memo {d}")
                });
                cx.ui().label(value.clone());
            });
        },
        Store::new(),
    );

    harness.run();
    assert_eq!(runs.get(), 1);
    assert!(harness.query_by_label("memo 0").is_some());

    // Same deps over several frames: the body does not run again.
    harness.step();
    harness.step();
    assert_eq!(runs.get(), 1);

    dep.set(1);
    harness.step();
    assert_eq!(runs.get(), 2);
    harness.run();
    assert!(harness.query_by_label("memo 1").is_some());
    assert_eq!(runs.get(), 2);
}

#[test]
fn memo_reference_outlives_the_cx_borrow() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                let value: &String = use_memo(cx, (), || String::from("shared"));
                // The memo borrows the store, not the `Cx`, so a `State` guard
                // and further hooks can be taken while it is alive...
                let mut count = use_state(cx, || 0i32);
                *count += 1;
                // ...and it is still readable in two places afterwards.
                cx.ui().label(format!("first: {value}"));
                cx.ui().label(format!("second: {value} {}", *count));
            });
        },
        Store::new(),
    );

    // The body mutates every pass, so it never settles: use `step`.
    harness.step();
    assert!(harness.query_by_label("first: shared").is_some());
    assert!(harness.query_by_label_contains("second: shared").is_some());
}

#[test]
fn memo_is_dropped_on_unmount() {
    let runs = Rc::new(Cell::new(0u32));
    let show = Rc::new(Cell::new(true));

    let runs_in_app = Rc::clone(&runs);
    let show_in_app = Rc::clone(&show);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let runs = Rc::clone(&runs_in_app);
            let show = Rc::clone(&show_in_app);
            run_app(ui, store, move |cx| {
                if show.get() {
                    let value: &String = use_memo(cx, (), || {
                        runs.set(runs.get() + 1);
                        String::from("child")
                    });
                    cx.ui().label(value.clone());
                }
            });
        },
        Store::new(),
    );

    harness.run();
    assert_eq!(runs.get(), 1);
    assert_eq!(harness.state().len(), 1);

    show.set(false);
    harness.run();
    assert_eq!(harness.state().len(), 0);

    // Remounting has to recompute: the sweep took the cached value with it.
    show.set(true);
    harness.run();
    assert_eq!(runs.get(), 2);
}
