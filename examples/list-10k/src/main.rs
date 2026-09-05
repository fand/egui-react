//! Runs the `list-10k` example on its own; the app itself is in `lib.rs`.

use list_10k::App;
use react_egui::prelude::*;
use react_egui_app::{Options, run};

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("react-egui: list-10k"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
