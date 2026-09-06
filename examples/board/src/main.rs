//! Runs the `board` example on its own; the app itself is in `lib.rs`.

use board::App;
use egui_react::prelude::*;
use egui_react_app::{Options, run};

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-react: board"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
