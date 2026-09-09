//! Runs the `notes` example; the app itself is in `lib.rs`.

use egui_react::prelude::*;
use egui_react_app::{Options, run};
use notes::{App, fonts};

fn main() -> eframe::Result {
    // The font is applied before the first frame, from `setup`, which is
    // where an app normally does it. `App` applies it once more on its first
    // frame for the gallery's sake; here that changes nothing.
    let fonts = fonts();
    run(
        Options {
            title: String::from("egui-react: notes"),
            setup: Some(Box::new(move |cc| fonts.apply(&cc.egui_ctx))),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
