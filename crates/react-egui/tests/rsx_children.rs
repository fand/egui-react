//! Plan 2.3 / test 3-3: the four shapes of `children` (none, one literal, one
//! `{expr}`, several nodes), and hooks inside children.

mod common;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use react_egui::prelude::*;

/// Takes `impl Into<String>`, which only a lone literal or `{expr}` can feed.
#[component]
fn Caption(cx: &mut Cx, children: impl Into<String>) {
    let text: String = children.into();
    cx.ui().label(format!("caption: {text}"));
}

/// Takes a `View`, so it also accepts `()` and several child nodes.
#[component]
fn Box(cx: &mut Cx, label: &str, children: impl View) {
    cx.ui().label(format!("box: {label}"));
    children.show(cx);
}

/// Declares no `children` at all; `#[component]` generates a `()` one.
#[component]
fn Leaf(cx: &mut Cx) {
    cx.ui().label("leaf");
}

#[component]
fn Counter(cx: &mut Cx, name: &str) {
    let mut count = use_state(cx, || 0i32);
    cx.ui().label(format!("{name}: {}", *count));
    if cx.ui().button(format!("{name} +")).clicked() {
        *count += 1;
    }
}

#[test]
fn every_children_shape_compiles_and_draws() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                let owned = String::from("from expr");
                rsx! {
                    <Caption>"from literal"</Caption>
                    <Caption>{owned}</Caption>
                    <Leaf/>
                    <Box label="empty"/>
                    <Box label="many">
                        "child one"
                        <Leaf/>
                        "child two"
                    </Box>
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("caption: from literal").is_some());
    assert!(harness.query_by_label("caption: from expr").is_some());
    assert!(harness.query_by_label("box: empty").is_some());
    assert!(harness.query_by_label("box: many").is_some());
    assert!(harness.query_by_label("child one").is_some());
    assert!(harness.query_by_label("child two").is_some());
    assert_eq!(harness.query_all_by_label("leaf").count(), 2);
}

#[test]
fn hooks_inside_children_keep_their_state() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                rsx! {
                    <Box label="left">
                        <Counter name="a"/>
                    </Box>
                    <Box label="right">
                        <Counter name="b"/>
                    </Box>
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("a: 0").is_some());
    assert!(harness.query_by_label("b: 0").is_some());

    harness.get_by_label("a +").click();
    harness.run();
    harness.get_by_label("a +").click();
    harness.run();
    assert!(harness.query_by_label("a: 2").is_some());
    assert!(harness.query_by_label("b: 0").is_some());

    // Nothing clicked: both survive the frame.
    harness.run();
    assert!(harness.query_by_label("a: 2").is_some());
    assert!(harness.query_by_label("b: 0").is_some());
}
