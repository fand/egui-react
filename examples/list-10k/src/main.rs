//! Runs the `list-10k` example on its own; the app itself is in `lib.rs`.
//! `Compare` adds the "plain egui" switch, so the plain version is a click away
//! rather than a second binary (`list-10k-plain` still exists for a build with
//! no egui-reactor in it at all).

use egui_reactor::prelude::*;
use egui_reactor_app::{Options, run};
use list_10k::Compare;

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-reactor: list-10k"),
            ..Default::default()
        },
        |_cx| rsx! { <Compare/> },
    )
}
