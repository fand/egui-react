//! Runs the gallery; the page itself is in `lib.rs`.

use gallery::{App, initial_example};
use react_egui::prelude::*;
use react_egui_app::{Options, run};

fn main() -> eframe::Result {
    // Read once, before the first pass: the root closure runs every pass and
    // the hash changes as the user picks examples.
    let start = initial_example();
    run(
        Options {
            title: String::from("react-egui: gallery"),
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
