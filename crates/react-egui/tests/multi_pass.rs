//! ARCHITECTURE.md 10, item 5 and 5.3: when egui runs a second pass for the
//! same frame, the second pass sees no input events, so a click handler fires
//! exactly once and `use_effect` does not re-run.

mod common;

use std::cell::Cell;
use std::num::NonZeroUsize;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_taffy::TuiBuilderLogic as _;
use react_egui::prelude::*;

/// Counters shared between the test body and the app closure.
#[derive(Default)]
struct Counters {
    /// Highest `Context::current_pass_index` seen, i.e. (passes per frame - 1).
    max_pass_index: Cell<usize>,
    /// Times the click handler ran.
    clicks: Cell<u32>,
    /// Times the `use_effect` body ran.
    effects: Cell<u32>,
    /// Set by the test to ask for a discard on the first pass of a frame.
    discard: Cell<bool>,
}

fn body(cx: &mut Cx<'_, '_>, counters: &Rc<Counters>) {
    let effect_counters = Rc::clone(counters);
    use_effect(cx, (), move || {
        effect_counters
            .effects
            .set(effect_counters.effects.get() + 1);
    });

    let mut count = use_state(cx, || 0i32);
    if cx.ui().button("bump").clicked() {
        *count += 1;
        counters.clicks.set(counters.clicks.get() + 1);
    }
    cx.ui().label(format!("count: {}", *count));
}

#[test]
fn manual_discard_runs_two_passes_but_one_handler() {
    let counters = Rc::new(Counters::default());
    let counters_in_app = Rc::clone(&counters);

    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let counters = Rc::clone(&counters_in_app);
            let pass_index = ui.ctx().current_pass_index();
            counters
                .max_pass_index
                .set(counters.max_pass_index.get().max(pass_index));
            if counters.discard.get() && pass_index == 0 {
                ui.ctx().request_discard("multi_pass test");
            }
            run_app(ui, store, |cx| body(cx, &counters));
        },
        Store::new(),
    );
    harness
        .ctx
        .options_mut(|o| o.max_passes = NonZeroUsize::new(2).unwrap());

    harness.run();
    assert_eq!(counters.effects.get(), 1);

    counters.max_pass_index.set(0);
    counters.discard.set(true);
    harness.get_by_label("bump").click();
    harness.step();
    counters.discard.set(false);

    assert_eq!(
        counters.max_pass_index.get(),
        1,
        "the frame must have run two passes"
    );
    assert_eq!(counters.clicks.get(), 1, "the handler must fire once");
    assert_eq!(counters.effects.get(), 1, "the effect must not re-run");

    harness.run();
    assert!(harness.query_by_label("count: 1").is_some());
}

/// The same check, but with `egui_taffy` deciding to run the second pass: a
/// click makes the label wider, taffy notices the layout changed and calls
/// `request_discard` itself.
fn taffy_body(cx: &mut Cx<'_, '_>, counters: &Rc<Counters>) {
    let effect_counters = Rc::clone(counters);
    use_effect(cx, (), move || {
        effect_counters
            .effects
            .set(effect_counters.effects.get() + 1);
    });

    let mut count = use_state(cx, || 0i32);
    let (store, scope) = (cx.store, cx.scope_id());
    egui_taffy::tui(cx.ui(), egui::Id::new("flex"))
        .reserve_available_space()
        .style(egui_taffy::taffy::Style {
            display: egui_taffy::taffy::Display::Flex,
            flex_direction: egui_taffy::taffy::FlexDirection::Row,
            ..Default::default()
        })
        .show(|tui| {
            tui.ui(|ui| {
                let mut cx = Cx::new(store, ui, scope);
                if cx.ui().button("bump").clicked() {
                    *count += 1;
                    counters.clicks.set(counters.clicks.get() + 1);
                }
            });
            tui.ui(|ui| {
                let mut cx = Cx::new(store, ui, scope);
                // The width of this leaf depends on the state.
                cx.ui().label("wide ".repeat(*count as usize + 1));
            });
        });
}

#[test]
fn taffy_discard_runs_two_passes_but_one_handler() {
    let counters = Rc::new(Counters::default());
    let counters_in_app = Rc::clone(&counters);

    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let counters = Rc::clone(&counters_in_app);
            let pass_index = ui.ctx().current_pass_index();
            counters
                .max_pass_index
                .set(counters.max_pass_index.get().max(pass_index));
            run_app(ui, store, |cx| taffy_body(cx, &counters));
        },
        Store::new(),
    );
    harness
        .ctx
        .options_mut(|o| o.max_passes = NonZeroUsize::new(2).unwrap());

    harness.run();
    assert_eq!(counters.effects.get(), 1);

    // A frame in which nothing changes needs one pass only, so the second pass
    // below really is taffy reacting to the new layout.
    counters.max_pass_index.set(0);
    harness.step();
    assert_eq!(counters.max_pass_index.get(), 0);

    counters.max_pass_index.set(0);
    harness.get_by_label("bump").click();
    harness.step();

    assert_eq!(
        counters.max_pass_index.get(),
        1,
        "egui_taffy must have asked for a second pass"
    );
    assert_eq!(counters.clicks.get(), 1, "the handler must fire once");
    assert_eq!(counters.effects.get(), 1, "the effect must not re-run");
}

/// Documents the limit of "the second pass re-runs nothing": an effect whose
/// deps are derived from state that a handler changed *during* pass 1 does run
/// again in pass 2, because its deps genuinely differ. The effect body sees
/// `[0, 1]` inside a single frame.
#[test]
fn effect_deps_changed_during_pass_one_rerun_in_pass_two() {
    let runs: Rc<std::cell::RefCell<Vec<i32>>> = Rc::new(std::cell::RefCell::new(Vec::new()));
    let discard = Rc::new(Cell::new(false));

    let runs_in_app = Rc::clone(&runs);
    let discard_in_app = Rc::clone(&discard);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let runs = Rc::clone(&runs_in_app);
            if discard_in_app.get() && ui.ctx().current_pass_index() == 0 {
                ui.ctx().request_discard("effect deps test");
            }
            run_app(ui, store, |cx| {
                let mut count = use_state(cx, || 0i32);
                let deps = *count;
                let runs = Rc::clone(&runs);
                use_effect(cx, deps, move || runs.borrow_mut().push(deps));
                if cx.ui().button("bump").clicked() {
                    *count += 1;
                }
                cx.ui().label(format!("count: {}", *count));
            });
        },
        Store::new(),
    );
    harness
        .ctx
        .options_mut(|o| o.max_passes = NonZeroUsize::new(2).unwrap());

    harness.run();
    assert_eq!(*runs.borrow(), [0]);

    discard.set(true);
    harness.get_by_label("bump").click();
    harness.step();
    discard.set(false);

    assert_eq!(*runs.borrow(), [0, 1], "deps changed, so pass 2 re-ran it");
}
