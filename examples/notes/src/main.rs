//! Runs the `notes` example; the app itself is in `lib.rs`.

use egui_react::prelude::*;
use egui_react_app::{Options, run};
use notes::App;

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-react: notes"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
