//! A `<Text>` whose width changes from frame to frame must not cost a second
//! pass: the galley is painted where the final layout puts it, and no other
//! node moves.

mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_react::prelude::*;
use egui_react_elements::prelude::*;

/// A label whose width changes on every frame, as a frame-time or fps readout
/// does. (egui's digits are all one width, so two numbers with the same digit
/// count would not do here.)
fn label(frame: usize) -> String {
    if frame.is_multiple_of(2) {
        String::from("iii")
    } else {
        String::from("WWWWWWWWWW")
    }
}

/// How many of `frames` frames asked for a discard, and the reasons they gave.
fn discards_over(
    frames: usize,
    body: impl Fn(&mut Cx<'_, '_>, usize) + 'static,
) -> (usize, Vec<String>) {
    let discards = Rc::new(Cell::new(0usize));
    let reasons = Rc::new(RefCell::new(Vec::new()));
    let frame = Rc::new(Cell::new(0usize));
    let counted = Rc::clone(&discards);
    let collected = Rc::clone(&reasons);
    let frame_in_app = Rc::clone(&frame);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui_state(
            move |ui, store: &mut Store| {
                run_app(ui, store, |cx| body(cx, frame_in_app.get()));
                if ui.ctx().output(|o| o.requested_discard()) {
                    counted.set(counted.get() + 1);
                    ui.ctx().output(|o| {
                        collected.borrow_mut().extend(
                            o.request_discard_reasons
                                .iter()
                                .map(|cause| cause.reason.to_string()),
                        );
                    });
                }
            },
            Store::new(),
        );
    harness.run();
    harness.run();
    discards.set(0);
    reasons.borrow_mut().clear();
    for n in 0..frames {
        frame.set(n);
        harness.run_steps(1);
    }
    (discards.get(), reasons.borrow().clone())
}

#[test]
fn a_text_that_changes_width_costs_no_second_pass() {
    let (discards, reasons) = discards_over(10, |cx, frame| {
        rsx! {
            <View direction="column" gap={8} p={12} grow={1.0}>
                <View direction="row" gap={8} align="center">
                    <Text>"showing 10000"</Text>
                    <Text>{label(frame)}</Text>
                </View>
            </View>
        }
        .show(cx);
    });
    assert_eq!(
        discards, 0,
        "a text that only changed its own width asked for a discard: {reasons:?}"
    );
}

/// The guard for the rule above: a widget after the text is drawn at last
/// frame's rect, so when the text pushes it along the frame is wrong and must
/// be drawn again.
#[test]
fn a_widget_pushed_by_a_wider_text_still_costs_a_pass() {
    let (discards, reasons) = discards_over(10, |cx, frame| {
        rsx! {
            <View direction="column" gap={8} p={12} grow={1.0}>
                <View direction="row" gap={8} align="center">
                    <Text>{label(frame)}</Text>
                    <Button>"x"</Button>
                </View>
            </View>
        }
        .show(cx);
    });
    assert_eq!(
        discards, 9,
        "a widget that moved was not drawn again: {reasons:?}"
    );
}

#[test]
fn a_text_that_changes_width_in_a_lite_row_costs_no_second_pass() {
    let (discards, reasons) = discards_over(10, |cx, frame| {
        rsx! {
            <View direction="column" w="100%" h={280.0}>
                <VirtualList rows={100} row_h={20.0} grow={1.0}
                    render={move |cx: &mut Cx<'_, '_>, i: usize| {
                        rsx! {
                            <View direction="row" gap={8} align="center" w="100%" h={18.0}>
                                <Text w={64.0}>{format!("#{i}")}</Text>
                                <Text>{label(frame + i)}</Text>
                            </View>
                        }
                        .show(cx);
                    }}
                />
            </View>
        }
        .show(cx);
    });
    assert_eq!(
        discards, 0,
        "a text that only changed its own width asked for a discard: {reasons:?}"
    );
}
