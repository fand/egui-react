//! Runs the plain egui layout tour on its own, next to `cargo run -p layout`.

#[cfg(not(target_arch = "wasm32"))]
use layout::plain::{self, PlainState};

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
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    eframe::run_native(
        "react-egui: layout (plain egui)",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::<PlainApp>::default())),
    )
}

/// The plain versions are native only. `trunk` builds the react-egui binaries,
/// and `cargo check --target wasm32` builds every binary in the workspace.
#[cfg(target_arch = "wasm32")]
fn main() {}
