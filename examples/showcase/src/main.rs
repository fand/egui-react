//! Runs the `showcase` example; the app itself is in `lib.rs`.

use react_egui::prelude::*;
use react_egui_app::{Options, run};
use showcase::App;

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("react-egui: showcase"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
