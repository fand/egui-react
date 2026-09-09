//! The same counter written with egui alone, for the side-by-side comparison.

#[derive(Default)]
pub struct PlainState {
    pub count: i32,
}

pub fn ui(ui: &mut egui::Ui, state: &mut PlainState) {
    let spacing = ui.spacing().clone();
    let font = egui::TextStyle::Button.resolve(ui.style());
    let row_width = ["-", "reset", "+"]
        .iter()
        .map(|label| {
            ui.fonts_mut(|f| {
                f.layout_no_wrap(label.to_string(), font.clone(), egui::Color32::PLACEHOLDER)
            })
            .size()
            .x + 2.0 * spacing.button_padding.x
        })
        .sum::<f32>()
        + 2.0 * spacing.item_spacing.x;
    let number_height = ui.fonts_mut(|f| f.row_height(&egui::FontId::proportional(32.0)));
    let block_height = number_height + 12.0 + spacing.interact_size.y;

    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 12.0;
        ui.add_space((ui.available_height() - block_height) / 2.0);
        ui.label(
            egui::RichText::new(state.count.to_string())
                .size(32.0)
                .strong(),
        );
        ui.allocate_ui_with_layout(
            egui::vec2(row_width, spacing.interact_size.y),
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
