//! Runs the `fetch` example on its own; the app itself is in `lib.rs`.

use egui_reactor::prelude::*;
use egui_reactor_app::{Options, run};
use fetch::App;

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-reactor: fetch"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
