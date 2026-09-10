//! Runs the virtualised egui list on its own, next to `cargo run -p list-10k`.

#[cfg(not(target_arch = "wasm32"))]
use list_10k::plain::{self, PlainState};

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
        "egui-reactor: list-10k (plain egui)",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::<PlainApp>::default())),
    )
}

/// The same plain list on the web, for a like-for-like measurement against the
/// egui-reactor binary (`docs/tasks/list-perf/measurements.md`). Build it with
/// `trunk build --release index-plain.html` from this directory.
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    use list_10k::plain::{self, PlainState};

    #[derive(Default)]
    struct PlainApp {
        state: PlainState,
    }

    impl eframe::App for PlainApp {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            egui::CentralPanel::default().show(ui, |ui| plain::ui(ui, &mut self.state));
        }
    }

    let canvas = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("egui_reactor_canvas"))
        .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        .expect("no <canvas id=\"egui_reactor_canvas\"> in the document");
    wasm_bindgen_futures::spawn_local(async move {
        eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|_cc| Ok(Box::<PlainApp>::default())),
            )
            .await
            .expect("could not start the web runner");
    });
}
