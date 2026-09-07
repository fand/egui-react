//! Runs the `patch` example on its own; the app itself is in `lib.rs`.

use egui_react::prelude::*;
use egui_react_app::{Options, run};
use patch::{App, gpu};

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-react: patch"),
            // The buffer and the layouts, once. The pipeline cannot be built
            // here: which shader to compile is a question about a patch that
            // has not been loaded yet, so `gpu::prepare` builds it the first
            // time a program arrives.
            setup: Some(Box::new(gpu::setup)),
            // Three columns want more than eframe's default 640.
            #[cfg(not(target_arch = "wasm32"))]
            native: eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default().with_inner_size([1180.0, 760.0]),
                ..Default::default()
            },
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
