//! `<VirtualList>` draws the rows in view and no others.

mod common;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use react_egui::prelude::*;
use react_egui_elements::prelude::*;

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
