//! Runs the `shell` example; the app itself is in `lib.rs`.

use egui_react::prelude::*;
use egui_react_app::{Options, run};
use shell::App;

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-react: shell"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
