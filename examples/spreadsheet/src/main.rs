//! Runs the `spreadsheet` example on its own; the app itself is in `lib.rs`.

use egui_react::prelude::*;
use egui_react_app::{Options, run};
use spreadsheet::App;

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-react: spreadsheet"),
            // A grid of eight columns and twenty rows wants more than
            // eframe's default 640.
            #[cfg(not(target_arch = "wasm32"))]
            native: eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default().with_inner_size([980.0, 680.0]),
                ..Default::default()
            },
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
