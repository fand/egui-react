//! Runs the `form` example on its own; the app itself is in `lib.rs`.

use form::App;
use react_egui::prelude::*;
use react_egui_app::{Options, run};

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("react-egui: form"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
