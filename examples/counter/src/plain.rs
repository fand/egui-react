//! The same counter written with egui alone, for the side-by-side comparison.
//!
//! At this size the two are close: one `i32`, three buttons. The difference is
//! that `<View align="center" justify="center">` is two attributes, while here
//! the block is measured and its leading space allocated by hand, because egui
//! lays out from the top left and has no notion of free space.

/// Everything the plain version keeps between frames.
#[derive(Default)]
pub struct PlainState {
    pub count: i32,
}

pub fn ui(ui: &mut egui::Ui, state: &mut PlainState) {
    // Measure first, place second: centring needs the block's height, and
    // centring the button row needs its width.
    let labels = ["-", "reset", "+"];
    let (block_height, row_width) = {
        let measure = |text: &str, font: egui::FontId| {
            ui.painter()
                .layout_no_wrap(text.to_owned(), font, egui::Color32::PLACEHOLDER)
                .size()
        };
        let button_font = egui::TextStyle::Button.resolve(ui.style());
        let number_height = measure("0", egui::FontId::proportional(32.0)).y;
        let row_width: f32 = labels
            .iter()
            .map(|label| {
                measure(label, button_font.clone()).x + 2.0 * ui.spacing().button_padding.x
            })
            .sum::<f32>()
            + ui.spacing().item_spacing.x * (labels.len() - 1) as f32;
        (
            number_height + 12.0 + ui.spacing().interact_size.y,
            row_width,
        )
    };

    // The gaps are all explicit, so egui's automatic vertical spacing would
    // only be added on top of them.
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.add_space(((ui.available_height() - block_height) / 2.0).max(0.0));

    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(state.count.to_string())
                .size(32.0)
                .strong(),
        );
        ui.add_space(12.0);
        // An exactly sized row, because a `horizontal` inside a centred column
        // still starts at the left edge.
        ui.allocate_ui_with_layout(
            egui::vec2(row_width, ui.spacing().interact_size.y),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                if ui.button("-").clicked() {
                    state.count -= 1;
                }
                if ui.button("reset").clicked() {
                    state.count = 0;
                }
                if ui.button("+").clicked() {
                    state.count += 1;
                }
            },
        );
    });
}
