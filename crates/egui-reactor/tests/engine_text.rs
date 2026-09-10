//! A tree of containers and text alone is right on its very first frame.
//!
//! Every other leaf has to draw before it knows its size, so a new one draws
//! invisible in a sizing pass and the frame is thrown away. A galley needs no
//! `Ui`: the layout engine lays it out while taffy asks for the node's size,
//! and paints it after the layout is final. So a new `<View>` / `<Text>` tree
//! costs one pass, and the text lands where it belongs in that pass.

use std::cell::Cell;
use std::num::NonZeroUsize;

use egui_reactor::prelude::*;
use egui_reactor::taffy;
use egui_reactor::taffy::prelude::*;

const ROOT_WIDTH: f32 = 300.0;
const ROOT_HEIGHT: f32 = 60.0;
/// Padding on the root, so that "where the text belongs" is not the corner a
/// fresh taffy node happens to sit in.
const PAD: f32 = 12.0;

/// The style of the root container: a padded column filling the rect.
fn root_style() -> taffy::Style {
    taffy::Style {
        display: taffy::Display::Flex,
        flex_direction: taffy::FlexDirection::Column,
        size: Size {
            width: length(ROOT_WIDTH),
            height: length(ROOT_HEIGHT),
        },
        padding: taffy::Rect {
            left: length(PAD),
            right: length(PAD),
            top: length(PAD),
            bottom: length(PAD),
        },
        ..Default::default()
    }
}

/// What one frame looked like from egui's side.
struct FrameResult {
    /// How many passes egui ran for the frame.
    passes: usize,
    /// Where the first text shape of the last pass was painted.
    text_pos: Option<egui::Pos2>,
    /// The top left corner of the rect the tree was given.
    root_min: egui::Pos2,
}

/// Draw one frame of a padded column holding one text, and a widget leaf
/// beside it when `with_leaf` is set.
fn frame(ctx: &egui::Context, store: &mut Store, with_leaf: bool) -> FrameResult {
    let passes = Cell::new(0);
    let root_min = Cell::new(egui::Pos2::ZERO);

    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        passes.set(passes.get() + 1);

        let root_rect =
            egui::Rect::from_min_size(ui.max_rect().min, egui::vec2(ROOT_WIDTH, ROOT_HEIGHT));
        root_min.set(root_rect.min);
        let mut root = ui.new_child(egui::UiBuilder::new().max_rect(root_rect));

        store.begin_pass(ui.ctx());
        {
            let store: &Store = store;
            let mut cx = Cx::new(store, &mut root, egui::Id::new("root"));
            cx.root_container(egui::Id::new("column"), root_style(), |cx| {
                cx.text(&ItemStyle::default(), "hello".into(), false, None);
                if with_leaf {
                    cx.leaf(&ItemStyle::default(), |ui| {
                        ui.label("beside");
                    });
                }
            });
        }
        store.end_pass();
    });

    let text_pos = output
        .shapes
        .iter()
        .find_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) => Some(text.pos),
            _ => None,
        });
    output.drop_without_applying_deltas();

    FrameResult {
        passes: passes.get(),
        text_pos,
        root_min: root_min.get(),
    }
}

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.options_mut(|o| o.max_passes = NonZeroUsize::new(3).unwrap());
    ctx
}

#[test]
fn a_new_text_tree_settles_in_one_pass() {
    let ctx = context();
    let mut store = Store::new();

    let first = frame(&ctx, &mut store, false);
    assert_eq!(
        first.passes, 1,
        "a new tree of containers and text needs no second pass"
    );
    assert_eq!(
        first.text_pos,
        Some(first.root_min + egui::vec2(PAD, PAD)),
        "and the text is painted inside the padding in that one pass"
    );

    // Nothing changed, so the second frame is one pass as well.
    let second = frame(&ctx, &mut store, false);
    assert_eq!(second.passes, 1);
    assert_eq!(second.text_pos, first.text_pos);
}

#[test]
fn a_new_widget_leaf_still_needs_a_second_pass() {
    let ctx = context();
    let mut store = Store::new();

    let first = frame(&ctx, &mut store, true);
    assert!(
        first.passes > 1,
        "a leaf that has to draw to be measured still costs a pass, got {}",
        first.passes
    );

    let second = frame(&ctx, &mut store, true);
    assert_eq!(second.passes, 1, "and only on the frame that added it");
}
