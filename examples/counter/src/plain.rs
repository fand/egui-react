//! The same counter written with egui alone, for the side-by-side comparison.

#[derive(Default)]
pub struct PlainState {
    pub count: i32,
}

pub fn ui(ui: &mut egui::Ui, state: &mut PlainState) {
    ui.label(
        egui::RichText::new(state.count.to_string())
            .size(32.0)
            .strong(),
    );
    ui.horizontal(|ui| {
        if ui.button("-").clicked() {
            state.count -= 1;
        }
        if ui.button("reset").clicked() {
            state.count = 0;
        }
        if ui.button("+").clicked() {
            state.count += 1;
        }
    });
}
