//! A `<Text>` inside a `<View>` can be selected with the mouse, like a
//! `egui::Label`.
//!
//! The layout engine paints a `<Text>` itself rather than putting a `Label` in
//! a `Ui`, so it has to run `LabelSelectionState` itself as well. The default
//! is the style's `interaction.selectable_labels`, and `selectable={false}`
//! turns it off, both as `Label::selectable` does.

mod common;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;
use egui_react_elements::prelude::*;

const LABEL: &str = "select me please";

/// A `<Text>` inside a `<View>`, so it is a taffy node and not a `Label`.
fn harness<'a>(selectable: Option<bool>) -> Harness<'a, Store> {
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            run_app(ui, store, move |cx| {
                // Two spellings of the same tree: `<Text>` on its own is the
                // default (follow the style), `selectable={..}` overrides it.
                match selectable {
                    None => rsx! {
                        <View p={20}>
                            <Text>{LABEL}</Text>
                        </View>
                    }
                    .show(cx),
                    Some(selectable) => rsx! {
                        <View p={20}>
                            <Text selectable={selectable}>{LABEL}</Text>
                        </View>
                    }
                    .show(cx),
                }
            });
        },
        Store::new(),
    );
    // The first frame creates the node, and a node created this frame has no
    // place on screen yet, so it paints deferred and is not selectable. From
    // the second frame on it is.
    harness.run();
    harness.run();
    harness
}

/// Is anything selected, anywhere?
fn has_selection(harness: &Harness<'_, Store>) -> bool {
    harness
        .ctx
        .plugin::<egui::text_selection::LabelSelectionState>()
        .lock()
        .has_selection()
}

/// Press at the left of the text and let go at its right.
fn drag_across(harness: &mut Harness<'_, Store>) {
    let rect = harness.get_by_label(LABEL).rect();
    let from = egui::pos2(rect.left() + 1.0, rect.center().y);
    let to = egui::pos2(rect.right() - 1.0, rect.center().y);

    harness.hover_at(from);
    harness.run();
    harness.drag_at(from);
    harness.run();
    // Twice: the first move is the one egui decides is a drag.
    harness.hover_at(to);
    harness.run();
    harness.hover_at(to);
    harness.run();
}

#[test]
fn dragging_across_a_text_selects_it() {
    let mut harness = harness(None);
    assert!(
        !has_selection(&harness),
        "nothing is selected to begin with"
    );

    drag_across(&mut harness);
    assert!(
        has_selection(&harness),
        "a `<Text>` follows `interaction.selectable_labels`, which is on by default"
    );

    let end = harness.get_by_label(LABEL).rect().right_center();
    harness.drop_at(end);
    harness.run();
    assert!(
        has_selection(&harness),
        "and the selection outlives the drag"
    );
}

#[test]
fn selectable_false_selects_nothing() {
    let mut harness = harness(Some(false));

    drag_across(&mut harness);
    assert!(
        !has_selection(&harness),
        "`selectable={{false}}` drops the selection, as `Label::selectable(false)` does"
    );
}
