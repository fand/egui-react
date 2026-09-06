//! Runs the `todo` example on its own; the app itself is in `lib.rs`.

use egui_react::prelude::*;
use egui_react_app::{Options, run};
use todo::App;

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-react: todo"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
