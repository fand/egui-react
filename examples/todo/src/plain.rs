//! The same todo list written with egui alone, for the gallery's side by side.
//!
//! Three differences show up here that the counter is too small to expose.
//! The state is one `struct` that every function takes `&mut`, instead of a
//! hook per piece. The list cannot be edited while it is being iterated, so
//! the index of whatever the user clicked has to be carried out of the loop
//! and applied afterwards — which is what `use_reducer`'s `Dispatch` does for
//! you. And persistence is written by hand, rather than being the difference
//! between `use_state` and `use_persisted`.

use serde::{Deserialize, Serialize};

use crate::Todo;

/// Everything the plain version keeps between frames.
///
/// The egui-react version has no such type. `saved`, `draft` and the reducer's
/// list are three hooks, each owned by the component that reads it.
#[derive(Default, Serialize, Deserialize)]
pub struct PlainState {
    pub todos: Vec<Todo>,
    /// The text field's buffer. Not saved: it is not part of the list.
    #[serde(skip)]
    pub draft: String,
}

/// The key the standalone binary stores the list under.
pub const STORAGE_KEY: &str = "todo_plain";

impl PlainState {
    /// Read the list back. `use_persisted` is this, plus the key.
    pub fn load(json: &str) -> Self {
        serde_json::from_str(json).unwrap_or_default()
    }

    /// Serialize the list for the caller to write into eframe's storage.
    pub fn save(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| String::from("{}"))
    }
}

pub fn ui(ui: &mut egui::Ui, state: &mut PlainState) {
    // `p={12}` and `gap={8}` on the root `<View>`.
    egui::Frame::new().inner_margin(12.0).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.label(egui::RichText::new("todo").size(22.0).strong());

        let remaining = format!("{} left", state.todos.iter().filter(|t| !t.done).count());
        ui.horizontal(|ui| {
            // `<TextEdit grow={1.0}>` next to a label: the field's width is
            // whatever the label does not need.
            let label_width = text_width(ui, &remaining);
            let width = ui.available_width() - label_width - ui.spacing().item_spacing.x;
            let edit = ui.add_sized(
                egui::vec2(width, ui.spacing().interact_size.y),
                egui::TextEdit::singleline(&mut state.draft).hint_text("what needs doing?"),
            );
            if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let text = state.draft.trim().to_owned();
                if !text.is_empty() {
                    state.todos.push(Todo { text, done: false });
                }
                state.draft.clear();
            }
            ui.label(remaining);
        });

        ui.separator();

        // The loop borrows the list, so it cannot remove from it. The index
        // is carried out here and applied below; the egui-react version
        // sends `Msg::Remove(i)` and never sees this.
        let mut remove = None;
        for (i, todo) in state
            .todos
            .iter_mut()
            .enumerate()
            .filter(|(_, todo)| !todo.done)
        {
            ui.horizontal(|ui| {
                ui.checkbox(&mut todo.done, "");
                ui.label(&todo.text);
                // `<Text grow={1.0}>` before the button: here the button is
                // put in a right-to-left `Ui` filling the rest of the row.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let button = ui.button("x");
                    // `<Button label="remove">`, by hand: an "x" is drawn but
                    // "remove" is the name assistive technology reads.
                    ui.ctx()
                        .accesskit_node_builder(button.id, |node| node.set_label("remove"));
                    if button.clicked() {
                        remove = Some(i);
                    }
                });
            });
        }
        if let Some(i) = remove {
            state.todos.remove(i);
        }

        let done: Vec<usize> = state
            .todos
            .iter()
            .enumerate()
            .filter(|(_, todo)| todo.done)
            .map(|(i, _)| i)
            .collect();
        if !done.is_empty() {
            // Same again: the clicks are collected, then applied.
            let mut undo = None;
            let mut clear = false;
            egui::CollapsingHeader::new(format!("done ({})", done.len()))
                .id_salt("done")
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    for &i in &done {
                        ui.horizontal(|ui| {
                            ui.label(&state.todos[i].text);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("undo").clicked() {
                                        undo = Some(i);
                                    }
                                },
                            );
                        });
                    }
                    clear = ui.button("clear done").clicked();
                });
            if let Some(i) = undo {
                state.todos[i].done = false;
            }
            if clear {
                state.todos.retain(|todo| !todo.done);
            }
        }
    });
}

/// The width `text` will take as a default label.
fn text_width(ui: &egui::Ui, text: &str) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, egui::Color32::PLACEHOLDER)
        .size()
        .x
}
