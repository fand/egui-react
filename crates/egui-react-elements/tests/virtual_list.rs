//! `<VirtualList>` draws the rows in view and no others.

mod common;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;
use egui_react_elements::prelude::*;

const ROWS: usize = 10_000;
const ROW_H: f32 = 18.0;

/// A row with state of its own, to show the closure is a normal component
/// body and not a formatting callback.
#[component]
fn Row(cx: &mut Cx, index: usize) {
    let mut clicks = use_state(cx, || 0u32);
    rsx! {
        <View direction="row" gap={8} align="center" w="100%">
            <Text>{format!("row {index}")}</Text>
            <Button on_click={|| *clicks += 1}>"+"</Button>
        </View>
    }
}

fn harness<'a>() -> Harness<'a, Store> {
    Harness::builder()
        .with_size(egui::vec2(300.0, 300.0))
        .build_ui_state(
            |ui, store: &mut Store| {
                run_app(ui, store, |cx| {
                    rsx! {
                        <View direction="column" w="100%" h={280.0}>
                            <VirtualList
                                rows={ROWS}
                                row_h={ROW_H}
                                grow={1.0}
                                render={|cx: &mut Cx<'_, '_>, i: usize| {
                                    rsx! { <Row index={i}/> }.show(cx);
                                }}
                            />
                        </View>
                    }
                    .show(cx);
                });
            },
            Store::new(),
        )
}

/// A row that says which index it is and how often it was clicked, so a test
/// can read both back by label.
#[component]
fn CountedRow(cx: &mut Cx, index: usize) {
    let mut clicks = use_state(cx, || 0u32);
    let bump = format!("bump {index}");
    rsx! {
        <View direction="row" gap={8} align="center" w="100%">
            <Text>{format!("item {index}")}</Text>
            <Text>{format!("clicks {index}:{}", *clicks)}</Text>
            <Button label={bump.as_str()} on_click={|| *clicks += 1}>"+"</Button>
        </View>
    }
}

fn counted_harness<'a>() -> Harness<'a, Store> {
    Harness::builder()
        .with_size(egui::vec2(300.0, 300.0))
        .build_ui_state(
            |ui, store: &mut Store| {
                run_app(ui, store, |cx| {
                    rsx! {
                        <View direction="column" w="100%" h={280.0}>
                            <VirtualList
                                rows={ROWS}
                                row_h={ROW_H}
                                grow={1.0}
                                render={|cx: &mut Cx<'_, '_>, i: usize| {
                                    rsx! { <CountedRow index={i}/> }.show(cx);
                                }}
                            />
                        </View>
                    }
                    .show(cx);
                });
            },
            Store::new(),
        )
}

/// Scroll the list by `points`, positive downwards, one frame per call.
///
/// Wheel events rather than `Node::scroll_down`, so the distance is a number
/// this test picks. `TouchPhase::Start` turns off egui's wheel smoothing, so
/// one call moves exactly this far.
fn scroll(harness: &mut Harness<'_, Store>, points: f32) {
    let input = harness.input_mut();
    input
        .events
        .push(egui::Event::PointerMoved(egui::pos2(150.0, 150.0)));
    for (phase, delta) in [
        (egui::TouchPhase::Start, egui::Vec2::ZERO),
        (egui::TouchPhase::Move, egui::vec2(0.0, -points)),
    ] {
        input.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            phase,
            delta,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.step();
}

/// Which of the first `WINDOW` rows show `clicks`.
const WINDOW: usize = 80;

fn rows_with_clicks(harness: &Harness<'_, Store>, clicks: u32) -> Vec<usize> {
    (0..WINDOW)
        .filter(|i| {
            harness
                .query_by_label(&format!("clicks {i}:{clicks}"))
                .is_some()
        })
        .collect()
}

/// Hooks are keyed by the row, the taffy tree by the slot the row sits in. So a
/// row that moves to another slot takes its state with it, and the row that
/// takes over the slot does not inherit it.
#[test]
fn row_state_follows_the_row_not_the_slot() {
    let mut harness = counted_harness();
    harness.run();
    harness.run();

    for _ in 0..3 {
        harness.get_by_label("bump 5").click();
        harness.run();
    }
    assert_eq!(rows_with_clicks(&harness, 3), vec![5]);

    // Two rows' worth: row 5 is still on screen, in a different slot.
    scroll(&mut harness, ROW_H * 2.0);
    assert!(
        harness.query_by_label("item 5").is_some(),
        "row 5 should still be on screen",
    );
    assert_eq!(
        rows_with_clicks(&harness, 3),
        vec![5],
        "the count belongs to row 5, not to the slot it used to be in",
    );

    // Far enough that row 5 unmounts, then back. Its state is gone, the way
    // React drops the state of an unmounted component; what matters here is
    // that no other row picked it up.
    for _ in 0..20 {
        scroll(&mut harness, ROW_H * 3.0);
    }
    assert!(harness.query_by_label("item 5").is_none());
    for _ in 0..20 {
        scroll(&mut harness, -ROW_H * 3.0);
    }
    assert!(
        harness.query_by_label("item 5").is_some(),
        "row 5 should be back on screen",
    );
    assert!(rows_with_clicks(&harness, 3).is_empty());
}

/// Rows used to open a taffy tree keyed by their index, and egui_taffy keeps one
/// state entry per tree in egui memory forever. Keyed by slot, a long scroll
/// reuses the same handful of trees, so memory stops growing with the distance
/// scrolled.
#[test]
fn egui_memory_does_not_grow_with_scroll_distance() {
    let mut harness = counted_harness();
    harness.run();
    harness.run();

    for _ in 0..20 {
        scroll(&mut harness, ROW_H * 4.0);
    }
    let after_short = harness.ctx.data(|d| d.len());
    for _ in 0..200 {
        scroll(&mut harness, ROW_H * 4.0);
    }
    let after_long = harness.ctx.data(|d| d.len());

    assert_eq!(
        after_short, after_long,
        "egui memory grew from {after_short} to {after_long} entries over a longer scroll",
    );
}

#[test]
fn only_the_visible_rows_are_drawn() {
    let mut harness = harness();
    harness.run();
    harness.run();

    let drawn = (0..ROWS)
        .filter(|i| harness.query_by_label(&format!("row {i}")).is_some())
        .count();
    assert!(
        (5..60).contains(&drawn),
        "expected the viewport's worth of rows, drew {drawn}",
    );
    assert!(harness.query_by_label("row 0").is_some());
    assert!(harness.query_by_label("row 9999").is_none());
}

/// Scrolling brings later rows in and drops earlier ones.
#[test]
fn scrolling_moves_the_window_of_rows() {
    let mut harness = harness();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("row 0").is_some());

    // Scroll until the first row is gone, or give up. Each `scroll_down` moves
    // by one wheel notch, and the first row leaves after a few of them.
    for _ in 0..30 {
        if harness.query_by_label("row 0").is_none() {
            break;
        }
        harness.get_by_label("row 0").scroll_down();
        harness.run();
    }

    assert!(
        harness.query_by_label("row 0").is_none(),
        "the first row should have scrolled out",
    );
    let drawn: Vec<usize> = (0..ROWS)
        .filter(|i| harness.query_by_label(&format!("row {i}")).is_some())
        .collect();
    assert!(!drawn.is_empty(), "something should still be on screen");
    assert!(
        drawn[0] > 0,
        "the visible window should have moved: {:?}",
        &drawn[..drawn.len().min(3)],
    );
}
