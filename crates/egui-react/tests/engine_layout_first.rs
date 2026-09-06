//! A frame where only the root rect resized is laid out before it draws.
//!
//! Both tests build the same row through `Cx`: a fixed width leaf, a leaf that
//! grows into the free space, and a leaf sized by its content. Each leaf
//! reports the size the test chose with `Ui::set_min_size`, so nothing here
//! depends on font metrics. The root takes the whole width of the rect it is
//! given, so widening that rect moves the last two leaves without touching any
//! style.
//!
//! Ported from the `egui_taffy` fork's `tests/layout_first.rs`.

use std::cell::{Cell, RefCell};
use std::num::NonZeroUsize;

use egui_react::prelude::*;
use egui_react::taffy;
use egui_react::taffy::prelude::*;

const ROW_HEIGHT: f32 = 24.0;
const FIXED_WIDTH: f32 = 40.0;
const TAIL_WIDTH: f32 = 30.0;

/// What one frame of the row looked like from egui's side.
struct FrameResult {
    /// How many passes egui ran for the frame.
    passes: usize,
    /// Did the tree itself ask for a discard in any of them?
    discarded: bool,
    /// Where each leaf was drawn in the first pass, relative to the root rect.
    first_pass: Vec<(&'static str, egui::Rect)>,
}

/// The style of the root container: a row as wide as the rect it is given.
///
/// Not a fixed length: the root has to follow the rect without its style
/// changing, or the style change alone would dirty the tree.
fn root_style() -> taffy::Style {
    taffy::Style {
        display: taffy::Display::Flex,
        flex_direction: taffy::FlexDirection::Row,
        size: Size {
            width: percent(1.0),
            height: length(ROW_HEIGHT),
        },
        ..Default::default()
    }
}

/// A leaf that reports `size` as its measurement and records where it drew.
fn leaf(
    cx: &mut Cx<'_, '_>,
    id: &'static str,
    style: ItemStyle,
    size: egui::Vec2,
    record: &mut impl FnMut(&'static str, egui::Rect),
) {
    cx.leaf(&style, |ui| {
        record(id, ui.max_rect());
        ui.set_min_size(size);
    });
}

/// Draw the row once, in a rect `root_width` wide, with `tail_width` as the
/// measurement the content sized leaf reports.
fn frame(ctx: &egui::Context, store: &mut Store, root_width: f32, tail_width: f32) -> FrameResult {
    let passes = Cell::new(0);
    let discarded = Cell::new(false);
    let first_pass = RefCell::new(Vec::new());

    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        passes.set(passes.get() + 1);
        let is_first_pass = passes.get() == 1;
        let before = ui.ctx().output(|o| o.requested_discard());

        let root_rect =
            egui::Rect::from_min_size(ui.max_rect().min, egui::vec2(root_width, ROW_HEIGHT));
        let mut record = |id: &'static str, rect: egui::Rect| {
            if is_first_pass {
                first_pass
                    .borrow_mut()
                    .push((id, rect.translate(-root_rect.min.to_vec2())));
            }
        };
        let mut root = ui.new_child(egui::UiBuilder::new().max_rect(root_rect));

        store.begin_pass(ui.ctx());
        {
            let store: &Store = store;
            let mut cx = Cx::new(store, &mut root, egui::Id::new("root"));
            cx.root_container(egui::Id::new("row"), root_style(), |cx| {
                leaf(
                    cx,
                    "fixed",
                    ItemStyle::default().w(FIXED_WIDTH),
                    egui::vec2(FIXED_WIDTH, ROW_HEIGHT),
                    &mut record,
                );
                leaf(
                    cx,
                    "grow",
                    // Without `min_w` the automatic minimum size of a flex item
                    // is its content, and the leaf would widen with its text
                    // instead of taking the free space.
                    ItemStyle::default().grow(1.0).basis(0.0).min_w(0.0),
                    egui::vec2(50.0, ROW_HEIGHT),
                    &mut record,
                );
                leaf(
                    cx,
                    "tail",
                    ItemStyle::default(),
                    egui::vec2(tail_width, ROW_HEIGHT),
                    &mut record,
                );
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
        first_pass: first_pass.into_inner(),
    }
}

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.options_mut(|o| o.max_passes = NonZeroUsize::new(3).unwrap());
    ctx
}

/// Where the three leaves belong in a row `root_width` wide.
fn expected(root_width: f32) -> Vec<(&'static str, egui::Rect)> {
    let at =
        |x: f32, w: f32| egui::Rect::from_min_size(egui::pos2(x, 0.0), egui::vec2(w, ROW_HEIGHT));
    vec![
        ("fixed", at(0.0, FIXED_WIDTH)),
        (
            "grow",
            at(FIXED_WIDTH, root_width - FIXED_WIDTH - TAIL_WIDTH),
        ),
        ("tail", at(root_width - TAIL_WIDTH, TAIL_WIDTH)),
    ]
}

#[test]
fn resized_root_draws_the_new_layout_in_the_first_pass() {
    let ctx = context();
    let mut store = Store::new();

    // First frame: every node is new, so it draws in a sizing pass and the
    // frame has to be redrawn.
    let first = frame(&ctx, &mut store, 300.0, TAIL_WIDTH);
    assert!(first.discarded, "a new tree has to be drawn twice");

    // Settle, so the next frame can only differ by its width.
    let second = frame(&ctx, &mut store, 300.0, TAIL_WIDTH);
    assert!(!second.discarded);
    assert_eq!(second.first_pass, expected(300.0));

    // The rect is wider now. Nothing else changed, so the layout is computed
    // before the children draw and they draw where they belong right away.
    let third = frame(&ctx, &mut store, 400.0, TAIL_WIDTH);
    assert_eq!(
        third.first_pass,
        expected(400.0),
        "the first pass must already use the new width"
    );
    assert!(
        !third.discarded,
        "drawing the right frame at once must not ask for a second pass"
    );
    assert_eq!(third.passes, 1);
}

#[test]
fn measurement_changing_after_the_early_layout_still_discards() {
    let ctx = context();
    let mut store = Store::new();

    let first = frame(&ctx, &mut store, 300.0, TAIL_WIDTH);
    assert!(first.discarded, "a new tree has to be drawn twice");
    let second = frame(&ctx, &mut store, 300.0, TAIL_WIDTH);
    assert!(!second.discarded);

    // Wider rect and a wider content sized leaf in the same frame. The early
    // layout only knows the old measurement, so the leaves draw at the wrong
    // place; the new measurement dirties the node and the recalculation after
    // drawing sees the difference.
    let third = frame(&ctx, &mut store, 400.0, 80.0);
    assert_eq!(
        third.first_pass,
        expected(400.0),
        "the early layout uses the width it has and last frame's measurement"
    );
    assert!(
        third.discarded,
        "a measurement that moves the layout still has to ask for a second pass"
    );
    assert_eq!(third.passes, 2);
}
