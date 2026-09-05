//! Plan 3.4 / test 4-5: the same guarantee the core's `multi_pass` test pins
//! down, now with real elements: when `<View>` makes egui run a second pass for
//! the same frame, the click handler still fires exactly once and `use_effect`
//! does not re-run.

mod common;

use std::cell::Cell;
use std::num::NonZeroUsize;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use react_egui::prelude::*;
use react_egui_elements::prelude::*;

#[derive(Default)]
struct Counters {
    /// Highest `Context::current_pass_index` seen, i.e. (passes per frame - 1).
    max_pass_index: Cell<usize>,
    /// Times the click handler ran.
    clicks: Cell<u32>,
    /// Times the `use_effect` body ran.
    effects: Cell<u32>,
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
            run_app(ui, store, move |cx| {
                let effect_counters = Rc::clone(&counters);
                use_effect(cx, (), move || {
                    effect_counters
                        .effects
                        .set(effect_counters.effects.get() + 1);
                });

                let mut count = use_state(cx, || 0i32);
                rsx! {
                    <View direction="row" gap={4}>
                        <Button on_click={|| {
                            *count += 1;
                            counters.clicks.set(counters.clicks.get() + 1);
                        }}>"bump"</Button>
                        // Read after the button, so a click widens this in the
                        // very same pass; taffy then asks for a second one.
                        <Text>{"wide ".repeat(*count as usize + 1)}</Text>
                    </View>
                }
                .show(cx);
            });
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
        "<View> must have asked for a second pass"
    );
    assert_eq!(counters.clicks.get(), 1, "the handler must fire once");
    assert_eq!(counters.effects.get(), 1, "the effect must not re-run");

    harness.run();
    assert!(harness.query_by_label("wide wide ").is_some());
}
