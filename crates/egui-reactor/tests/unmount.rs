//! ARCHITECTURE.md 10, item 7 and 5.2: the end-of-pass sweep drops the state of
//! a component that was not visited and runs its `use_effect` cleanup, and a
//! remount starts from the initial value again.

mod common;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::prelude::*;

type Log = Arc<Mutex<Vec<&'static str>>>;

fn child(cx: &mut Cx<'_, '_>, log: &Log) {
    let mut count = use_state(cx, || 0i32);

    let body_log = Arc::clone(log);
    use_effect(cx, (), move || {
        body_log.lock().unwrap().push("mount");
        let cleanup_log = Arc::clone(&body_log);
        move || cleanup_log.lock().unwrap().push("cleanup")
    });

    cx.ui().label(format!("child: {}", *count));
    if cx.ui().button("child +").clicked() {
        *count += 1;
    }
}

#[test]
fn sweep_unmounts_and_remounts() {
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let show = Rc::new(Cell::new(true));

    let log_in_app = Arc::clone(&log);
    let show_in_app = Rc::clone(&show);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let log = Arc::clone(&log_in_app);
            let show = Rc::clone(&show_in_app);
            run_app(ui, store, move |cx| {
                cx.ui().label("parent");
                if show.get() {
                    child(cx, &log);
                }
            });
        },
        Store::new(),
    );

    harness.run();
    assert_eq!(*log.lock().unwrap(), ["mount"]);
    assert!(harness.query_by_label("child: 0").is_some());
    // One slot for `use_state`, one for `use_effect`.
    assert_eq!(harness.state().len(), 2);

    harness.get_by_label("child +").click();
    harness.run();
    assert!(harness.query_by_label("child: 1").is_some());

    // Hidden: the sweep runs the cleanup and drops both slots.
    show.set(false);
    harness.run();
    assert_eq!(*log.lock().unwrap(), ["mount", "cleanup"]);
    assert!(harness.query_by_label("child: 1").is_none());
    assert_eq!(harness.state().len(), 0);

    // Shown again: a fresh mount, back at the initial value.
    show.set(true);
    harness.run();
    assert_eq!(*log.lock().unwrap(), ["mount", "cleanup", "mount"]);
    assert!(harness.query_by_label("child: 0").is_some());
    assert_eq!(harness.state().len(), 2);
}
