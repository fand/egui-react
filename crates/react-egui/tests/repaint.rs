//! ARCHITECTURE.md 5.6: a frame that mutates state through `DerefMut` requests
//! a repaint; a frame that only reads does not.
//!
//! The UI is labels only (no buttons, no hover, no cursor blink), so egui has
//! no reason of its own to ask for another frame.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use react_egui::prelude::*;

#[test]
fn repaint_is_requested_only_on_mutation() {
    let mutate = Rc::new(Cell::new(false));
    let mutate_in_app = Rc::clone(&mutate);

    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let mutate = Rc::clone(&mutate_in_app);
            run_app(ui, store, move |cx| {
                let mut n = use_state(cx, || 0i32);
                if mutate.get() {
                    *n += 1;
                }
                cx.ui.label(format!("n: {}", *n));
            });
        },
        Store::new(),
    );

    // Settle: `run` loops until nothing asks for another frame.
    harness.run();
    assert!(!harness.ctx.has_requested_repaint());

    // A read-only frame must not request a repaint.
    harness.step();
    assert!(!harness.ctx.has_requested_repaint());

    // A frame that mutates through `DerefMut` must.
    mutate.set(true);
    harness.step();
    assert!(harness.ctx.has_requested_repaint());

    mutate.set(false);
    harness.run();
    assert!(!harness.ctx.has_requested_repaint());
}
