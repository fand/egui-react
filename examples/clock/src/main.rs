//! Runs the `clock` example on its own; the app itself is in `lib.rs`.

use clock::App;
use react_egui::prelude::*;
use react_egui_app::{Options, run};

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("react-egui: clock"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
