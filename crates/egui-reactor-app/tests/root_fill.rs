//! The root container fills the window.
//!
//! `reserve_available_space` hands the layout engine the window's size as available
//! space but leaves the root node's own size at `auto`, so without the 100%
//! minimums in `root_style` taffy sizes that node by its content. `grow` then
//! has no free space to claim and `justify="center"` has nothing to centre in,
//! and an app written as `<View grow={1.0} justify="center">` draws in the
//! top-left corner instead of the middle.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::prelude::*;
use egui_reactor_app::{root_id, root_style};
use egui_reactor_elements::prelude::*;

/// `examples/counter`'s frame, without the state.
#[component]
fn App(cx: &mut Cx) {
    rsx! {
        <View direction="column" align="center" justify="center" gap={12} grow={1.0}>
            <Text>"middle"</Text>
        </View>
    }
}

/// The runner's frame, minus eframe: one pass inside the root container.
fn run_app(ui: &mut egui::Ui, store: &mut Store) {
    store.begin_pass(ui.ctx());
    {
        let store: &Store = store;
        let mut cx = Cx::new(store, ui, root_id());
        let view = rsx! { <App/> };
        cx.root_container(root_id(), root_style(), |cx| view.show(cx));
    }
    store.end_pass();
}

#[test]
fn a_centred_child_lands_in_the_middle_of_the_window() {
    let size = egui::vec2(400.0, 300.0);
    let mut harness = Harness::builder()
        .with_size(size)
        .build_ui_state(run_app, Store::new());
    harness.run();

    let centre = harness.get_by_label("middle").rect().center();
    let want = (size / 2.0).to_pos2();
    // Generous: the harness insets the app by an 8px margin, and the text's own
    // box is a few points off centre. Top-left would be off by ~180x140.
    assert!(
        (centre - want).abs().max_elem() < 24.0,
        "expected the label near {want:?}, found it at {centre:?}",
    );
}
