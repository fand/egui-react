//! Spike example. For now it only opens an empty eframe window; the hand-written
//! Counter and Dialog are added in a later step of the spike.

use std::num::NonZeroUsize;

use react_egui as _;

struct SpikeApp;

impl eframe::App for SpikeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.label("react-egui spike");
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "react-egui spike",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            cc.egui_ctx
                .options_mut(|o| o.max_passes = NonZeroUsize::new(2).unwrap());
            Ok(Box::new(SpikeApp))
        }),
    )
}
