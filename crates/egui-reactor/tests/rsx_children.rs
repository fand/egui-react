//! Plan 2.3 / test 3-3: the four shapes of `children` (none, one literal, one
//! `{expr}`, several nodes), and hooks inside children.
//!
//! Plan 1: the paint shorthands ride in the same `style` prop, so an element
//! that only ever declared `style` gains `bg` and friends for free.

mod common;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::prelude::*;

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

/// Takes the one `style` prop every element takes, and nothing else.
///
/// The paint shorthands land in `style.paint`, so this component never had to
/// change to accept them.
#[component]
fn Painted(cx: &mut Cx, #[prop(default)] style: ItemStyle, children: impl View) {
    let paint = style.paint;
    assert_eq!(paint.bg, Some(egui::Color32::RED));
    assert_eq!(
        paint.border,
        Some(egui::Stroke::new(2.0, egui::Color32::BLACK))
    );
    assert_eq!(paint.radius, Some(4.0));
    assert!(paint.shadow, "a bare `shadow` attribute is `true`");
    assert_eq!(paint.opacity, Some(0.5));
    // The layout half of the same prop is untouched by them.
    assert_eq!(style.p, Some(Length::Px(8.0)));
    cx.ui().label("painted");
    children.show(cx);
}

#[test]
fn the_paint_shorthands_reach_the_style_prop() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                rsx! {
                    <Painted
                        bg={egui::Color32::RED}
                        shadow
                        radius={4.0}
                        border={egui::Stroke::new(2.0, egui::Color32::BLACK)}
                        opacity={0.5}
                        p={8}
                    >
                        "inside the paint"
                    </Painted>
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("painted").is_some());
    assert!(harness.query_by_label("inside the paint").is_some());
}

/// An element written at the call site of a `macro_rules!` whose body is an
/// `rsx!` of its own: `<Leaf/>` here carries the caller's hygiene, and the
/// `cx` it is drawn with has to be the closure's, not the component's.
macro_rules! boxed {
    ($label:literal, $($body:tt)*) => {
        rsx! { <Box key={$label} label={$label}> $($body)* </Box> }
    };
}

#[component]
fn Wrapper(cx: &mut Cx) {
    rsx! {
        <Caption>"outer"</Caption>
        {boxed!("one", <Leaf/>)}
        {boxed!("two", <Leaf/> <Counter name="inner"/>)}
    }
}

#[test]
fn an_element_expanded_through_macro_rules_draws_with_the_closures_cx() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| rsx! { <Wrapper/> }.show(cx));
        },
        Store::new(),
    );
    harness.run();
    assert!(harness.query_by_label("box: one").is_some());
    assert!(harness.query_by_label("box: two").is_some());
    assert_eq!(harness.query_all_by_label("leaf").count(), 2);
    assert!(harness.query_by_label("inner: 0").is_some());
}
