//! The engine asks egui for a second pass only when the layout moved.
//!
//! Both tests build the same row through `Cx`: a fixed width leaf, a leaf that
//! grows into the free space, and a leaf sized by its content. Each leaf
//! reports the size the test chose with `Ui::set_min_size`, so nothing here
//! depends on font metrics.
//!
//! Ported from the `egui_taffy` fork's `tests/discard.rs`, which is where this
//! rule was first written.

use std::cell::Cell;
use std::num::NonZeroUsize;

use egui_reactor::prelude::*;
use egui_reactor::taffy;
use egui_reactor::taffy::prelude::*;

const ROOT_WIDTH: f32 = 300.0;
const ROW_HEIGHT: f32 = 24.0;
const FIXED_WIDTH: f32 = 40.0;

/// What one frame of the row looked like from egui's side.
struct FrameResult {
    /// How many passes egui ran for the frame.
    passes: usize,
    /// Did the tree itself ask for a discard in any of them?
    discarded: bool,
}

/// The style of the root container: a row as wide and as tall as the rect.
fn root_style() -> taffy::Style {
    taffy::Style {
        display: taffy::Display::Flex,
        flex_direction: taffy::FlexDirection::Row,
        size: Size {
            width: length(ROOT_WIDTH),
            height: length(ROW_HEIGHT),
        },
        ..Default::default()
    }
}

/// The three leaves, with `grow_width` and `tail_width` as what the last two
/// measure.
fn row(cx: &mut Cx<'_, '_>, grow_width: f32, tail_width: f32) {
    leaf(
        cx,
        ItemStyle::default().w(FIXED_WIDTH),
        egui::vec2(FIXED_WIDTH, ROW_HEIGHT),
    );
    leaf(
        cx,
        // Without `min_w` the automatic minimum size of a flex item is its
        // content, and the leaf would widen with its text instead of taking
        // the free space.
        ItemStyle::default().grow(1.0).basis(0.0).min_w(0.0),
        egui::vec2(grow_width, ROW_HEIGHT),
    );
    leaf(cx, ItemStyle::default(), egui::vec2(tail_width, ROW_HEIGHT));
}

/// A leaf that reports `size` as its measurement and draws nothing.
fn leaf(cx: &mut Cx<'_, '_>, style: ItemStyle, size: egui::Vec2) {
    cx.leaf(&style, |ui| ui.set_min_size(size));
}

/// Draw the row once, with `root_height` as the height of the rect the tree is
/// given and the two measured widths as the leaf sizes.
fn frame(
    ctx: &egui::Context,
    store: &mut Store,
    root_height: f32,
    grow: f32,
    tail: f32,
) -> FrameResult {
    let passes = Cell::new(0);
    let discarded = Cell::new(false);

    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        passes.set(passes.get() + 1);
        let before = ui.ctx().output(|o| o.requested_discard());

        // A `Ui` of exactly the rect the tree should get: `root_container`
        // reserves all of it, which is the runner's own root.
        let root_rect =
            egui::Rect::from_min_size(ui.max_rect().min, egui::vec2(ROOT_WIDTH, root_height));
        let mut root = ui.new_child(egui::UiBuilder::new().max_rect(root_rect));

        store.begin_pass(ui.ctx());
        {
            let store: &Store = store;
            let mut cx = Cx::new(store, &mut root, egui::Id::new("root"));
            cx.root_container(egui::Id::new("row"), root_style(), |cx| {
                row(cx, grow, tail);
            });
        }
        store.end_pass();

        let after = ui.ctx().output(|o| o.requested_discard());
        discarded.set(discarded.get() || (!before && after));
    });
    output.drop_without_applying_deltas();

    FrameResult {
        passes: passes.get(),
        discarded: discarded.get(),
    }
}

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.options_mut(|o| o.max_passes = NonZeroUsize::new(3).unwrap());
    ctx
}

#[test]
fn unchanged_layout_does_not_discard() {
    let ctx = context();
    let mut store = Store::new();

    // First frame: every node is new, so it draws in a sizing pass and the
    // frame has to be redrawn.
    let first = frame(&ctx, &mut store, ROW_HEIGHT, 50.0, 30.0);
    assert!(first.discarded, "a new tree has to be drawn twice");

    // Second frame: the rect the tree is given is taller, which is what a row
    // inside a scroll area sees on every scrolled frame, and the growing leaf
    // measures wider. Neither moves anything: the root height comes from the
    // style and the growing leaf takes the free space either way.
    let second = frame(&ctx, &mut store, ROW_HEIGHT + 20.0, 60.0, 30.0);
    assert!(
        !second.discarded,
        "recalculating to the same layout must not ask for a second pass"
    );
    assert_eq!(second.passes, 1);
}

#[test]
fn moved_layout_discards() {
    let ctx = context();
    let mut store = Store::new();

    let first = frame(&ctx, &mut store, ROW_HEIGHT, 50.0, 30.0);
    assert!(first.discarded, "a new tree has to be drawn twice");

    // Settle, so that the discard below can only come from the new measurement.
    let second = frame(&ctx, &mut store, ROW_HEIGHT, 50.0, 30.0);
    assert!(!second.discarded);

    // The content sized leaf is wider, so it moves left and the growing leaf
    // shrinks. Both were drawn at their old place, so the frame is stale.
    let third = frame(&ctx, &mut store, ROW_HEIGHT, 50.0, 80.0);
    assert!(
        third.discarded,
        "a layout change has to ask for a second pass"
    );
    assert_eq!(third.passes, 2);
}
