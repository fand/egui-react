//! Runs the `list-10k` example on its own; the app itself is in `lib.rs`.

use list_10k::App;
use egui_react::prelude::*;
use egui_react_app::{Options, run};

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-react: list-10k"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
