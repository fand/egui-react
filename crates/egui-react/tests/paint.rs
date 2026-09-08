//! Plan 2: the taffy path paints what `PaintStyle` says.
//!
//! A node's box is not painted where it is declared — its rect is only known
//! once taffy has solved the frame — so the engine claims a shape slot in draw
//! order and fills it afterwards. These tests check both halves: that the
//! padding and the border of a painted node reach the layout, and that the
//! shapes really land in the frame's output, at the node's border box.
//!
//! Core APIs only (`cx.container`, `cx.leaf`, `cx.text`), as `hidden.rs` does,
//! and over `Context::run_ui` rather than a kittest harness, because what is
//! under test here is the list of shapes egui came out of the frame with.

use std::cell::Cell;

use egui_react::prelude::*;

/// The padding of the painted container.
const PAD: f32 = 8.0;
/// The gap between the row's items.
const GAP: f32 = 6.0;
/// The width of the painted container's border.
const BORDER: f32 = 2.0;

const TEXT_BG: egui::Color32 = egui::Color32::GREEN;

/// What the app should draw this frame.
#[derive(Clone, Copy)]
struct Opts {
    /// The painted container's background.
    bg: egui::Color32,
    /// Give it a border too?
    border: bool,
    /// Hide it with `display="none"`?
    hidden: bool,
    /// Multiply its opacity, and its children's?
    opacity: Option<f32>,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            bg: egui::Color32::RED,
            border: false,
            hidden: false,
            opacity: None,
        }
    }
}

/// The rects the leaves were given, read back out of the frame.
struct Rects {
    first: Cell<egui::Rect>,
    inside: Cell<egui::Rect>,
    last: Cell<egui::Rect>,
    text: Cell<egui::Rect>,
}

impl Default for Rects {
    fn default() -> Self {
        let nothing = || Cell::new(egui::Rect::NOTHING);
        Self {
            first: nothing(),
            inside: nothing(),
            last: nothing(),
            text: nothing(),
        }
    }
}

/// A row of: a plain leaf, a painted container holding a leaf, a plain leaf,
/// and a `<Text>` with a background of its own.
///
/// The two plain leaves are the rulers: everything the painted container
/// reserves shows up as the distance between them and the leaf inside it.
fn app(cx: &mut Cx<'_, '_>, opts: Opts, rects: &Rects) {
    let row = ContainerStyle::default()
        .direction("row")
        .align("start")
        .gap(GAP);
    cx.container(egui::Id::new("row"), &row, &ItemStyle::default(), |cx| {
        cx.scope("first", |cx| {
            rects.first.set(leaf(cx, "first"));
        });

        cx.scope("painted", |cx| {
            let id = cx.layout_id();
            let inner = ContainerStyle::default()
                .direction("column")
                .display(if opts.hidden { "none" } else { "flex" });
            let mut item = ItemStyle::default().p(PAD).bg(opts.bg);
            if opts.border {
                item = item.border(egui::Stroke::new(BORDER, egui::Color32::BLACK));
            }
            if let Some(opacity) = opts.opacity {
                item = item.opacity(opacity);
            }
            cx.container(id, &inner, &item, |cx| {
                rects.inside.set(leaf(cx, "inside"));
            });
        });

        cx.scope("last", |cx| {
            rects.last.set(leaf(cx, "last"));
        });

        cx.scope("text", |cx| {
            let style = ItemStyle::default().bg(TEXT_BG);
            let response = cx.text(&style, "text node".into(), false, Some(false));
            rects.text.set(response.rect);
        });
    });
}

/// One leaf holding a label, returning the rect the layout gave it.
fn leaf(cx: &mut Cx<'_, '_>, label: &str) -> egui::Rect {
    cx.leaf(&ItemStyle::default(), |ui| {
        ui.label(label);
        ui.max_rect()
    })
}

/// What one frame came out with.
struct Frame {
    /// Every shape of the frame, with the `Shape::Vec`s flattened.
    shapes: Vec<egui::Shape>,
    /// How many passes egui ran for it.
    passes: usize,
}

impl Frame {
    /// The filled rects of this frame in `color`.
    fn filled(&self, color: egui::Color32) -> Vec<egui::Rect> {
        self.shapes
            .iter()
            .filter_map(|shape| match shape {
                egui::Shape::Rect(rect) if rect.fill == color => Some(rect.rect),
                _ => None,
            })
            .collect()
    }

    /// The stroked rects of this frame, with their stroke.
    fn stroked(&self) -> Vec<(egui::Rect, egui::Stroke)> {
        self.shapes
            .iter()
            .filter_map(|shape| match shape {
                egui::Shape::Rect(rect) if rect.stroke.width > 0.0 => {
                    Some((rect.rect, rect.stroke))
                }
                _ => None,
            })
            .collect()
    }
}

/// Draw the app once and collect the shapes egui was left with.
fn frame(ctx: &egui::Context, store: &mut Store, opts: Opts, rects: &Rects) -> Frame {
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        store.begin_pass(ui.ctx());
        {
            let store: &Store = store;
            let mut cx = Cx::new(store, ui, egui::Id::new("root"));
            app(&mut cx, opts, rects);
        }
        store.end_pass();
    });

    let mut shapes = Vec::new();
    for clipped in std::mem::take(&mut output.shapes) {
        flatten(clipped.shape, &mut shapes);
    }
    let passes = output.platform_output.num_completed_passes;
    output.drop_without_applying_deltas();

    Frame { shapes, passes }
}

/// Unpack the `Shape::Vec`s, which is how a node's shadow and background are
/// set into their one slot.
fn flatten(shape: egui::Shape, out: &mut Vec<egui::Shape>) {
    match shape {
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                flatten(shape, out);
            }
        }
        shape => out.push(shape),
    }
}

/// Draw enough frames for the layout to settle, and return the last one.
fn settled(ctx: &egui::Context, store: &mut Store, opts: Opts, rects: &Rects) -> Frame {
    let mut last = frame(ctx, store, opts, rects);
    for _ in 0..2 {
        last = frame(ctx, store, opts, rects);
    }
    last
}

/// `a` and `b` are the same point, up to what rounding a rect can do.
#[track_caller]
fn close(a: f32, b: f32, what: &str) {
    assert!((a - b).abs() < 0.5, "{what}: {a} != {b}");
}

#[test]
fn padding_and_border_of_a_painted_node_are_reserved() {
    let ctx = egui::Context::default();
    let mut store = Store::new();
    let rects = Rects::default();

    // Padding only: the leaf inside sits `PAD` in from the container's box on
    // every side, and the box takes that much room in the row.
    settled(&ctx, &mut store, Opts::default(), &rects);
    let (first, inside, last) = (rects.first.get(), rects.inside.get(), rects.last.get());
    close(inside.left() - first.right(), GAP + PAD, "left of the box");
    close(last.left() - inside.right(), PAD + GAP, "right of the box");
    close(inside.top() - first.top(), PAD, "top of the box");

    // And a border of 2 is reserved on top of the padding.
    let opts = Opts {
        border: true,
        ..Opts::default()
    };
    settled(&ctx, &mut store, opts, &rects);
    let (first, inside, last) = (rects.first.get(), rects.inside.get(), rects.last.get());
    close(
        inside.left() - first.right(),
        GAP + PAD + BORDER,
        "left of the bordered box",
    );
    close(
        last.left() - inside.right(),
        PAD + BORDER + GAP,
        "right of the bordered box",
    );
    close(
        inside.top() - first.top(),
        PAD + BORDER,
        "top of the bordered box",
    );
}

#[test]
fn the_background_and_the_border_reach_the_frames_shapes() {
    let ctx = egui::Context::default();
    let mut store = Store::new();
    let rects = Rects::default();

    let opts = Opts {
        border: true,
        ..Opts::default()
    };
    let frame = settled(&ctx, &mut store, opts, &rects);
    let inside = rects.inside.get();

    let backgrounds = frame.filled(egui::Color32::RED);
    assert_eq!(backgrounds.len(), 1, "one background: {backgrounds:?}");
    let bg = backgrounds[0];
    assert!(
        bg.contains_rect(inside.expand(PAD + BORDER - 0.5)),
        "the background covers the padding and the border: {bg:?} around {inside:?}",
    );

    let borders = frame.stroked();
    let border = borders
        .iter()
        .find(|(rect, _)| *rect == bg)
        .unwrap_or_else(|| panic!("no border on the box {bg:?}: {borders:?}"));
    assert_eq!(border.1.width, BORDER);
    assert_eq!(border.1.color, egui::Color32::BLACK);
}

#[test]
fn a_text_node_paints_its_own_background() {
    let ctx = egui::Context::default();
    let mut store = Store::new();
    let rects = Rects::default();

    let frame = settled(&ctx, &mut store, Opts::default(), &rects);
    let text = rects.text.get();

    let painted = frame.filled(TEXT_BG);
    assert_eq!(painted.len(), 1, "one text background: {painted:?}");
    assert!(
        painted[0].expand(0.5).contains_rect(text),
        "the text's background covers it: {:?} around {text:?}",
        painted[0],
    );
}

#[test]
fn a_hidden_node_paints_nothing() {
    let ctx = egui::Context::default();
    let mut store = Store::new();
    let rects = Rects::default();

    let opts = Opts {
        border: true,
        hidden: true,
        ..Opts::default()
    };
    let frame = settled(&ctx, &mut store, opts, &rects);
    assert!(
        frame.filled(egui::Color32::RED).is_empty(),
        "a `display=\"none\"` node claims no slot",
    );
    assert!(
        !frame.filled(TEXT_BG).is_empty(),
        "its visible sibling still paints",
    );
}

#[test]
fn a_background_that_changes_costs_no_second_pass() {
    let ctx = egui::Context::default();
    let mut store = Store::new();
    let rects = Rects::default();

    let colors = [
        egui::Color32::RED,
        egui::Color32::BLUE,
        egui::Color32::RED,
        egui::Color32::BLUE,
    ];
    let mut passes = Vec::new();
    for bg in colors {
        let opts = Opts {
            bg,
            ..Opts::default()
        };
        let frame = frame(&ctx, &mut store, opts, &rects);
        passes.push(frame.passes);
        // The colour of the frame really did change with the prop.
        assert_eq!(frame.filled(bg).len(), 1, "the background is {bg:?}");
    }

    // The first frames build the tree and measure; from there on a new colour
    // is just another shape in a slot that is claimed anyway.
    assert_eq!(&passes[2..], &[1, 1], "passes per frame: {passes:?}");
}

/// Plan 1.5: `Painter::set` applies the painter's opacity as the shape is set,
/// and by then the tree's `Ui` is back at the opacity it had outside the node.
/// So the factor is recorded where the slot is claimed and put back on a
/// painter of the engine's own.
#[test]
fn opacity_reaches_a_deferred_box() {
    let ctx = egui::Context::default();
    let mut store = Store::new();
    let rects = Rects::default();

    let opts = Opts {
        opacity: Some(0.5),
        ..Opts::default()
    };
    let frame = settled(&ctx, &mut store, opts, &rects);

    assert!(
        frame.filled(egui::Color32::RED).is_empty(),
        "the background is not painted at full opacity",
    );
    let faded = frame.filled(egui::Color32::RED.gamma_multiply(0.5));
    assert_eq!(faded.len(), 1, "one half-transparent background: {faded:?}");

    // The `<Text>` beside it is outside the faded node and keeps its own.
    assert_eq!(frame.filled(TEXT_BG).len(), 1, "the sibling is untouched");
}
