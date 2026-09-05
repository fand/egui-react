//! Runs the plain egui form on its own, next to `cargo run -p form`.

#[cfg(not(target_arch = "wasm32"))]
use form::plain::{self, PlainState, STORAGE_KEY};

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct PlainApp {
    state: PlainState,
}

#[cfg(not(target_arch = "wasm32"))]
impl eframe::App for PlainApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| plain::ui(ui, &mut self.state));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(STORAGE_KEY, self.state.save());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    eframe::run_native(
        "react-egui: form (plain egui)",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            let state = cc
                .storage
                .and_then(|storage| storage.get_string(STORAGE_KEY))
                .map_or_else(PlainState::default, |json| PlainState::load(&json));
            Ok(Box::new(PlainApp { state }))
        }),
    )
}

/// The plain versions are native only. `trunk` builds the react-egui binaries,
/// and `cargo check --target wasm32` builds every binary in the workspace.
#[cfg(target_arch = "wasm32")]
fn main() {}
