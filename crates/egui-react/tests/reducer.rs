//! Plan 1.3 / test 2-3: `use_reducer` applies queued messages when the hook is
//! visited, `Dispatch` is `Send` and repaints from another thread, and a second
//! pass in the same frame does not apply a message twice.

mod common;

use std::cell::{Cell, RefCell};
use std::num::NonZeroUsize;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;

enum Msg {
    Inc,
    Add(i32),
}

fn reduce(state: &mut i32, msg: Msg) {
    match msg {
        Msg::Inc => *state += 1,
        Msg::Add(n) => *state += n,
    }
}

/// A place for the test body to pick up the `Dispatch` the app made.
type Shared = Rc<RefCell<Option<Dispatch<Msg>>>>;

fn harness_with(
    shared: &Shared,
    discard: &Rc<Cell<bool>>,
    max_pass_index: &Rc<Cell<usize>>,
) -> Harness<'static, Store> {
    let shared = Rc::clone(shared);
    let discard = Rc::clone(discard);
    let max_pass_index = Rc::clone(max_pass_index);
    let harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let pass_index = ui.ctx().current_pass_index();
            max_pass_index.set(max_pass_index.get().max(pass_index));
            if discard.get() && pass_index == 0 {
                ui.ctx().request_discard("reducer test");
            }
            let shared = Rc::clone(&shared);
            run_app(ui, store, move |cx| {
                let (count, dispatch) = use_reducer(cx, reduce, || 0i32);
                *shared.borrow_mut() = Some(dispatch.clone());
                if cx.ui().button("inc").clicked() {
                    dispatch.send(Msg::Inc);
                }
                cx.ui().label(format!("count: {}", *count));
            });
        },
        Store::new(),
    );
    harness
        .ctx
        .options_mut(|o| o.max_passes = NonZeroUsize::new(2).unwrap());
    harness
}

#[test]
fn a_handler_send_lands_on_the_next_frame() {
    let shared: Shared = Rc::new(RefCell::new(None));
    let discard = Rc::new(Cell::new(false));
    let max_pass_index = Rc::new(Cell::new(0usize));
    let mut harness = harness_with(&shared, &discard, &max_pass_index);

    harness.run();
    assert!(harness.query_by_label("count: 0").is_some());

    harness.get_by_label("inc").click();
    harness.run();
    assert!(harness.query_by_label("count: 1").is_some());
}

#[test]
fn dispatch_from_another_thread_repaints_and_lands() {
    let shared: Shared = Rc::new(RefCell::new(None));
    let discard = Rc::new(Cell::new(false));
    let max_pass_index = Rc::new(Cell::new(0usize));
    let mut harness = harness_with(&shared, &discard, &max_pass_index);

    harness.run();
    assert!(!harness.ctx.has_requested_repaint());

    // `Dispatch` is `Clone + Send + 'static`, which is the whole point of it.
    let dispatch = shared.borrow().clone().expect("the app made a Dispatch");
    std::thread::spawn(move || dispatch.send(Msg::Add(41)))
        .join()
        .unwrap();
    assert!(
        harness.ctx.has_requested_repaint(),
        "send must request a repaint or the message would sit in the queue"
    );

    harness.run();
    assert!(harness.query_by_label("count: 41").is_some());
}

#[test]
fn a_message_is_not_applied_twice_in_a_two_pass_frame() {
    let shared: Shared = Rc::new(RefCell::new(None));
    let discard = Rc::new(Cell::new(false));
    let max_pass_index = Rc::new(Cell::new(0usize));
    let mut harness = harness_with(&shared, &discard, &max_pass_index);

    harness.run();

    // Pass 1 sends, pass 2 of the same frame visits the hook again and drains
    // the queue. The reducer must still have run exactly once in total.
    discard.set(true);
    max_pass_index.set(0);
    harness.get_by_label("inc").click();
    harness.step();
    discard.set(false);
    assert_eq!(
        max_pass_index.get(),
        1,
        "the frame must have run two passes"
    );

    harness.run();
    assert!(harness.query_by_label("count: 1").is_some());
    assert!(harness.query_by_label("count: 2").is_none());
}
