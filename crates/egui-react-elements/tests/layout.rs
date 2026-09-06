//! Plan 3.1 / test 4-3: `<View>`'s flex and grid attributes reach taffy, and a
//! `<Text>` does not wrap inside a narrow parent.

mod common;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;
use egui_react_elements::prelude::*;

/// Draw `app` once and hand the test a harness to measure rects on.
fn harness_for(app: impl Fn(&mut Cx<'_, '_>) + 'static) -> Harness<'static, Store> {
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            run_app(ui, store, |cx| app(cx));
        },
        Store::new(),
    );
    harness.run();
    harness
}

#[test]
fn direction_places_the_children() {
    let row = harness_for(|cx| {
        rsx! {
            <View direction="row">
                <Text>"one"</Text>
                <Text>"two"</Text>
            </View>
        }
        .show(cx);
    });
    let (a, b) = (
        row.get_by_label("one").rect(),
        row.get_by_label("two").rect(),
    );
    assert!(a.right() <= b.left(), "row: {a:?} {b:?}");

    let column = harness_for(|cx| {
        rsx! {
            <View direction="column">
                <Text>"one"</Text>
                <Text>"two"</Text>
            </View>
        }
        .show(cx);
    });
    let (a, b) = (
        column.get_by_label("one").rect(),
        column.get_by_label("two").rect(),
    );
    assert!(a.bottom() <= b.top(), "column: {a:?} {b:?}");
}

#[test]
fn justify_and_gap_control_the_main_axis() {
    let packed = harness_for(|cx| {
        rsx! {
            <View direction="row" gap={0}>
                <Text>"one"</Text>
                <Text>"two"</Text>
            </View>
        }
        .show(cx);
    });
    let gap_packed =
        packed.get_by_label("two").rect().left() - packed.get_by_label("one").rect().right();

    let spaced = harness_for(|cx| {
        rsx! {
            <View direction="row" gap={24}>
                <Text>"one"</Text>
                <Text>"two"</Text>
            </View>
        }
        .show(cx);
    });
    let gap_spaced =
        spaced.get_by_label("two").rect().left() - spaced.get_by_label("one").rect().right();
    assert!(
        gap_spaced - gap_packed > 20.0,
        "gap={{24}} must push them apart: {gap_packed} vs {gap_spaced}"
    );

    let between = harness_for(|cx| {
        rsx! {
            <View direction="row" justify="space-between" w={300.0}>
                <Text>"one"</Text>
                <Text>"two"</Text>
            </View>
        }
        .show(cx);
    });
    let gap_between =
        between.get_by_label("two").rect().left() - between.get_by_label("one").rect().right();
    assert!(
        gap_between > 100.0,
        "space-between must push them to the two ends of a 300pt row: {gap_between}"
    );
}

#[test]
fn align_controls_the_cross_axis() {
    let harness = harness_for(|cx| {
        rsx! {
            <View direction="row" align="center" h={80.0}>
                <Text>"small"</Text>
                <Text size={40.0}>"big"</Text>
            </View>
        }
        .show(cx);
    });
    let small = harness.get_by_label("small").rect();
    let big = harness.get_by_label("big").rect();
    assert!(
        (small.center().y - big.center().y).abs() < 2.0,
        "align=center lines the centres up: {small:?} {big:?}"
    );
}

#[test]
fn grow_shares_the_leftover_space() {
    let harness = harness_for(|cx| {
        rsx! {
            <View direction="row" w={300.0}>
                <Text grow={1.0}>"left"</Text>
                <Text>"right"</Text>
            </View>
        }
        .show(cx);
    });
    let left = harness.get_by_label("left").rect();
    let right = harness.get_by_label("right").rect();
    assert!(
        right.left() > left.right() + 50.0,
        "grow pushes the second item to the end: {left:?} {right:?}"
    );
}

#[test]
fn width_padding_and_margin_reach_taffy() {
    let harness = harness_for(|cx| {
        rsx! {
            <View direction="column">
                <View w={200.0} p={16.0}>
                    <Text>"padded"</Text>
                </View>
                <View ml={40.0}>
                    <Text>"indented"</Text>
                </View>
            </View>
        }
        .show(cx);
    });
    let padded = harness.get_by_label("padded").rect();
    let indented = harness.get_by_label("indented").rect();
    assert!(
        padded.left() > indented.left() - 40.0 + 15.0,
        "p={{16}} offsets the child: {padded:?}"
    );
    assert!(
        indented.left() - padded.left() > 20.0,
        "ml={{40}} indents the second row: {padded:?} {indented:?}"
    );
}

#[test]
fn grid_places_children_in_columns() {
    let harness = harness_for(|cx| {
        rsx! {
            <View display="grid" cols={2}>
                <Text>"c1"</Text>
                <Text>"c2"</Text>
                <Text>"c3"</Text>
                <Text>"c4"</Text>
            </View>
        }
        .show(cx);
    });
    let rects: Vec<egui::Rect> = ["c1", "c2", "c3", "c4"]
        .iter()
        .map(|l| harness.get_by_label(l).rect())
        .collect();

    assert!(rects[0].left() < rects[1].left(), "c1 | c2: {rects:?}");
    assert!(
        (rects[0].top() - rects[1].top()).abs() < 1.0,
        "c1 and c2 share a row: {rects:?}"
    );
    assert!(rects[0].top() < rects[2].top(), "c3 wraps: {rects:?}");
    assert!(
        (rects[0].left() - rects[2].left()).abs() < 1.0,
        "c1 and c3 share a column: {rects:?}"
    );
}

#[test]
fn col_span_widens_a_grid_item() {
    let harness = harness_for(|cx| {
        rsx! {
            <View display="grid" cols={2} w={400.0}>
                <Text col_span={2}>"wide"</Text>
                <Text>"c1"</Text>
                <Text>"c2"</Text>
            </View>
        }
        .show(cx);
    });
    let wide = harness.get_by_label("wide").rect();
    let c1 = harness.get_by_label("c1").rect();
    let c2 = harness.get_by_label("c2").rect();
    assert!(wide.top() < c1.top(), "the wide cell has its own row");
    assert!(c1.left() < c2.left(), "the next two share a row");
}

#[test]
fn text_extends_instead_of_wrapping() {
    let long = "the quick brown fox jumps over the lazy dog again and again and again";
    let harness = harness_for(move |cx| {
        rsx! {
            <View direction="column" w={80.0}>
                <Text>{long}</Text>
            </View>
        }
        .show(cx);
    });
    let extended = harness.get_by_label(long).rect();
    assert!(
        extended.width() > 80.0,
        "the default wrap mode is Extend: {extended:?}"
    );

    let harness = harness_for(move |cx| {
        rsx! {
            <View direction="column" w={80.0}>
                <Text wrap>{long}</Text>
            </View>
        }
        .show(cx);
    });
    let wrapped = harness.get_by_label(long).rect();
    assert!(
        wrapped.height() > extended.height() * 1.5,
        "wrap makes it taller: {extended:?} {wrapped:?}"
    );
}

/// A wrapper component takes its caller's `ItemStyle` through `style=` and adds
/// to it with the shorthand attributes; `rsx!` chains the two into one call.
#[component]
fn Chip(cx: &mut Cx, #[prop(default)] style: ItemStyle, label: &str) {
    rsx! {
        <View style={style} p={10.0}>
            <Text>{label}</Text>
        </View>
    }
}

#[test]
fn style_and_shorthand_attributes_are_merged() {
    let harness = harness_for(|cx| {
        rsx! {
            <View direction="row">
                // `w` reaches `Chip`'s `style` prop, and `Chip` adds `p` to it.
                <Chip w={160.0} label="wide"/>
                <Chip label="plain"/>
                <Text>"tail"</Text>
            </View>
        }
        .show(cx);
    });

    let wide = harness.get_by_label("wide").rect();
    let plain = harness.get_by_label("plain").rect();
    let tail = harness.get_by_label("tail").rect();

    // The caller's `w={160.0}` survived: the next chip starts a full 160pt in.
    assert!(
        plain.left() - wide.left() > 140.0,
        "the first chip is 160pt wide: {wide:?} {plain:?}"
    );
    // ...and the callee's `p={10.0}` is applied to both of them.
    assert!(
        wide.left() > 15.0 && plain.left() - tail.left() < -15.0,
        "p={{10}} pads the text inside each chip: {wide:?} {plain:?} {tail:?}"
    );
}
