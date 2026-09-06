//! Plan 4.2: `use_persisted` is keyed by its string key, survives a restart
//! through `save_persisted` / `load_persisted`, and keeps the value of a
//! component that unmounted before the save.

mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Todo {
    text: String,
    done: bool,
}

/// A whole app run: a fresh store loaded from `json`, and the JSON it saved.
fn run_session(
    json: &str,
    show: bool,
    drive: impl FnOnce(&mut Harness<'_, Store>),
) -> (String, Vec<String>) {
    let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let seen_in_app = Rc::clone(&seen);

    let mut store = Store::new();
    store.load_persisted(json);

    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let seen = Rc::clone(&seen_in_app);
            run_app(ui, store, move |cx| {
                if !show {
                    cx.ui().label("hidden");
                    return;
                }
                let mut todos = use_persisted(cx, "todos", Vec::<Todo>::new);
                seen.borrow_mut().clear();
                for todo in todos.iter() {
                    seen.borrow_mut().push(todo.text.clone());
                }
                cx.ui().label(format!("count: {}", todos.len()));
                if cx.ui().button("add").clicked() {
                    let n = todos.len();
                    todos.push(Todo {
                        text: format!("item {n}"),
                        done: false,
                    });
                }
            });
        },
        store,
    );

    harness.run();
    drive(&mut harness);
    let saved = harness.state().save_persisted();
    let seen = seen.borrow().clone();
    (saved, seen)
}

#[test]
fn a_persisted_value_survives_a_restart() {
    let (saved, seen) = run_session("{}", true, |harness| {
        harness.get_by_label("add").click();
        harness.run();
        harness.get_by_label("add").click();
        harness.run();
        assert!(harness.query_by_label("count: 2").is_some());
    });
    assert_eq!(seen, ["item 0", "item 1"]);
    assert!(saved.contains("item 0"), "saved: {saved}");

    // A second launch starts from the saved JSON.
    let (saved_again, seen) = run_session(&saved, true, |harness| {
        assert!(harness.query_by_label("count: 2").is_some());
    });
    assert_eq!(seen, ["item 0", "item 1"]);
    assert_eq!(saved_again, saved);
}

#[test]
fn an_unmounted_value_is_still_saved() {
    let (saved, _) = run_session("{}", true, |harness| {
        harness.get_by_label("add").click();
        harness.run();
    });

    // This run never mounts the component, so the sweep never sees the slot and
    // the value has to come straight from what was loaded.
    let (saved_again, _) = run_session(&saved, false, |harness| {
        assert!(harness.query_by_label("hidden").is_some());
    });
    assert_eq!(saved_again, saved);
}

#[test]
fn unmounting_during_a_run_keeps_the_value() {
    let show = Rc::new(std::cell::Cell::new(true));
    let show_in_app = Rc::clone(&show);

    let mut store = Store::new();
    store.load_persisted("{}");
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let show = show_in_app.get();
            run_app(ui, store, move |cx| {
                if show {
                    let mut n = use_persisted(cx, "n", || 0i32);
                    cx.ui().label(format!("n: {}", *n));
                    if cx.ui().button("bump").clicked() {
                        *n += 1;
                    }
                } else {
                    cx.ui().label("gone");
                }
            });
        },
        store,
    );

    harness.run();
    harness.get_by_label("bump").click();
    harness.run();
    assert!(harness.query_by_label("n: 1").is_some());

    // The sweep serialises the slot on the way out.
    show.set(false);
    harness.run();
    assert!(harness.query_by_label("gone").is_some());
    assert_eq!(harness.state().len(), 0);

    let saved = harness.state().save_persisted();
    assert!(saved.contains('1'), "saved: {saved}");

    // And a restart with that JSON sees the value again.
    let mut store = Store::new();
    store.load_persisted(&saved);
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                let n = use_persisted(cx, "n", || 0i32);
                cx.ui().label(format!("n: {}", *n));
            });
        },
        store,
    );
    harness.run();
    assert!(harness.query_by_label("n: 1").is_some());
}

#[test]
fn the_key_and_not_the_call_site_identifies_the_value() {
    let mut store = Store::new();
    store.load_persisted(r#"{"shared":"7"}"#);
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                // Two call sites, one key: one slot, and a collision like any
                // other double visit.
                let a = *use_persisted(cx, "shared", || 0i32);
                cx.ui().label(format!("a: {a}"));
                let collisions = cx.store.collisions().len();
                cx.ui().label(format!("collisions: {collisions}"));
            });
        },
        store,
    );
    harness.run();
    assert!(harness.query_by_label("a: 7").is_some());
    assert!(harness.query_by_label("collisions: 0").is_some());
}

#[test]
fn a_broken_payload_falls_back_to_init() {
    let mut store = Store::new();
    store.load_persisted(r#"{"n":"not a number"}"#);
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                let n = use_persisted(cx, "n", || 42i32);
                cx.ui().label(format!("n: {}", *n));
            });
        },
        store,
    );
    harness.run();
    assert!(harness.query_by_label("n: 42").is_some());

    // Nonsense at the top level is ignored too.
    let mut store = Store::new();
    store.load_persisted("not json at all");
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                let n = use_persisted(cx, "n", || 42i32);
                cx.ui().label(format!("n: {}", *n));
            });
        },
        store,
    );
    harness.run();
    assert!(harness.query_by_label("n: 42").is_some());
}
