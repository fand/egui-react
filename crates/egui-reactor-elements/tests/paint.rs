//! Plan step 4: the paint props on the elements users write.
//!
//! The engine's own tests (`egui-reactor/tests/paint.rs`) check the shapes; these
//! check that `rsx!` and the elements pass them on: a `<View>` reserves what it
//! paints, a `<Button>` takes its padding over as the widget's own, and a
//! widget that paints a box of its own still works when the caller paints one.
//!
//! A `<View>` registers nothing an accessibility query can find, so its box is
//! measured against plain `<Label>`s around it: everything the painted view
//! reserves shows up as the distance between them and the label inside it.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::prelude::*;
use egui_reactor_elements::prelude::*;

const PAD: f32 = 8.0;
const BORDER: f32 = 2.0;

/// A painted view between four rulers: three in its row, one in the line under.
///
/// `border` picks the width of the stroke, `0.0` for the case without one: a
/// border of no width reserves nothing, which is half of what is asserted.
fn app(cx: &mut Cx<'_, '_>, border: f32) {
    rsx! {
        <View direction="column" align="start" gap={0}>
            <View direction="row" align="start" gap={0}>
                <Label>"before"</Label>
                <View
                    bg={egui::Color32::RED}
                    p={PAD}
                    border={egui::Stroke::new(border, egui::Color32::BLACK)}
                >
                    <Label>"inside"</Label>
                </View>
                <Label>"after"</Label>
            </View>
            <Label>"below"</Label>
        </View>
    }
    .show(cx);
}

fn harness_for(border: f32) -> Harness<'static, Store> {
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            run_app(ui, store, move |cx| app(cx, border));
        },
        Store::new(),
    );
    // Three frames, so every leaf has drawn once and the layout has settled.
    harness.run();
    harness.run();
    harness.run();
    harness
}

/// `a` and `b` are the same distance, up to a point.
///
/// A widget's rect is the galley it drew, and the leaf around it is that size
/// `ceil`ed, so a measurement taken from one widget to the next carries up to
/// a point of rounding — the same slack `widgets.rs` allows.
#[track_caller]
fn close(a: f32, b: f32, what: &str) {
    assert!((a - b).abs() <= 1.0, "{what}: {a} != {b}");
}

#[test]
fn a_painted_view_reserves_its_padding_and_its_border() {
    for (border, inset) in [(0.0, PAD), (BORDER, PAD + BORDER)] {
        let harness = harness_for(border);
        let before = harness.get_by_label("before").rect();
        let inside = harness.get_by_label("inside").rect();
        let after = harness.get_by_label("after").rect();
        let below = harness.get_by_label("below").rect();

        close(inside.left() - before.right(), inset, "the left inset");
        close(after.left() - inside.right(), inset, "the right inset");
        close(inside.top() - before.top(), inset, "the top inset");
        close(below.top() - inside.bottom(), inset, "the bottom inset");
    }
}

#[test]
fn a_buttons_padding_becomes_the_widgets_own() {
    let clicks = Rc::new(Cell::new(0u32));
    let clicks_in_app = Rc::clone(&clicks);

    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let clicks = Rc::clone(&clicks_in_app);
            run_app(ui, store, move |cx| {
                let bg = cx.ui().visuals().widgets.inactive.weak_bg_fill;
                rsx! {
                    <View direction="column">
                        <Button label="plain">"x"</Button>
                        <Button
                            label="floating"
                            p={PAD}
                            radius={24.0}
                            shadow
                            bg={bg}
                            on_click={|| clicks.set(clicks.get() + 1)}
                        >"x"</Button>
                    </View>
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    harness.run();
    let base = harness.ctx.global_style().spacing.button_padding;
    let plain = harness.get_by_label("plain").rect();
    let floating = harness.get_by_label("floating").rect();

    // The widget itself grew, which is the point of `button_padding`: the pill
    // it draws and the area it takes clicks in are the whole box. The leaf
    // measure is `ceil`ed, so allow a point either way.
    close(
        floating.width() - plain.width(),
        2.0 * (PAD - base.x),
        "the button's width",
    );
    close(
        floating.height() - plain.height(),
        2.0 * (PAD - base.y),
        "the button's height",
    );

    // And it is still a button.
    harness.get_by_label("floating").click();
    harness.run();
    assert_eq!(clicks.get(), 1);
}

#[test]
fn a_painted_text_edit_and_combo_box_still_work() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                let mut text = use_state(cx, String::new);
                let mut index = use_state(cx, || 0usize);
                let options = ["alpha", "beta", "gamma"];
                rsx! {
                    <View direction="column" gap={4}>
                        <TextEdit
                            bind={text.bind()}
                            bg={egui::Color32::DARK_BLUE}
                            border={egui::Stroke::new(1.0, egui::Color32::WHITE)}
                            radius={4.0}
                        />
                        <ComboBox
                            bind={index.bind()}
                            options={&options}
                            bg={egui::Color32::DARK_GREEN}
                            radius={4.0}
                        />
                    </View>
                }
                .show(cx);
                let (text, index) = (text.clone(), *index);
                cx.ui().label(format!("value: {text} / {index}"));
            });
        },
        Store::new(),
    );

    harness.run();
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .focus();
    harness.run();
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .type_text("hi");
    harness.run();
    assert!(harness.query_by_label("value: hi / 0").is_some());

    harness.get_by_role(egui::accesskit::Role::ComboBox).click();
    harness.run();
    harness.get_by_label("gamma").click();
    harness.run();
    assert!(harness.query_by_label("value: hi / 2").is_some());
}
