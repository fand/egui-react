//! `<VirtualList>` draws the rows in view and no others.

mod common;

use std::cell::Cell;
use std::rc::Rc;

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

/// Rows used to open a taffy tree keyed by their index, so every row that
/// scrolled in built a tree of its own. Keyed by slot, a long scroll reuses the
/// same handful of trees.
///
/// Both counts are checked: the store's, which is where the trees live, and
/// egui's own memory, which is what a widget inside a row would grow.
#[test]
fn memory_does_not_grow_with_scroll_distance() {
    let mut harness = counted_harness();
    harness.run();
    harness.run();

    for _ in 0..20 {
        scroll(&mut harness, ROW_H * 4.0);
    }
    let (trees_short, data_short) = (harness.state().tree_count(), harness.ctx.data(|d| d.len()));
    for _ in 0..200 {
        scroll(&mut harness, ROW_H * 4.0);
    }
    let (trees_long, data_long) = (harness.state().tree_count(), harness.ctx.data(|d| d.len()));

    assert_eq!(
        trees_short, trees_long,
        "layout trees grew from {trees_short} to {trees_long} over a longer scroll",
    );
    assert_eq!(
        data_short, data_long,
        "egui memory grew from {data_short} to {data_long} entries over a longer scroll",
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

/// A row whose layout does not depend on how wide its text is: a fixed column
/// and a growing one, the shape `examples/list-10k` uses.
///
/// The pitch tests are about the row's *root* rect, so nothing inside the row
/// may move on its own.
#[component]
fn PitchRow(cx: &mut Cx, index: usize) {
    rsx! {
        <View direction="row" gap={8} align="center" w="100%" h={ROW_H}>
            <Text w={80.0}>{format!("row {index}")}</Text>
            <Text grow={1.0}>"filler"</Text>
        </View>
    }
}

/// A row with a widget leaf, the shape of list-10k's row: a widget has to draw
/// to be measured, so a slot whose tree was dropped costs a sizing pass.
#[component]
fn ButtonRow(cx: &mut Cx, index: usize) {
    rsx! {
        <View direction="row" gap={8} align="center" w="100%" h={ROW_H}>
            <Text w={80.0}>{format!("row {index}")}</Text>
            <Text grow={1.0}>"filler"</Text>
            <Button>"x"</Button>
        </View>
    }
}

/// A row that draws twice as tall as the height the list was told.
///
/// `<VirtualList>` says every row must be `row_h` tall and that a taller one
/// overlaps the next; this is that case, written down.
#[component]
fn TallRow(cx: &mut Cx, index: usize) {
    rsx! {
        <View direction="column" w="100%" h={ROW_H * 2.0}>
            <Text>{format!("tall {index}")}</Text>
            <Text>{format!("more {index}")}</Text>
        </View>
    }
}

/// A list whose rows sit at exactly `ROW_H` and that counts discard requests.
///
/// `item_spacing.y` is zeroed, because `show_rows` puts the rows at
/// `row_h + item_spacing.y` and the test wants one number. `tall` swaps in a
/// row that draws over its height.
#[derive(Clone, Copy)]
enum RowKind {
    Pitch,
    Tall,
    Button,
}

fn pitch_harness<'a>(discards: &Rc<Cell<usize>>, kind: RowKind) -> Harness<'a, Store> {
    let discards = Rc::clone(discards);
    Harness::builder()
        .with_size(egui::vec2(300.0, 300.0))
        .build_ui_state(
            move |ui, store: &mut Store| {
                ui.spacing_mut().item_spacing.y = 0.0;
                run_app(ui, store, |cx| {
                    rsx! {
                        <View direction="column" w="100%" h={280.0}>
                            <VirtualList
                                rows={ROWS}
                                row_h={ROW_H}
                                grow={1.0}
                                render={move |cx: &mut Cx<'_, '_>, i: usize| match kind {
                                    RowKind::Pitch => rsx! { <PitchRow index={i}/> }.show(cx),
                                    RowKind::Tall => rsx! { <TallRow index={i}/> }.show(cx),
                                    RowKind::Button => rsx! { <ButtonRow index={i}/> }.show(cx),
                                }}
                            />
                        </View>
                    }
                    .show(cx);
                });
                if ui.ctx().output(|o| o.requested_discard()) {
                    discards.set(discards.get() + 1);
                }
            },
            Store::new(),
        )
}

/// The top of each `prefix i` label that is on screen, in order.
fn label_tops(harness: &Harness<'_, Store>, prefix: &str) -> Vec<(usize, f32)> {
    (0..WINDOW)
        .filter_map(|i| {
            harness
                .query_by_label(&format!("{prefix} {i}"))
                .map(|node| (i, node.rect().top()))
        })
        .collect()
}

/// Consecutive rows sit exactly `ROW_H` apart.
fn assert_row_pitch(tops: &[(usize, f32)]) {
    assert!(tops.len() > 4, "expected a screenful of rows, got {tops:?}");
    for pair in tops.windows(2) {
        let [(a, top_a), (b, top_b)] = pair else {
            unreachable!()
        };
        assert_eq!(b - a, 1, "rows {a} and {b} are not neighbours");
        assert!(
            (top_b - top_a - ROW_H).abs() < 0.01,
            "rows {a} and {b} are {} apart, not {ROW_H}",
            top_b - top_a,
        );
    }
}

/// A row's tree is laid out into a rect of its own size, so scrolling does not
/// change it and nothing has to be laid out again.
///
/// Before this, a row's root rect ran from the row to the bottom of the
/// viewport, so every visible row tree saw a new root size on every scrolled
/// frame.
#[test]
fn scrolling_keeps_the_rows_at_one_pitch_and_asks_for_no_second_pass() {
    let discards = Rc::new(Cell::new(0usize));
    let mut harness = pitch_harness(&discards, RowKind::Pitch);
    harness.run();
    harness.run();
    assert_row_pitch(&label_tops(&harness, "row"));

    // One row per frame, so the visible range keeps its length and no slot is
    // added or dropped.
    discards.set(0);
    for _ in 0..20 {
        scroll(&mut harness, ROW_H);
    }
    assert_eq!(
        discards.get(),
        0,
        "scrolling asked for {} extra passes",
        discards.get(),
    );

    let tops = label_tops(&harness, "row");
    assert!(
        tops.first().unwrap().0 > 0,
        "the window should have moved: {tops:?}",
    );
    assert_row_pitch(&tops);
}

/// A row that draws taller than `row_h` still moves the cursor by `row_h`.
///
/// The row overlaps the one below it — that is the caller's problem, and the
/// element's docs say so. What must not happen is the list drifting out of step
/// with the range `show_rows` computed.
#[test]
fn a_row_taller_than_row_h_still_advances_by_row_h() {
    let discards = Rc::new(Cell::new(0usize));
    let mut harness = pitch_harness(&discards, RowKind::Tall);
    harness.run();
    harness.run();

    let tops = label_tops(&harness, "tall");
    assert_row_pitch(&tops);
    // The overlap: the second line of a row is drawn below where the next row
    // starts.
    let more = harness.get_by_label("more 0").rect();
    assert!(
        more.bottom() > tops[1].1,
        "a double height row should reach into the next row: {more:?} against {}",
        tops[1].1,
    );

    discards.set(0);
    for _ in 0..10 {
        scroll(&mut harness, ROW_H);
    }
    assert_eq!(discards.get(), 0);
    assert_row_pitch(&label_tops(&harness, "tall"));
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

/// Scrolling by a fraction of a row, as a trackpad does, makes the visible
/// range one row longer on some frames and one row shorter on others. The slot
/// that comes and goes must keep its layout between appearances; if the store
/// dropped it the moment it was not drawn, every reappearance would be a new
/// tree, a sizing pass and a discard, and egui would warn about discards on
/// consecutive frames.
#[test]
fn fractional_scrolling_asks_for_no_second_pass() {
    let discards = Rc::new(Cell::new(0usize));
    let mut harness = pitch_harness(&discards, RowKind::Button);
    harness.run();
    harness.run();

    // The first time the extra slot appears it is a new tree and costs one
    // sizing pass, once. That is fine; what must not happen is paying it again
    // on every reappearance.
    for _ in 0..10 {
        scroll(&mut harness, ROW_H * 0.37);
    }
    discards.set(0);
    let mut lengths = std::collections::BTreeSet::new();
    for _ in 0..40 {
        scroll(&mut harness, ROW_H * 0.37);
        lengths.insert(label_tops(&harness, "row").len());
    }
    assert!(
        lengths.len() > 1,
        "the visible range should change length as the offset moves: {lengths:?}",
    );
    assert_eq!(
        discards.get(),
        0,
        "fractional scrolling asked for {} extra passes",
        discards.get(),
    );
}
