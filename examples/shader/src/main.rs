//! Runs the `shader` example on its own; the app itself is in `lib.rs`.

use egui_reactor::prelude::*;
use egui_reactor_app::{Options, run};
use shader::{App, gpu};

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-reactor: shader"),
            // The one place the pipeline can be built: eframe has a window and
            // a wgpu device here, and nothing has been drawn yet.
            setup: Some(Box::new(gpu::setup)),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
