//! Every attribute has a row, and the code column is the code that drew it.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;
use egui_react_app::{root_id, root_style};
use styles::App;

/// Tall enough that no row is scrolled out of the tree: a `ScrollArea`
/// culls what it does not draw, and a culled label cannot be found.
fn harness<'a>() -> Harness<'a, Store> {
    Harness::builder()
        .with_size(egui::vec2(600.0, 2400.0))
        .build_ui_state(
            |ui, store: &mut Store| {
                store.begin_pass(ui.ctx());
                {
                    let store: &Store = store;
                    let mut cx = Cx::new(store, ui, root_id());
                    let view = rsx! { <App/> };
                    cx.root_container(root_id(), root_style(), |cx| view.show(cx));
                }
                store.end_pass();
            },
            Store::new(),
        )
}

/// Every attribute `ItemStyle` has a setter for, layout and paint alike.
const ATTRIBUTES: &[&str] = &[
    "w",
    "h",
    "min_w",
    "min_h",
    "max_w",
    "max_h",
    "grow",
    "shrink",
    "basis",
    "align_self",
    "m",
    "mx",
    "my",
    "mt",
    "mr",
    "mb",
    "ml",
    "p",
    "px",
    "py",
    "pt",
    "pr",
    "pb",
    "pl",
    "col_span",
    "row_span",
    "bg",
    "border",
    "radius",
    "shadow",
    "custom_shadow",
    "opacity",
];

#[test]
fn every_attribute_has_a_row() {
    let mut harness = harness();
    harness.run();
    harness.run();
    for name in ATTRIBUTES {
        assert!(
            harness.query_by_label(name).is_some(),
            "no row for `{name}`"
        );
    }
}

#[test]
fn the_code_column_is_the_code_that_drew_the_row() {
    let mut harness = harness();
    harness.run();
    harness.run();
    assert!(
        harness
            .query_by_label("<Chip look={look} w={120.0}/>")
            .is_some(),
        "the `w` row's code"
    );
    assert!(
        harness
            .query_by_label("<Chip look={look} grow={1.0}/> <Chip look={look}/>")
            .is_some(),
        "the `grow` row's code, two elements"
    );
}
