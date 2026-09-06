//! Plan 2.1 / 2.3 / test 3-5: `#[event]` with no payload, a value payload and
//! a borrowed payload; the `events=` escape hatch; and `emit` as a no-op when
//! the parent passed no handler at all.

mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;

#[component]
fn Editor(cx: &mut Cx, #[event] on_save: (), #[event] on_resize: usize, #[event] on_rename: &str) {
    if cx.ui().button("save").clicked() {
        on_save.emit(());
    }
    if cx.ui().button("resize").clicked() {
        on_resize.emit(42);
    }
    if cx.ui().button("rename").clicked() {
        on_rename.emit("renamed");
    }
}

type Log = Rc<RefCell<Vec<String>>>;

#[test]
fn three_event_shapes_reach_their_handlers() {
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let log_in_app = Rc::clone(&log);

    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let log = Rc::clone(&log_in_app);
            run_app(ui, store, move |cx| {
                let mut entries = log.borrow_mut();
                rsx! {
                    <Editor
                        on_save={|| entries.push(String::from("save"))}
                        on_resize={|n: usize| entries.push(format!("resize {n}"))}
                        on_rename={|name: &str| entries.push(format!("rename {name}"))}
                    />
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(log.borrow().is_empty());

    harness.get_by_label("save").click();
    harness.run();
    harness.get_by_label("resize").click();
    harness.run();
    harness.get_by_label("rename").click();
    harness.run();

    assert_eq!(*log.borrow(), ["save", "resize 42", "rename renamed"]);
}

#[test]
fn the_events_escape_hatch_replaces_the_fused_closure() {
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let log_in_app = Rc::clone(&log);

    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let log = Rc::clone(&log_in_app);
            run_app(ui, store, move |cx| {
                let mut entries = log.borrow_mut();
                rsx! {
                    <Editor events={|event| match event {
                        EditorEvent::Save(()) => entries.push(String::from("raw save")),
                        EditorEvent::Resize(n) => entries.push(format!("raw resize {n}")),
                        EditorEvent::Rename(name) => entries.push(format!("raw rename {name}")),
                    }}/>
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    harness.get_by_label("resize").click();
    harness.run();
    assert_eq!(*log.borrow(), ["raw resize 42"]);
}

#[test]
fn emitting_without_a_handler_is_a_no_op() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                rsx! { <Editor/> }.show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    // Clicking every button must not panic; the emitters hit the no-op sink.
    harness.get_by_label("save").click();
    harness.run();
    harness.get_by_label("resize").click();
    harness.run();
    harness.get_by_label("rename").click();
    harness.run();
    assert!(harness.query_by_label("save").is_some());
}
