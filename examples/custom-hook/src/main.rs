//! Runs the `custom-hook` example on its own; the app itself is in `lib.rs`.

use custom_hook::App;
use react_egui::prelude::*;
use react_egui_app::{Options, run};

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("react-egui: custom-hook"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
