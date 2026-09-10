//! Plan 3.1 / tests 6-1 .. 6-9: `use_future` reports `Pending` until the visit
//! after the future finished, restarts on a deps change and throws stale results
//! away, `spawn` reaches a `Dispatch`, and a pending hook is counted by the
//! nearest suspense boundary.

mod common;

use std::cell::{Cell, RefCell};
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::task::Poll;
use std::time::{Duration, Instant};

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::prelude::*;

/// One future per launch, each blocked until the test completes it by hand.
///
/// Blocking in `recv` is fine: on native the future has a thread to itself.
#[derive(Clone, Default)]
struct Gates {
    senders: Rc<RefCell<Vec<Sender<String>>>>,
}

impl Gates {
    /// Build the future for one launch and remember how to complete it.
    fn open(&self) -> impl Future<Output = String> + Send + 'static {
        let (tx, rx) = mpsc::channel();
        self.senders.borrow_mut().push(tx);
        async move {
            rx.recv()
                .expect("the gate was dropped before it was completed")
        }
    }

    /// How many futures have been launched.
    fn launches(&self) -> usize {
        self.senders.borrow().len()
    }

    /// Let launch `index` finish with `value`.
    fn complete(&self, index: usize, value: &str) {
        self.senders.borrow()[index]
            .send(String::from(value))
            .expect("the future is still waiting on its gate");
    }
}

/// Poll `cond` for up to two seconds.
fn wait_until(what: &str, cond: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if cond() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {what}");
}

/// Wait for the finished future to ask for a repaint.
///
/// The request is made *after* the result was written, so this also means the
/// next pass will see the result.
fn wait_for_repaint(harness: &Harness<'static, Store>) {
    wait_until("the future to request a repaint", || {
        harness.ctx.has_requested_repaint()
    });
}

/// An app that shows one `use_future` as "loading" or "ready: <value>".
fn loader_harness(gates: &Gates, dep: &Rc<Cell<u32>>) -> Harness<'static, Store> {
    let gates = gates.clone();
    let dep = Rc::clone(dep);
    Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let gates = gates.clone();
            let dep = Rc::clone(&dep);
            run_app(ui, store, move |cx| {
                let value = use_future(cx, dep.get(), || gates.open());
                match value {
                    Poll::Pending => cx.ui().label("loading"),
                    Poll::Ready(value) => cx.ui().label(format!("ready: {value}")),
                };
            });
        },
        Store::new(),
    )
}

#[test]
fn pending_then_ready_after_repaint() {
    let gates = Gates::default();
    let dep = Rc::new(Cell::new(0u32));
    let mut harness = loader_harness(&gates, &dep);

    harness.run();
    assert!(harness.query_by_label("loading").is_some());
    assert_eq!(gates.launches(), 1);
    assert!(!harness.ctx.has_requested_repaint());

    gates.complete(0, "hello");
    wait_for_repaint(&harness);

    harness.run();
    assert!(harness.query_by_label("ready: hello").is_some());
    assert!(harness.query_by_label("loading").is_none());
}

#[test]
fn ready_reference_lives_next_to_a_state_guard() {
    let gates = Gates::default();
    let gates_in_app = gates.clone();
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let gates = gates_in_app.clone();
            run_app(ui, store, move |cx| {
                // The `&Poll<T>` borrows the store, not the `Cx`, so the guard
                // below can be taken and mutated while it is alive.
                let value = use_future(cx, (), || gates.open());
                let mut count = use_state(cx, || 0i32);
                *count += 1;
                cx.ui().label(format!("count: {}", *count));
                match value {
                    Poll::Pending => cx.ui().label("loading"),
                    Poll::Ready(value) => cx.ui().label(format!("ready: {value} {}", *count)),
                };
            });
        },
        Store::new(),
    );

    // The body writes state every pass, so it never settles: use `step`.
    harness.step();
    assert!(harness.query_by_label("loading").is_some());

    // `wait_for_repaint` is no help here: the body asks for a repaint on every
    // pass of its own accord. Step until the result lands instead.
    gates.complete(0, "value");
    let deadline = Instant::now() + Duration::from_secs(2);
    while harness.query_by_label_contains("ready: value").is_none() {
        assert!(Instant::now() < deadline, "the result never landed");
        std::thread::sleep(Duration::from_millis(10));
        harness.step();
    }
}

#[test]
fn deps_change_restarts_and_stale_result_is_dropped() {
    let gates = Gates::default();
    let dep = Rc::new(Cell::new(0u32));
    let mut harness = loader_harness(&gates, &dep);

    harness.run();
    assert_eq!(gates.launches(), 1);

    dep.set(1);
    harness.run();
    assert_eq!(gates.launches(), 2, "a deps change builds a new future");
    assert!(harness.query_by_label("loading").is_some());

    // The first future finishes late. Its generation is gone, so the result is
    // dropped and the hook stays pending.
    gates.complete(0, "stale");
    wait_for_repaint(&harness);
    harness.run();
    assert!(harness.query_by_label("loading").is_some());
    assert!(harness.query_by_label("ready: stale").is_none());

    gates.complete(1, "fresh");
    wait_for_repaint(&harness);
    harness.run();
    assert!(harness.query_by_label("ready: fresh").is_some());
}

#[test]
fn stale_result_does_not_overwrite_a_newer_one_before_visit() {
    let gates = Gates::default();
    let dep = Rc::new(Cell::new(0u32));
    let mut harness = loader_harness(&gates, &dep);

    harness.run();
    dep.set(1);
    harness.run();
    assert_eq!(gates.launches(), 2);

    // Both futures finish before the hook is visited again, newest first. The
    // older write has to see that something newer is already there.
    gates.complete(1, "fresh");
    wait_for_repaint(&harness);
    gates.complete(0, "stale");
    // Give the stale write its chance to land. Waiting longer can only make the
    // test stricter, never flakier: a correct write never happens at all.
    std::thread::sleep(Duration::from_millis(100));

    harness.run();
    assert!(harness.query_by_label("ready: fresh").is_some());
    assert!(harness.query_by_label("ready: stale").is_none());
}

#[test]
fn a_two_pass_frame_spawns_once() {
    let gates = Gates::default();
    let gates_in_app = gates.clone();
    let max_pass_index = Rc::new(Cell::new(0usize));
    let max_pass_index_in_app = Rc::clone(&max_pass_index);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let pass_index = ui.ctx().current_pass_index();
            max_pass_index_in_app.set(max_pass_index_in_app.get().max(pass_index));
            if pass_index == 0 {
                ui.ctx().request_discard("future test");
            }
            let gates = gates_in_app.clone();
            run_app(ui, store, move |cx| {
                let value = use_future(cx, (), || gates.open());
                assert!(value.is_pending());
            });
        },
        Store::new(),
    );
    harness
        .ctx
        .options_mut(|o| o.max_passes = NonZeroUsize::new(2).unwrap());

    harness.step();
    assert_eq!(
        max_pass_index.get(),
        1,
        "the frame must have run two passes"
    );
    assert_eq!(
        gates.launches(),
        1,
        "the second pass sees the same deps and must not launch again"
    );
}

#[test]
fn unmount_while_pending_does_not_panic_and_frees_the_slot() {
    let gates = Gates::default();
    let show = Rc::new(Cell::new(false));

    let gates_in_app = gates.clone();
    let show_in_app = Rc::clone(&show);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let gates = gates_in_app.clone();
            let show = Rc::clone(&show_in_app);
            run_app(ui, store, move |cx| {
                use_state(cx, || 0i32);
                if show.get() {
                    let value = use_future(cx, (), || gates.open());
                    cx.ui().label(format!("pending: {}", value.is_pending()));
                }
            });
        },
        Store::new(),
    );

    harness.run();
    let baseline = harness.state().len();

    show.set(true);
    harness.run();
    assert!(harness.state().len() > baseline);
    assert_eq!(gates.launches(), 1);

    show.set(false);
    harness.run();
    assert_eq!(
        harness.state().len(),
        baseline,
        "both of the hook's slots must be swept"
    );
    assert!(!harness.ctx.has_requested_repaint());

    // The future still owns its half of the inbox, so finishing is harmless.
    gates.complete(0, "nobody is listening");
    wait_for_repaint(&harness);
    harness.run();
}

#[test]
fn immediately_ready_future_lands_on_the_next_frame() {
    // The hook only appears once the test asks for it: `Harness::new` already
    // runs the app until it settles, which would swallow the first visit.
    let start = Rc::new(Cell::new(false));
    let start_in_app = Rc::clone(&start);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let start = Rc::clone(&start_in_app);
            run_app(ui, store, move |cx| {
                if !start.get() {
                    cx.ui().label("idle");
                    return;
                }
                let value = use_future(cx, (), || async { 1u32 });
                match value {
                    Poll::Pending => cx.ui().label("loading"),
                    Poll::Ready(value) => cx.ui().label(format!("ready: {value}")),
                };
            });
        },
        Store::new(),
    );
    harness.run();

    // One frame only. The launch never reads the inbox, however fast the future
    // was, so this assertion cannot race with the thread.
    start.set(true);
    harness.step();
    assert!(harness.query_by_label("loading").is_some());

    wait_for_repaint(&harness);
    harness.run();
    assert!(harness.query_by_label("ready: 1").is_some());
}

enum Msg {
    Add(i32),
}

#[test]
fn spawn_with_dispatch_lands() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                let (count, dispatch) =
                    use_reducer(cx, |s: &mut i32, Msg::Add(n)| *s += n, || 0i32);
                if cx.ui().button("load").clicked() {
                    let dispatch = dispatch.clone();
                    spawn(async move { dispatch.send(Msg::Add(41)) });
                }
                cx.ui().label(format!("count: {}", *count));
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("count: 0").is_some());

    // The click starts the future, which reports through `Dispatch`. Pointer
    // input makes egui ask for repaints of its own, so `wait_for_repaint` says
    // nothing here: run until the message has actually been applied. That
    // `send` requests a repaint is pinned down by `reducer.rs`.
    harness.get_by_label("load").click();
    harness.step();
    let deadline = Instant::now() + Duration::from_secs(2);
    while harness.query_by_label("count: 41").is_none() {
        assert!(Instant::now() < deadline, "the dispatch never landed");
        std::thread::sleep(Duration::from_millis(10));
        harness.run();
    }
}

#[test]
fn pending_is_counted_by_the_nearest_boundary() {
    let gates = Gates::default();
    let counts = Rc::new(Cell::new((0usize, 0usize)));

    let gates_in_app = gates.clone();
    let counts_in_app = Rc::clone(&counts);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let gates = gates_in_app.clone();
            let counts = Rc::clone(&counts_in_app);
            run_app(ui, store, move |cx| {
                let store = cx.store;
                store.begin_suspense();
                use_future(cx, (), || gates.open());
                use_future(cx, (), || gates.open());
                use_future(cx, (), || gates.open());
                store.begin_suspense();
                use_future(cx, (), || gates.open());
                let inner = store.end_suspense();
                let outer = store.end_suspense();
                counts.set((outer, inner));
            });
        },
        Store::new(),
    );

    harness.run();
    assert_eq!(gates.launches(), 4);
    assert_eq!(
        counts.get(),
        (3, 1),
        "the inner boundary catches its own pending hook"
    );

    // The third hook of the outer boundary becomes ready.
    gates.complete(2, "done");
    wait_for_repaint(&harness);
    harness.run();
    assert_eq!(counts.get(), (2, 1));
}
