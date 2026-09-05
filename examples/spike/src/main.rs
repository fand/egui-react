//! Visual check for the spike: a hand-written Counter and Dialog driven by the
//! react-egui core, with no macros yet.

mod components;

use std::num::NonZeroUsize;

use react_egui::prelude::*;
use react_egui::rsx;

/// Owns the hook store, exactly as `react-egui-app`'s runner will.
#[derive(Default)]
struct SpikeApp {
    store: Store,
}

impl eframe::App for SpikeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // The root `Ui` eframe hands out has no margin and no background, so in
        // light mode the text would be drawn dark-on-dark and be invisible.
        // `CentralPanel` paints the panel fill first; `react-egui-app::run`
        // must do the same.
        egui::CentralPanel::default().show(ui, |ui| {
            self.store.begin_pass(ui.ctx());
            {
                let store: &Store = &self.store;
                let mut cx = Cx::new(store, ui, egui::Id::new("root"));
                rsx! { <components::App/> }.show(&mut cx);
            }
            // Every guard died with the component bodies above, so the sweep is safe.
            self.store.end_pass();
        });
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "react-egui spike",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            cc.egui_ctx
                .options_mut(|o| o.max_passes = NonZeroUsize::new(2).unwrap());
            Ok(Box::new(SpikeApp::default()))
        }),
    )
}
