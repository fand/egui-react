//! A picture of the whole frame, so a change to the docking is noticed.
//!
//! `shell` keeps its own `snapshot` feature rather than joining the gallery's,
//! because it is not in the gallery: its panels carve the window, so the
//! gallery would have to depend on it for the picture alone.
//!
//! ```sh
//! cargo test -p shell --features snapshot
//! ```
//!
//! Regenerate the image with `UPDATE_SNAPSHOTS=1` and commit it.

#![cfg(feature = "snapshot")]

use egui_kittest::Harness;
use react_egui::prelude::*;
use react_egui_app::{root_id, root_style};
use shell::App;

#[test]
fn shell() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 600.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::default())
        .build_ui_state(
            |ui, store: &mut Store| {
                store.begin_pass(ui.ctx());
                {
                    let store: &Store = store;
                    let mut cx = Cx::new(store, ui, root_id());
                    let view = rsx! { <App/> };
                    cx.root_container(root_id(), root_style(), |cx| view.show(cx));
                }
                store.end_pass();
            },
            Store::new(),
        );
    harness.run();
    harness.snapshot("shell");
}
