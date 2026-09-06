//! Resizing the window lands on the layout the same window size renders from
//! scratch.
//!
//! This is a guard, not a regression test: it pins the *result* of a resize so
//! that changes to when `egui_taffy` computes its layout cannot quietly move
//! anything. A `grow` child is what makes the layout depend on the window: it
//! takes whatever space the rest leaves, so both the window width and the
//! window height show up in a rect a test can read.

mod common;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;
use egui_react_elements::prelude::*;

const SMALL: egui::Vec2 = egui::vec2(300.0, 300.0);
const LARGE: egui::Vec2 = egui::vec2(400.0, 350.0);

/// A column that fills the window: a row whose first item grows, then a
/// growing filler, then a label pushed to the bottom.
fn app(cx: &mut Cx<'_, '_>) {
    rsx! {
        <View direction="column" w="100%" h="100%">
            <View direction="row" w="100%">
                <Text grow={1.0}>"left"</Text>
                <Text>"right"</Text>
            </View>
            <View grow={1.0} w="100%">
                <Text>"middle"</Text>
            </View>
            <Text>"bottom"</Text>
        </View>
    }
    .show(cx);
}

fn harness(size: egui::Vec2) -> Harness<'static, Store> {
    let mut harness = Harness::builder().with_size(size).build_ui_state(
        |ui, store: &mut Store| run_app(ui, store, app),
        Store::new(),
    );
    harness.run();
    harness
}

/// The two rects that move with the window: `right` follows the width, `bottom`
/// follows the height.
fn probes(harness: &Harness<'static, Store>) -> (egui::Rect, egui::Rect) {
    (
        harness.get_by_label("right").rect(),
        harness.get_by_label("bottom").rect(),
    )
}

#[test]
fn a_resize_lands_on_the_layout_of_the_new_size() {
    let fresh_small = probes(&harness(SMALL));
    let fresh_large = probes(&harness(LARGE));
    assert_ne!(
        fresh_small, fresh_large,
        "the fixture has to react to the window size at all"
    );

    let mut harness = harness(SMALL);
    assert_eq!(probes(&harness), fresh_small);

    harness.set_size(LARGE);
    harness.run();
    assert_eq!(
        probes(&harness),
        fresh_large,
        "after growing the window the layout must be the one this size renders from scratch"
    );

    harness.set_size(SMALL);
    harness.run();
    assert_eq!(
        probes(&harness),
        fresh_small,
        "and the same going back down"
    );
}
