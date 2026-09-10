//! Runs the `escape-hatch` example on its own; the app itself is in `lib.rs`.

use egui_reactor::prelude::*;
use egui_reactor_app::{Options, run};
use escape_hatch::App;

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-reactor: escape-hatch"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
