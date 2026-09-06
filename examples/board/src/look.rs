//! The colours, and the one thing both versions paint by hand.
//!
//! Shared like `board.rs` is, for the same reason: what the two versions of
//! this example are being compared on is where they keep their state, so
//! everything that is merely *drawing* belongs to both of them. `board.rs`
//! knows nothing about egui and this is where the parts that do live.

use crate::board::Label;

/// Provided to the whole tree; every level that draws something reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub dark: bool,
}

impl Theme {
    pub const DARK: Self = Self { dark: true };

    /// Headings and the insertion line.
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

    /// The colour of a card's tag, and of the toolbar chip that filters by it.
    pub fn label(self, label: Label) -> egui::Color32 {
        match label {
            Label::None => egui::Color32::from_gray(if self.dark { 0x55 } else { 0xbb }),
            Label::Red => egui::Color32::from_rgb(0xc0, 0x50, 0x50),
            Label::Yellow => egui::Color32::from_rgb(0xc0, 0xa0, 0x40),
            Label::Green => egui::Color32::from_rgb(0x50, 0xa0, 0x60),
            Label::Blue => egui::Color32::from_rgb(0x50, 0x80, 0xc0),
        }
    }
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
