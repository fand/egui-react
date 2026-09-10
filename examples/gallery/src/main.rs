//! Runs the gallery; the page itself is in `lib.rs`.

use egui_reactor::prelude::*;
use egui_reactor_app::a11y::WebA11y;
use egui_reactor_app::{Options, run};
use gallery::{App, initial_example};

fn main() -> eframe::Result {
    // Read once, before the first pass: the root closure runs every pass and
    // the hash changes as the user picks examples.
    let start = initial_example();
    let canvas_id = Options::default().canvas_id;
    run(
        Options {
            title: String::from("egui-reactor: gallery"),
            setup: Some(Box::new(move |cc| {
                // What the two wgpu examples need before anything is drawn: the
                // shader example's pipeline, and the buffer and layouts the patch
                // example builds its pipelines from. Each lands in
                // `callback_resources` under its own type, so they cost the other
                // examples nothing and cannot clash with each other.
                shader::gpu::setup(cc);
                patch::gpu::setup(cc);
                // The accessibility tree, mirrored into hidden DOM elements
                // over the canvas. Does nothing off the web; there, egui's
                // tree would otherwise be thrown away by eframe.
                cc.egui_ctx.add_plugin(WebA11y::new(canvas_id));
            })),
            // Three columns need more than eframe's default 640: the list and
            // the code take 200 and 40%, and the example gets what is left.
            #[cfg(not(target_arch = "wasm32"))]
            native: eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
                ..Default::default()
            },
            ..Default::default()
        },
        move |_cx| rsx! { <App start={start}/> },
    )
}
