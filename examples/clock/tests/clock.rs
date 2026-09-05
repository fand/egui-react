//! The stopwatch counts egui's own clock, and the effect's cleanup runs when
//! the child goes away.

use clock::App;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use react_egui::prelude::*;
use react_egui_app::{root_id, root_style};

/// A quarter of a second per frame, egui_kittest's default, so `i.time` moves
/// in steps a test can predict.
fn harness<'a>() -> Harness<'a, Store> {
    Harness::builder()
        .with_size(egui::vec2(420.0, 520.0))
        .build_ui_state(
            |ui, store: &mut Store| {
                store.begin_pass(ui.ctx());
                {
                    let store: &Store = store;
                    let mut cx = Cx::new(store, ui, root_id());
                    // A fixed wall clock, so the test is not racing midnight.
                    let view = rsx! { <App now={12 * 3600 + 34 * 60 + 56}/> };
                    cx.root_container(root_id(), root_style(), |cx| view.show(cx));
                }
                store.end_pass();
            },
            Store::new(),
        )
}

/// The stopwatch reading, as text.
fn elapsed(harness: &Harness<'_, Store>) -> String {
    harness
        .get_all_by_role(egui::accesskit::Role::Label)
        .filter_map(|node| node.accesskit_node().value())
        .find(|text| text.len() == 8 && text.as_bytes()[2] == b':' && text.as_bytes()[5] == b'.')
        .expect("the stopwatch reading")
}

#[test]
fn the_wall_clock_shows_the_time_it_is_given() {
    let mut harness = harness();
    harness.run();
    assert!(harness.query_by_label("12:34:56").is_some());
}

#[test]
fn the_stopwatch_runs_and_stops() {
    let mut harness = harness();
    harness.run();
    assert_eq!(elapsed(&harness), "00:00.00");

    harness.get_by_label("start").click();
    // `step`, not `run`: a running stopwatch asks for the next frame every
    // frame, so `run` would never decide it had settled.
    harness.step();
    harness.step();
    harness.step();
    let running = elapsed(&harness);
    assert_ne!(running, "00:00.00", "the stopwatch did not advance");

    harness.get_by_label("lap").click();
    harness.step();
    harness.step();
    let laps: Vec<String> = harness
        .get_all_by_role(egui::accesskit::Role::Label)
        .filter_map(|node| node.accesskit_node().value())
        .filter(|label| label.starts_with("lap 1  "))
        .collect();
    assert_eq!(laps.len(), 1, "expected one lap line");

    harness.get_by_label("stop").click();
    harness.step();
    harness.step();
    let stopped = elapsed(&harness);
    harness.run();
    assert_eq!(
        elapsed(&harness),
        stopped,
        "a stopped stopwatch kept counting"
    );

    harness.get_by_label("reset").click();
    harness.run();
    assert_eq!(elapsed(&harness), "00:00.00");
}

/// The effect's cleanup is what puts "unmounted" in the log.
#[test]
fn unmounting_the_ticker_runs_its_cleanup() {
    let mut harness = harness();
    harness.run();

    assert!(harness.query_by_label("ticker: mounted").is_some());
    assert!(harness.query_by_label("ticker mounted").is_some());
    assert!(harness.query_by_label("ticker unmounted").is_none());

    harness.get_by_label("show ticker").click();
    // One pass to apply the write, one for the sweep's cleanup message to be
    // applied on the next visit to the reducer.
    harness.run();
    harness.run();

    assert!(harness.query_by_label("ticker: mounted").is_none());
    assert!(harness.query_by_label("ticker unmounted").is_some());
    assert!(harness.query_by_label("ticker mounted").is_some());
}
