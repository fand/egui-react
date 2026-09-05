//! Each hatch does what it says: the closure keeps state, the leaf follows the
//! slider, the painter counts its samples, and the nested `Cx` has hooks of its
//! own.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use escape_hatch::App;
use react_egui::prelude::*;
use react_egui_app::{root_id, root_style};

fn harness<'a>() -> Harness<'a, Store> {
    Harness::builder()
        .with_size(egui::vec2(420.0, 620.0))
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

/// The painter leaf: its samples come from state the buttons write.
#[test]
fn the_painter_counts_its_samples() {
    let mut harness = harness();
    harness.run();
    assert!(harness.query_by_label("0 samples").is_some());

    for expected in ["1 samples", "2 samples"] {
        harness.get_by_label("add sample").click();
        // One pass to apply the write, one to draw the new count.
        harness.run();
        harness.run();
        assert!(harness.query_by_label(expected).is_some());
    }

    harness.get_by_label("clear").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("0 samples").is_some());
}

/// The unwrapped `egui::ProgressBar` in a leaf follows the wrapped `<Slider>`.
#[test]
fn the_slider_drives_the_leaf() {
    let mut harness = harness();
    harness.run();
    assert!(harness.query_by_label("progress 0.35").is_some());

    // Dragging is fiddly headless; focusing and nudging is the stable way.
    harness.get_by_role(egui::accesskit::Role::Slider).focus();
    harness.run();
    harness.key_press(egui::Key::ArrowRight);
    harness.run();
    harness.run();
    assert!(
        harness.query_by_label("progress 0.35").is_none(),
        "the leaf did not follow the slider",
    );
}

/// A hook called inside a `view` closure keeps its state like any other.
#[test]
fn state_inside_a_view_closure_survives_frames() {
    let mut harness = harness();
    harness.run();
    assert!(harness.query_by_label("opened 0 times").is_some());

    harness.get_by_label("open the docs").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("opened 1 times").is_some());

    // Still there several frames later.
    harness.run();
    harness.run();
    assert!(harness.query_by_label("opened 1 times").is_some());
}

/// The `Cx` built inside `ui.group` has its own hook scope.
#[test]
fn the_nested_cx_keeps_its_own_state() {
    let mut harness = harness();
    harness.run();
    assert!(harness.query_by_label("outer: 0").is_some());
    assert!(harness.query_by_label("inner: 0").is_some());

    harness.get_by_label("inner +").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("inner: 1").is_some());
    assert!(harness.query_by_label("outer: 0").is_some());

    harness.get_by_label("outer +").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("outer: 1").is_some());
    assert!(harness.query_by_label("inner: 1").is_some());
}
