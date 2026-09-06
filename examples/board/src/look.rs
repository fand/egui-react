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
