//! The same layout tour written with egui alone, for the gallery's side by
//! side. This is where the difference stops being cosmetic.
//!
//! egui lays out one widget after another and never looks back, so there is no
//! free space to distribute and nothing to distribute it with. Everything
//! `<View>` spells as an attribute — `justify`, `grow`, `wrap`, `display=grid`
//! — is arithmetic here: measure the children, work out the leftover, and
//! allocate it in the right places by hand. The last section is the one case
//! where the two versions are the same length, because it is egui's own
//! containers on both sides.

/// The plain version keeps nothing between frames; the tour has no state.
///
/// It exists so the gallery can drive every plain example through one shape.
#[derive(Default)]
pub struct PlainState;

/// The padding inside a chip, matching `<Chip p={6}>`.
const CHIP_PAD: f32 = 6.0;

pub fn ui(ui: &mut egui::Ui, _state: &mut PlainState) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Frame::new().inner_margin(12.0).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(egui::RichText::new("layout").size(22.0).strong());

            section(ui, "direction=\"row\"", |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    for label in ["one", "two", "three"] {
                        chip(ui, label);
                    }
                });
            });

            section(ui, "justify", |ui| {
                // The rows carry `mb={4}` on top of the section's own `gap={4}`,
                // so 8 between them and 4 after the last. Spelled out, because
                // egui adds its own spacing around anything it is not told not
                // to.
                ui.spacing_mut().item_spacing.y = 0.0;
                for (i, justify) in ["start", "center", "end", "space-between", "space-around"]
                    .into_iter()
                    .enumerate()
                {
                    if i > 0 {
                        ui.add_space(8.0);
                    }
                    justified_row(ui, justify, &["a", "b", "c"]);
                }
                ui.add_space(4.0);
            });

            section(ui, "align + grow", align_and_grow);

            section(ui, "wrap + gap", |ui| {
                let labels: Vec<String> = (0..8).map(|i| format!("item {i}")).collect();
                wrapped(ui, &labels, 260.0, egui::vec2(8.0, 4.0));
            });

            section(ui, "nested", nested);

            section(ui, "display=\"grid\" cols={3}", grid);

            section(ui, "egui containers as leaves", |ui| {
                // The one section that is the same on both sides: `<Grid>` and
                // `<Vertical>` are these two containers, wrapped as leaves.
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    egui::Grid::new("grid").num_columns(2).show(ui, |ui| {
                        ui.label("grid a1");
                        ui.label("grid b1");
                        ui.end_row();
                        ui.label("grid a2");
                        ui.label("grid b2");
                        ui.end_row();
                    });
                    ui.vertical(|ui| {
                        ui.label("vertical 1");
                        ui.label("vertical 2");
                    });
                });
            });
        });
    });
}

/// One labelled section: `<Section>` in the egui-react version.
fn section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.label(egui::RichText::new(title).strong());
    ui.add_space(4.0);
    egui::Frame::new().inner_margin(8.0).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.set_width(ui.available_width());
        body(ui);
    });
    ui.add_space(12.0);
}

/// `justify` by hand.
///
/// Flexbox states where the leftover space goes; egui needs it worked out.
/// Every child is measured, the leftover is divided according to the rule, and
/// the pieces are allocated as explicit gaps.
fn justified_row(ui: &mut egui::Ui, justify: &str, labels: &[&str]) {
    let widths: Vec<f32> = labels.iter().map(|label| chip_width(ui, label)).collect();
    let free = (ui.available_width() - widths.iter().sum::<f32>()).max(0.0);
    let n = labels.len() as f32;
    let (lead, between) = match justify {
        "start" => (0.0, 0.0),
        "center" => (free / 2.0, 0.0),
        "end" => (free, 0.0),
        "space-between" => (0.0, free / (n - 1.0)),
        "space-around" => (free / (2.0 * n), free / n),
        other => panic!("unknown justify {other:?}"),
    };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(lead);
        for (i, label) in labels.iter().enumerate() {
            if i > 0 {
                ui.add_space(between);
            }
            chip(ui, label);
        }
    });
}

/// `align="center"` on a 64px row, with `grow={1.0}` on the middle chip.
///
/// The row's height has to be allocated up front so there is something to
/// centre against, and the growing chip's width is the leftover, computed.
fn align_and_grow(ui: &mut egui::Ui) {
    let gap = 8.0;
    let fixed = chip_width(ui, "fixed");
    let width = ui.available_width();
    let grown = (width - 2.0 * fixed - 2.0 * gap).max(0.0);

    ui.allocate_ui_with_layout(
        egui::vec2(width, 64.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = gap;
            chip(ui, "fixed");
            sized_chip(ui, "grow=1", grown);
            chip(ui, "fixed");
        },
    );
}

/// `wrap` by hand: break the children into lines that fit, then draw the lines.
fn wrapped(ui: &mut egui::Ui, labels: &[String], width: f32, gap: egui::Vec2) {
    let mut lines: Vec<Vec<&str>> = Vec::new();
    let mut line: Vec<&str> = Vec::new();
    let mut used = 0.0;
    for label in labels {
        let needed = if line.is_empty() {
            chip_width(ui, label)
        } else {
            used + gap.x + chip_width(ui, label)
        };
        if !line.is_empty() && needed > width {
            lines.push(std::mem::take(&mut line));
            used = chip_width(ui, label);
        } else {
            used = needed;
        }
        line.push(label);
    }
    lines.push(line);

    ui.spacing_mut().item_spacing.y = gap.y;
    for line in lines {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap.x;
            for label in line {
                chip(ui, label);
            }
        });
    }
}

/// Two columns with `grow={1.0}` and `grow={2.0}`: a third and two thirds.
fn nested(ui: &mut egui::Ui) {
    let gap = 8.0;
    let labels = [["left top", "left bottom"], ["right top", "right bottom"]];
    let grow = [1.0, 2.0];

    // `grow` shares out what is *left over*, not the whole row, so each
    // column's own width has to be measured before the leftover can be split.
    let bases: Vec<f32> = labels
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|label| chip_width(ui, label))
                .fold(0.0, f32::max)
        })
        .collect();
    let free = (ui.available_width() - gap - bases.iter().sum::<f32>()).max(0.0);
    let total: f32 = grow.iter().sum();
    let widths: Vec<f32> = bases
        .iter()
        .zip(grow)
        .map(|(base, grow)| base + free * grow / total)
        .collect();

    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (width, labels) in widths.iter().zip(labels) {
            // A plain `allocate_ui` would inherit the row's left-to-right
            // layout and put all four chips in one line, so each column asks
            // for its own top-down `Ui` of the width worked out above.
            ui.allocate_ui_with_layout(
                egui::vec2(*width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    // The allocation shrinks to its content unless the column
                    // insists, and then the second column would start against
                    // the first one's text instead of at a third of the row.
                    ui.set_min_width(*width);
                    ui.spacing_mut().item_spacing.y = 4.0;
                    for label in labels {
                        chip(ui, label);
                    }
                },
            );
        }
    });
}

/// `display="grid" cols={3}`, including one cell spanning all three columns.
///
/// `egui::Grid` sizes its columns from their content, so equal columns and a
/// spanning cell both need the column width computed and allocated by hand.
fn grid(ui: &mut egui::Ui) {
    let gap = 6.0;
    let full = ui.available_width();
    let column = (full - 2.0 * gap) / 3.0;

    ui.spacing_mut().item_spacing.y = gap;
    ui.horizontal(|ui| sized_chip(ui, "spans three columns", full));
    for row in 0..2 {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for col in 0..3 {
                sized_chip(ui, &format!("cell {}", row * 3 + col), column);
            }
        });
    }
}

/// A chip: a label with `<Chip p={6}>`'s padding around it.
fn chip(ui: &mut egui::Ui, label: &str) {
    egui::Frame::new()
        .inner_margin(CHIP_PAD)
        .show(ui, |ui| ui.label(label));
}

/// A chip stretched to `width`, for the cases where taffy would have sized it.
fn sized_chip(ui: &mut egui::Ui, label: &str, width: f32) {
    egui::Frame::new().inner_margin(CHIP_PAD).show(ui, |ui| {
        ui.set_min_width((width - 2.0 * CHIP_PAD).max(0.0));
        ui.label(label);
    });
}

/// How wide [`chip`] will be.
fn chip_width(ui: &egui::Ui, label: &str) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let text = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, egui::Color32::PLACEHOLDER)
        .size()
        .x;
    text + 2.0 * CHIP_PAD
}
