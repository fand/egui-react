//! Runs the `todo` example on its own; the app itself is in `lib.rs`.

use react_egui::prelude::*;
use react_egui_app::{Options, run};
use todo::App;

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("react-egui: todo"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
