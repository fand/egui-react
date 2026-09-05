//! The same settings form written with egui alone.
//!
//! egui widgets take `&mut` the value they edit, exactly as a `bind` prop does,
//! so the widgets themselves are a wash. What differs is around them: the state
//! is one `struct` threaded through by hand, the log has to be collected before
//! the rows are drawn because the rows already borrow the state, and saving is
//! written out rather than being the difference between two hook names.

use crate::{LABEL, Settings, THEMES};

/// Everything the plain version keeps between frames.
#[derive(Default)]
pub struct PlainState {
    pub settings: Settings,
    pub log: Vec<String>,
}

/// The key the standalone binary stores the settings under.
pub const STORAGE_KEY: &str = "form_plain";

impl PlainState {
    /// `use_persisted("form/settings", ..)` is this, plus the key.
    pub fn load(json: &str) -> Self {
        Self {
            settings: serde_json::from_str(json).unwrap_or_default(),
            log: Vec::new(),
        }
    }

    /// Serialize the settings for the caller to write into eframe's storage.
    /// The log is not part of them.
    pub fn save(&self) -> String {
        serde_json::to_string(&self.settings).unwrap_or_else(|_| String::from("{}"))
    }
}

pub fn ui(ui: &mut egui::Ui, state: &mut PlainState) {
    // Read what the rows below will need, before they borrow the state.
    let summary = state.settings.summary();
    let entries: Vec<String> = state.log.iter().rev().take(8).cloned().collect();
    let mut log = Vec::new();

    egui::Frame::new().inner_margin(12.0).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.label(egui::RichText::new("settings").size(22.0).strong());

        // A `Grid` does the column alignment that `<Field>`'s label width does
        // on the other side; `min_col_width` is what makes the labels a column
        // rather than each row starting where its own text ended.
        egui::Grid::new("form")
            .num_columns(2)
            .min_col_width(LABEL)
            .show(ui, |ui| {
                let settings = &mut state.settings;

                ui.label("name");
                let name =
                    ui.add(egui::TextEdit::singleline(&mut settings.name).desired_width(200.0));
                if name.changed() {
                    log.push(String::from("name edited"));
                }
                ui.end_row();

                ui.label("notify");
                if ui.checkbox(&mut settings.notify, "").changed() {
                    log.push(format!("notify = {}", settings.notify));
                }
                ui.end_row();

                ui.label("autosave");
                if ui.checkbox(&mut settings.autosave, "").changed() {
                    log.push(format!("autosave = {}", settings.autosave));
                }
                ui.end_row();

                ui.label("volume");
                if ui
                    .add(egui::Slider::new(&mut settings.volume, 0..=100))
                    .changed()
                {
                    log.push(String::from("volume changed"));
                }
                ui.end_row();

                ui.label("theme");
                let theme = egui::ComboBox::from_id_salt("theme").show_index(
                    ui,
                    &mut settings.theme,
                    THEMES.len(),
                    |i| THEMES[i],
                );
                if theme.changed() {
                    log.push(format!("theme = {}", THEMES[settings.theme]));
                }
                ui.end_row();
            });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label(&summary);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("reset").clicked() {
                    state.settings = Settings::default();
                    log.push(String::from("reset"));
                }
            });
        });

        if !entries.is_empty() {
            egui::CollapsingHeader::new(format!("log ({})", entries.len()))
                .id_salt("log")
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for line in &entries {
                        ui.label(line);
                    }
                });
        }
    });

    state.log.extend(log);
}
