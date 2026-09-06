//! The same counter written with egui alone, for the gallery's side by side.
//!
//! At this size the two are close: one `i32`, three buttons. The difference is
//! that `<View align="center" justify="center">` is two attributes, while here
//! the block has to be measured and its leading space allocated by hand,
//! because egui lays out from the top left and has no notion of free space.

/// Everything the plain version keeps between frames.
///
/// The egui-react version has no such type: `use_state(cx, || 0i32)` is the
/// whole of it, and the value lives in the store under the component's scope.
#[derive(Default)]
pub struct PlainState {
    pub count: i32,
}

/// The gap between the number and the row of buttons.
const GAP: f32 = 12.0;
/// The font size of the number.
const NUMBER: f32 = 32.0;

pub fn ui(ui: &mut egui::Ui, state: &mut PlainState) {
    let labels = ["-", "reset", "+"];

    // Measure first, place second. `justify="center"` needs to know how tall
    // the block is, and centring the row needs to know how wide it is.
    let row_height = ui.spacing().interact_size.y;
    let block_height = line_height(ui, NUMBER) + GAP + row_height;
    let row_width: f32 = labels
        .iter()
        .map(|label| button_width(ui, label))
        .sum::<f32>()
        + ui.spacing().item_spacing.x * (labels.len() - 1) as f32;

    // The gaps below are all explicit, so egui's automatic vertical spacing
    // would only be added on top of them.
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.add_space(((ui.available_height() - block_height) / 2.0).max(0.0));

    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(state.count.to_string())
                .size(NUMBER)
                .strong(),
        );
        ui.add_space(GAP);
        // A `horizontal` inside a centred column still starts at the left
        // edge, so the row goes into a `Ui` exactly as wide as it needs and
        // the column centres that instead.
        ui.allocate_ui_with_layout(
            egui::vec2(row_width, row_height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                if ui.button(labels[0]).clicked() {
                    state.count -= 1;
                }
                if ui.button(labels[1]).clicked() {
                    state.count = 0;
                }
                if ui.button(labels[2]).clicked() {
                    state.count += 1;
                }
            },
        );
    });
}

/// The height of one line of proportional text at `size`.
fn line_height(ui: &egui::Ui, size: f32) -> f32 {
    measure(ui, "0", egui::FontId::proportional(size)).y
}

/// The width a default button showing `text` will take.
fn button_width(ui: &egui::Ui, text: &str) -> f32 {
    let font = egui::TextStyle::Button.resolve(ui.style());
    measure(ui, text, font).x + 2.0 * ui.spacing().button_padding.x
}

/// The size `text` will take when drawn in `font`.
fn measure(ui: &egui::Ui, text: &str, font: egui::FontId) -> egui::Vec2 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, egui::Color32::PLACEHOLDER)
        .size()
}
