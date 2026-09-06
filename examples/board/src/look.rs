//! The colours, and the one thing both versions paint by hand.
//!
//! Shared like `board.rs` is, for the same reason: what the two versions of
//! this example are being compared on is where they keep their state, so
//! everything that is merely *drawing* belongs to both of them. `board.rs`
//! knows nothing about egui and this is where the parts that do live.

/// The height of the gap a dragged card would drop into: one card's row plus
/// the frame's margins, so the cards below move by exactly what the card in
/// hand will take up.
pub const PLACEHOLDER_H: f32 = 30.0;

/// How long a gap takes to open or close. Long enough to read as movement,
/// short enough that the card in hand never waits for it.
pub const GAP_TIME: f32 = 0.12;

/// How open the *picture* of a gap is, from `0.0` to `1.0`, on its way to
/// `open`. The layout itself opens and shuts at once; both versions draw the
/// cards below a gap shifted by the difference, so that they slide.
///
/// egui's animation runs on the gap's id, so a gap that is always in the
/// tree (closed at `0.0`) slides open from where it is and slides shut again
/// when the pointer moves on. With nothing in hand the answer is immediate:
/// the frame a card is dropped, the gap it fills is gone and the card is
/// there, and a gap that was still closing under it would show two cards.
pub fn gap_amount(ctx: &egui::Context, id: egui::Id, open: bool, carrying: bool) -> f32 {
    let time = if carrying { GAP_TIME } else { 0.0 };
    ctx.animate_bool_with_time_and_easing(id, open, time, egui::emath::easing::quadratic_out)
}

/// The transform that draws something `lift` points below where it was laid
/// out, for `Ui::with_visual_transform`. Zero is the identity, and the common
/// case.
pub fn lifted(lift: f32) -> egui::emath::TSTransform {
    egui::emath::TSTransform::from_translation(egui::vec2(0.0, lift))
}

/// Provided to the whole tree; every level that draws something reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub dark: bool,
}

impl Theme {
    pub const DARK: Self = Self { dark: true };

    /// Headings, the filter chips, and the outline of the gap a card would
    /// drop into.
    pub fn accent(self) -> egui::Color32 {
        if self.dark {
            egui::Color32::from_rgb(0x7f, 0xb2, 0xf0)
        } else {
            egui::Color32::from_rgb(0x1c, 0x54, 0x9c)
        }
    }

    /// A card's background.
    pub fn card(self) -> egui::Color32 {
        if self.dark {
            egui::Color32::from_rgb(0x2b, 0x2b, 0x2b)
        } else {
            egui::Color32::from_rgb(0xe8, 0xe8, 0xe8)
        }
    }

    pub fn text(self) -> egui::Color32 {
        if self.dark {
            egui::Color32::from_gray(0xdc)
        } else {
            egui::Color32::from_gray(0x1c)
        }
    }
}

/// Draw the gap a dragged card would drop into: an empty card, dashed, where
/// the card in hand would land.
///
/// A line between two cards says where the card goes; a gap the size of the
/// card says what the column will look like once it is there, which is the
/// question someone with a card in hand is actually asking.
pub fn placeholder(painter: &egui::Painter, rect: egui::Rect, theme: Theme) {
    painter.rect_filled(rect, 4u8, theme.card().gamma_multiply(0.4));
    let rect = rect.shrink(0.5);
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    let stroke = egui::Stroke::new(1.0, theme.accent());
    painter.extend(egui::Shape::dashed_line(&corners, stroke, 4.0, 4.0));
}

/// Draw the card that is in hand at `at`, on the layer above everything.
pub fn ghost(ctx: &egui::Context, at: egui::Pos2, title: &str, theme: Theme) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("board/ghost"),
    ));
    let font = egui::TextStyle::Body.resolve(&ctx.style_of(ctx.theme()));
    let galley = painter.layout_no_wrap(title.to_owned(), font, theme.text());
    let padding = egui::vec2(6.0, 4.0);
    let rect =
        egui::Rect::from_min_size(at + egui::vec2(10.0, 10.0), galley.size() + padding * 2.0);
    painter.rect_filled(rect, 4.0, theme.card());
    painter.galley(rect.min + padding, galley, theme.text());
}
