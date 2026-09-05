//! Plan 3.2 / tests 6-10 .. 6-16: `<Suspense>` shows its fallback until every
//! `use_future` below it is ready, switches within one frame, keeps its
//! children's state across a suspension, lets the nearest boundary catch its
//! own pending hooks, lays its children out in the parent's taffy tree, and
//! ignores input while the children are off screen.

mod common;

use std::cell::{Cell, RefCell};
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use react_egui::prelude::*;
use react_egui_elements::prelude::*;

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

/// Wait for a finished future to ask for a repaint, for at most two seconds.
///
/// The request is made after the result was written, so the next pass sees it.
fn wait_for_repaint(harness: &Harness<'static, Store>) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if harness.ctx.has_requested_repaint() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for the future to request a repaint");
}

/// A child that waits for its future and only then draws anything.
///
/// The `let`-`else` is what a component writes instead of React's throw.
#[component]
fn Waiter(cx: &mut Cx, name: &str, gates: Gates, #[prop(default)] attempt: u32) {
    let mut count = use_state(cx, || 0i32);
    let value = use_future(cx, (name, attempt), || gates.open());
    let Poll::Ready(value) = value else { return };
    cx.ui().label(format!("{name}: {value} {}", *count));
    if cx.ui().button(format!("{name} +")).clicked() {
        *count += 1;
    }
}

/// Two waiters inside one boundary.
#[component]
fn App(cx: &mut Cx, gates: Gates, #[prop(default)] attempt: u32) {
    let b = gates.clone();
    rsx! {
        <Suspense fallback={view(|cx| { cx.ui().label("loading"); })}>
            <Waiter name="a" gates={gates} attempt={attempt}/>
            <Waiter name="b" gates={b} attempt={attempt}/>
        </Suspense>
    }
}

/// Highest `Context::current_pass_index` the harness has seen.
type PassIndex = Rc<Cell<usize>>;

fn app_harness(
    gates: &Gates,
    attempt: &Rc<Cell<u32>>,
    passes: &PassIndex,
) -> Harness<'static, Store> {
    let gates = gates.clone();
    let attempt = Rc::clone(attempt);
    let passes = Rc::clone(passes);
    let harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            passes.set(passes.get().max(ui.ctx().current_pass_index()));
            let gates = gates.clone();
            let attempt = attempt.get();
            run_app(ui, store, move |cx| {
                rsx! { <App gates={gates} attempt={attempt}/> }.show(cx);
            });
        },
        Store::new(),
    );
    harness
        .ctx
        .options_mut(|o| o.max_passes = NonZeroUsize::new(3).unwrap());
    harness
}

/// Whether a label is drawn where the user can see it.
///
/// Suspended children are drawn into an invisible `Ui` far off screen. egui
/// still puts their widgets in the accessibility tree, so "not shown" has to be
/// checked by position rather than by the node being absent.
fn shows(harness: &Harness<'static, Store>, label: &str) -> bool {
    harness
        .query_by_label_contains(label)
        .is_some_and(|node| node.rect().min.x > 0.0)
}

/// Click where `pos` is, without going through the accessibility tree.
fn click_at(harness: &mut Harness<'static, Store>, pos: egui::Pos2) {
    harness.hover_at(pos);
    harness.drag_at(pos);
    harness.drop_at(pos);
    harness.run();
}

#[test]
fn fallback_while_pending_then_children() {
    let gates = Gates::default();
    let attempt = Rc::new(Cell::new(0u32));
    let passes: PassIndex = Rc::default();
    let mut harness = app_harness(&gates, &attempt, &passes);

    harness.run();
    assert_eq!(gates.launches(), 2, "both children ran off screen");
    assert!(shows(&harness, "loading"));
    assert!(!shows(&harness, "a: "));
    assert!(!shows(&harness, "b: "));

    gates.complete(0, "one");
    gates.complete(1, "two");
    wait_for_repaint(&harness);
    harness.run();

    assert!(shows(&harness, "a: one"));
    assert!(shows(&harness, "b: two"));
    assert!(!shows(&harness, "loading"));
}

#[test]
fn partial_children_are_never_shown() {
    let gates = Gates::default();
    let attempt = Rc::new(Cell::new(0u32));
    let passes: PassIndex = Rc::default();
    let mut harness = app_harness(&gates, &attempt, &passes);

    harness.run();
    gates.complete(0, "one");
    wait_for_repaint(&harness);
    harness.run();

    assert!(shows(&harness, "loading"));
    assert!(
        !shows(&harness, "a: one"),
        "the ready child must not be shown while its sibling waits"
    );
}

#[test]
fn switch_happens_in_one_frame() {
    let gates = Gates::default();
    let attempt = Rc::new(Cell::new(0u32));
    let passes: PassIndex = Rc::default();
    let mut harness = app_harness(&gates, &attempt, &passes);

    harness.run();
    gates.complete(0, "one");
    gates.complete(1, "two");
    wait_for_repaint(&harness);

    // A single frame: the boundary resolves in pass 1 and `request_discard`
    // redraws the children visibly in pass 2, so no half-drawn frame is shown.
    passes.set(0);
    harness.step();
    assert!(shows(&harness, "a: one"));
    assert!(!shows(&harness, "loading"));
    assert!(
        passes.get() >= 1,
        "the switch must have run a second pass in the same frame"
    );
}

#[test]
fn children_state_survives_suspension() {
    let gates = Gates::default();
    let attempt = Rc::new(Cell::new(0u32));
    let passes: PassIndex = Rc::default();
    let mut harness = app_harness(&gates, &attempt, &passes);

    harness.run();
    gates.complete(0, "one");
    gates.complete(1, "two");
    wait_for_repaint(&harness);
    harness.run();

    harness.get_by_label("a +").click();
    harness.run();
    assert!(shows(&harness, "a: one 1"));

    // A deps change restarts both futures, so the boundary suspends again.
    attempt.set(1);
    harness.run();
    assert!(shows(&harness, "loading"));
    assert_eq!(gates.launches(), 4);

    gates.complete(2, "three");
    gates.complete(3, "four");
    wait_for_repaint(&harness);
    harness.run();
    assert!(
        shows(&harness, "a: three 1"),
        "the counter must have survived the suspension"
    );
}

/// The outer boundary's own child, plus a boundary of its own around another.
#[component]
fn Nested(cx: &mut Cx, gates: Gates) {
    let inner = gates.clone();
    rsx! {
        <Suspense fallback={view(|cx| { cx.ui().label("outer loading"); })}>
            <Waiter name="outer" gates={gates}/>
            <Suspense fallback={view(|cx| { cx.ui().label("inner loading"); })}>
                <Waiter name="inner" gates={inner}/>
            </Suspense>
        </Suspense>
    }
}

#[test]
fn nested_boundary_catches_its_own() {
    let gates = Gates::default();
    let gates_in_app = gates.clone();
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let gates = gates_in_app.clone();
            run_app(ui, store, move |cx| {
                rsx! { <Nested gates={gates}/> }.show(cx);
            });
        },
        Store::new(),
    );
    harness
        .ctx
        .options_mut(|o| o.max_passes = NonZeroUsize::new(3).unwrap());

    harness.run();
    assert_eq!(gates.launches(), 2);
    assert!(shows(&harness, "outer loading"));

    // Only the outer boundary's own child becomes ready. The inner one is
    // counted by the inner boundary, so the outer one resolves anyway.
    gates.complete(0, "done");
    wait_for_repaint(&harness);
    harness.run();

    assert!(!shows(&harness, "outer loading"));
    assert!(shows(&harness, "outer: done"));
    assert!(shows(&harness, "inner loading"));
    assert!(!shows(&harness, "inner: "));
}

/// A resolved boundary inside a row: its child has to grow in the parent's tree.
#[component]
fn InARow(cx: &mut Cx) {
    rsx! {
        <View direction="row" w={300.0}>
            <Suspense fallback={view(|cx| { cx.ui().label("loading"); })}>
                <Text grow={1.0}>"grown"</Text>
            </Suspense>
            <Text>"end"</Text>
        </View>
    }
}

#[test]
fn inside_a_view_children_lay_out_in_the_parent_tree() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                rsx! { <InARow/> }.show(cx);
            });
        },
        Store::new(),
    );
    harness
        .ctx
        .options_mut(|o| o.max_passes = NonZeroUsize::new(3).unwrap());

    // Nothing below the boundary waits, so it resolves on the first frame.
    harness.run();
    assert!(!shows(&harness, "loading"));

    let grown = harness.get_by_label("grown").rect();
    let end = harness.get_by_label("end").rect();
    assert!(
        end.left() > 200.0,
        "the suspense child did not grow inside the row: grown {grown:?}, end {end:?}"
    );
}

/// A child that draws a button whether or not its future is ready.
#[component]
fn Clickable(cx: &mut Cx, gates: Gates, attempt: u32) {
    let mut count = use_state(cx, || 0i32);
    let ready = use_future(cx, attempt, || gates.open()).is_ready();
    if cx.ui().button("child").clicked() {
        *count += 1;
    }
    cx.ui().label(format!("clicks: {} ready: {ready}", *count));
}

#[test]
fn offscreen_children_do_not_react_to_input() {
    let gates = Gates::default();
    let attempt = Rc::new(Cell::new(0u32));

    let gates_in_app = gates.clone();
    let attempt_in_app = Rc::clone(&attempt);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let gates = gates_in_app.clone();
            let attempt = attempt_in_app.get();
            run_app(ui, store, move |cx| {
                rsx! {
                    <Suspense fallback={view(|cx| { cx.ui().label("loading"); })}>
                        <Clickable gates={gates} attempt={attempt}/>
                    </Suspense>
                }
                .show(cx);
            });
        },
        Store::new(),
    );
    harness
        .ctx
        .options_mut(|o| o.max_passes = NonZeroUsize::new(3).unwrap());

    harness.run();
    gates.complete(0, "one");
    wait_for_repaint(&harness);
    harness.run();

    // Where the button sits once it is visible. Clicking there works.
    let button = harness.get_by_label("child").rect().center();
    click_at(&mut harness, button);
    assert!(shows(&harness, "clicks: 1"));

    // Suspend again and click the very same spot: the fallback is there now and
    // the children are off screen in an invisible, disabled `Ui`.
    attempt.set(1);
    harness.run();
    assert!(shows(&harness, "loading"));
    click_at(&mut harness, button);

    gates.complete(1, "two");
    wait_for_repaint(&harness);
    harness.run();
    assert!(
        shows(&harness, "clicks: 1"),
        "the offscreen button must not have been clicked"
    );
}
