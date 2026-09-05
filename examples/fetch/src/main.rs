//! Runs the `fetch` example on its own; the app itself is in `lib.rs`.

use fetch::App;
use react_egui::prelude::*;
use react_egui_app::{Options, run};

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("react-egui: fetch"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
