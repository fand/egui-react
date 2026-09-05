//! The same list, virtualised.
//!
//! `ScrollArea::show_rows` asks how tall a row is and how many there are, works
//! out which ones the viewport covers, and calls back with just that range. The
//! rest of the list costs nothing but a reserved height. That is the standard
//! immediate-mode answer to a long list, and it is why the frame time here does
//! not move when the count goes from a hundred to ten thousand.
//!
//! The other half of the difference is `use_memo`, written out by hand: the
//! rows are rebuilt only when the count, the filter or the removals change.
//! Without that, ten thousand `String`s per frame would cost more than drawing
//! them.

use std::collections::BTreeSet;

use crate::{DEFAULT_COUNT, INDEX_W, ROW_GAP, ROW_H, rows};

/// Everything the plain version keeps between frames, including the cache.
pub struct PlainState {
    pub count: usize,
    pub filter: String,
    pub removed: BTreeSet<usize>,
    /// The rows as last built, and what they were built from.
    cache: Vec<(usize, String)>,
    built_from: (usize, String, usize),
}

impl Default for PlainState {
    fn default() -> Self {
        Self::with_count(DEFAULT_COUNT)
    }
}

impl PlainState {
    pub fn with_count(count: usize) -> Self {
        Self {
            count,
            filter: String::new(),
            removed: BTreeSet::new(),
            cache: Vec::new(),
            built_from: (usize::MAX, String::new(), usize::MAX),
        }
    }

    /// `use_memo`, by hand: compare the inputs, rebuild only on a change.
    fn rows(&mut self) -> &[(usize, String)] {
        let now = (self.count, self.filter.clone(), self.removed.len());
        if self.built_from != now {
            self.cache = rows(self.count, &self.filter, &self.removed);
            self.built_from = now;
        }
        &self.cache
    }
}

pub fn ui(ui: &mut egui::Ui, state: &mut PlainState) {
    let frame_ms = ui.input(|i| i.stable_dt) * 1000.0;
    let mut remove = None;

    egui::Frame::new().inner_margin(12.0).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.label(egui::RichText::new("list-10k").size(22.0).strong());

        ui.add(egui::Slider::new(&mut state.count, 100..=10_000).text("rows"));
        ui.add(
            egui::TextEdit::singleline(&mut state.filter)
                .desired_width(220.0)
                .hint_text("filter"),
        );

        let shown = state.rows().len();
        ui.horizontal(|ui| {
            ui.label(format!("showing {shown}"));
            ui.label(format!(
                "last frame {frame_ms:.1} ms ({:.0} fps)",
                1000.0 / frame_ms.max(0.001)
            ));
        });

        // The gap between rows is the scroll area's own item spacing, because
        // `show_rows` adds it to the row height when it works out the range.
        ui.spacing_mut().item_spacing.y = ROW_GAP;
        let visible = state.rows();
        egui::ScrollArea::vertical().show_rows(ui, ROW_H, visible.len(), |ui, range| {
            for (i, name) in &visible[range] {
                ui.horizontal(|ui| {
                    // A column of its own width, like the other version's
                    // `<Text w={INDEX_W}>`. `add_sized` would centre the text
                    // in the box, and an allocation with no minimum would
                    // shrink to the text, so the names would not line up.
                    ui.allocate_ui_with_layout(
                        egui::vec2(INDEX_W, ROW_H),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_width(INDEX_W);
                            ui.label(format!("#{i}"))
                        },
                    );
                    ui.label(name);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("x").clicked() {
                            remove = Some(*i);
                        }
                    });
                });
            }
        });
    });

    if let Some(i) = remove {
        state.removed.insert(i);
    }
}
