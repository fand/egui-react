//! Runs the `counter` example on its own; the app itself is in `lib.rs`.

use counter::App;
use react_egui::prelude::*;
use react_egui_app::{Options, run};

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("react-egui: counter"),
            ..Default::default()
        },
        // The root closure gets a `Cx` it rarely needs: hooks belong in
        // components, so that a view can borrow their guards.
        |_cx| rsx! { <App/> },
    )
}
