//! Runs the gallery; the page itself is in `lib.rs`.

use gallery::{App, initial_example};
use react_egui::prelude::*;
use react_egui_app::a11y::WebA11y;
use react_egui_app::{Options, run};

fn main() -> eframe::Result {
    // Read once, before the first pass: the root closure runs every pass and
    // the hash changes as the user picks examples.
    let start = initial_example();
    let canvas_id = Options::default().canvas_id;
    run(
        Options {
            title: String::from("react-egui: gallery"),
            setup: Some(Box::new(move |cc| {
                // The shader example's pipeline, built once for the whole
                // gallery. It lands in `callback_resources` under its own
                // type, so it costs the other examples nothing and cannot
                // clash with them.
                shader::gpu::setup(cc);
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
