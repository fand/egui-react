//! Runs the `clock` example on its own; the app itself is in `lib.rs`.

use clock::App;
use egui_reactor::prelude::*;
use egui_reactor_app::{Options, run};

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-reactor: clock"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
