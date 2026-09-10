//! ARCHITECTURE.md 4 (`use_effect` の詳細): the body runs on the first visit and
//! whenever the deps hash changes, the previous cleanup runs before the new
//! body, and equal deps mean no re-run.

mod common;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use common::run_app;
use egui_kittest::Harness;
use egui_reactor::prelude::*;

type Log = Arc<Mutex<Vec<&'static str>>>;

fn take_log(log: &Log) -> Vec<&'static str> {
    log.lock().unwrap().clone()
}

#[test]
fn effect_reruns_only_when_deps_change() {
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let dep = Rc::new(Cell::new(0u32));

    let log_in_app = Arc::clone(&log);
    let dep_in_app = Rc::clone(&dep);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let log = Arc::clone(&log_in_app);
            let dep = Rc::clone(&dep_in_app);
            run_app(ui, store, move |cx| {
                let body_log = Arc::clone(&log);
                use_effect(cx, dep.get(), move || {
                    body_log.lock().unwrap().push("body");
                    let cleanup_log = Arc::clone(&body_log);
                    move || cleanup_log.lock().unwrap().push("cleanup")
                });
                // An effect with no cleanup must compile too (`IntoCleanup` for `()`).
                use_effect(cx, (), || {});
                cx.ui().label("effect");
            });
        },
        Store::new(),
    );

    harness.run();
    assert_eq!(take_log(&log), ["body"]);

    // Same deps over several frames: no re-run.
    harness.step();
    harness.step();
    assert_eq!(take_log(&log), ["body"]);

    // Changed deps: cleanup first, then the body.
    dep.set(1);
    harness.step();
    assert_eq!(take_log(&log), ["body", "cleanup", "body"]);

    // And stable again afterwards.
    harness.step();
    assert_eq!(take_log(&log), ["body", "cleanup", "body"]);
}
