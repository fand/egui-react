//! Runs the `custom-hook` example on its own; the app itself is in `lib.rs`.

use custom_hook::App;
use egui_react::prelude::*;
use egui_react_app::{Options, run};

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-react: custom-hook"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
