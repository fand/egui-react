//! Runs the `form` example on its own; the app itself is in `lib.rs`.

use egui_react::prelude::*;
use egui_react_app::{Options, run};
use form::App;

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-react: form"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
