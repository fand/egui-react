//! Plan 3.2 / test 4-1: every widget is driven through kittest, and both its
//! `bind` and its events behave.

mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use react_egui::prelude::*;
use react_egui_elements::prelude::*;

type Log = Rc<RefCell<Vec<String>>>;

fn log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

#[test]
fn button_click_and_enabled() {
    let clicks = Rc::new(std::cell::Cell::new(0u32));
    let clicks_in_app = Rc::clone(&clicks);

    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let clicks = Rc::clone(&clicks_in_app);
            run_app(ui, store, move |cx| {
                rsx! {
                    <Button on_click={|| clicks.set(clicks.get() + 1)}>"press me"</Button>
                    <Button enabled={false} on_click={|| clicks.set(999)}>"disabled"</Button>
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    harness.get_by_label("press me").click();
    harness.run();
    assert_eq!(clicks.get(), 1);

    // A disabled button is still in the tree but does not fire.
    harness.get_by_label("disabled").click();
    harness.run();
    assert_eq!(clicks.get(), 1);
}

#[test]
fn label_and_text_draw_their_children() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                rsx! {
                    <Label>"a label"</Label>
                    <Text strong size={18.0}>"styled text"</Text>
                    <Separator/>
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("a label").is_some());
    assert!(harness.query_by_label("styled text").is_some());
}

#[test]
fn text_edit_binds_and_reports_change_and_submit() {
    let log = log();
    let log_in_app = Rc::clone(&log);

    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let log = Rc::clone(&log_in_app);
            run_app(ui, store, move |cx| {
                let mut text = use_state(cx, String::new);
                let mut entries = log.borrow_mut();
                rsx! {
                    <TextEdit
                        bind={text.bind()}
                        hint="type here"
                        on_change={|| entries.push(String::from("change"))}
                        on_submit={|text: String| entries.push(format!("submit {text}"))}
                    />
                }
                .show(cx);
                drop(entries);
                cx.ui().label(format!("value: {}", *text));
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("value: ").is_some());

    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .focus();
    harness.run();
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .type_text("hi");
    harness.run();
    assert!(harness.query_by_label("value: hi").is_some());
    assert!(log.borrow().iter().any(|e| e == "change"));

    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(log.borrow().iter().any(|e| e == "submit hi"));
}

#[test]
fn checkbox_binds_and_reports_the_new_value() {
    let log = log();
    let log_in_app = Rc::clone(&log);

    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let log = Rc::clone(&log_in_app);
            run_app(ui, store, move |cx| {
                let mut checked = use_state(cx, || false);
                let mut entries = log.borrow_mut();
                rsx! {
                    <Checkbox
                        bind={checked.bind()}
                        label="agree"
                        on_change={|v: bool| entries.push(format!("changed to {v}"))}
                    />
                }
                .show(cx);
                drop(entries);
                cx.ui().label(format!("checked: {}", *checked));
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("checked: false").is_some());

    harness.get_by_label("agree").click();
    harness.run();
    assert!(harness.query_by_label("checked: true").is_some());
    assert_eq!(*log.borrow(), ["changed to true"]);
}

#[test]
fn slider_binds_and_reports_change() {
    let log = log();
    let log_in_app = Rc::clone(&log);

    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let log = Rc::clone(&log_in_app);
            run_app(ui, store, move |cx| {
                let mut value = use_state(cx, || 5i32);
                let mut entries = log.borrow_mut();
                rsx! {
                    <Slider
                        bind={value.bind()}
                        range={0..=10}
                        label="amount"
                        on_change={|| entries.push(String::from("change"))}
                    />
                }
                .show(cx);
                drop(entries);
                cx.ui().label(format!("value: {}", *value));
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("value: 5").is_some());

    // Dragging a slider is fiddly headless; focusing it and nudging with the
    // arrow key is the stable way to drive it.
    harness.get_by_role(egui::accesskit::Role::Slider).focus();
    harness.run();
    harness.key_press(egui::Key::ArrowRight);
    harness.run();
    assert!(harness.query_by_label("value: 6").is_some());
    assert!(log.borrow().iter().any(|e| e == "change"));
}

#[test]
fn combo_box_selects_by_index() {
    let log = log();
    let log_in_app = Rc::clone(&log);

    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let log = Rc::clone(&log_in_app);
            run_app(ui, store, move |cx| {
                let mut index = use_state(cx, || 0usize);
                let options = ["alpha", "beta", "gamma"];
                let mut entries = log.borrow_mut();
                rsx! {
                    <ComboBox
                        bind={index.bind()}
                        options={&options}
                        on_change={|i: usize| entries.push(format!("picked {i}"))}
                    />
                }
                .show(cx);
                drop(entries);
                cx.ui().label(format!("index: {}", *index));
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("index: 0").is_some());

    // The closed box carries the selection as its value, not as a label.
    assert_eq!(
        harness
            .get_by_role(egui::accesskit::Role::ComboBox)
            .value()
            .as_deref(),
        Some("alpha")
    );

    // Open the popup, then click an option.
    harness.get_by_role(egui::accesskit::Role::ComboBox).click();
    harness.run();
    harness.get_by_label("gamma").click();
    harness.run();
    assert!(harness.query_by_label("index: 2").is_some());
    assert_eq!(*log.borrow(), ["picked 2"]);
}

#[test]
fn image_draws_without_a_loader_installed() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                rsx! {
                    <Image
                        source={egui::ImageSource::Uri("file://missing.png".into())}
                        fit={egui::vec2(32.0, 32.0)}
                    />
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    // Installing the loader is the application's job (`egui_extras`), so all
    // this checks is that the element compiles and draws egui's placeholder
    // instead of panicking.
    harness.run();
}

/// A taffy leaf is measured from the size it reported the last time it was
/// drawn, and the first draw happens in a zero-width `Ui`. A widget left to
/// wrap reports one character wide there, taffy keeps the node that narrow, and
/// the label ends up written downwards. Every text-bearing widget therefore
/// extends instead of wrapping.
#[test]
fn a_label_in_a_taffy_leaf_stays_on_one_line() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                rsx! {
                    <View direction="row" gap={8}>
                        <Button on_click={|| {}}>"reset"</Button>
                        <Label>"a wide label"</Label>
                        <Checkbox bind={&mut false} label="tick this"/>
                    </View>
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    for label in ["reset", "a wide label", "tick this"] {
        let rect = harness.get_by_label(label).rect();
        assert!(rect.width() > rect.height(), "{label:?} wrapped: {rect:?}",);
    }
}
