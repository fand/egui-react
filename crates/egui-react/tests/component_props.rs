//! Plan 2.1 / test 3-4: optional props, `#[prop(default = ..)]`,
//! `#[prop(into)]`, borrowed props and props with generics.

mod common;

use std::fmt::Display;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;

/// `Option<T>` is optional without an attribute and its setter takes the inner
/// value; `#[prop(default = ..)]` sets a value; `#[prop(into)]` makes the setter
/// take `impl Into<T>`.
#[component]
fn Field(
    cx: &mut Cx,
    label: &str,
    hint: Option<&str>,
    #[prop(default = 3)] rows: usize,
    #[prop(default)] disabled: bool,
    #[prop(into)] value: String,
) {
    cx.ui().label(format!(
        "{label}/{}/{rows}/{disabled}/{value}",
        hint.unwrap_or("-")
    ));
}

/// A generic prop, plus a generic that only appears behind a reference.
#[component]
fn Show<T: Display>(cx: &mut Cx, value: T, items: &[T]) {
    let joined: Vec<String> = items.iter().map(|i| i.to_string()).collect();
    cx.ui()
        .label(format!("show {value} [{}]", joined.join(",")));
}

#[test]
fn optional_and_defaulted_props() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                rsx! {
                    // Everything optional left out; `value` takes a `&str`
                    // thanks to `#[prop(into)]`.
                    <Field label="plain" value="v"/>
                    // Everything spelled out.
                    <Field
                        label="full"
                        hint="h"
                        rows={9}
                        disabled
                        value={String::from("w")}
                    />
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("plain/-/3/false/v").is_some());
    assert!(harness.query_by_label("full/h/9/true/w").is_some());
}

#[test]
fn generic_props() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                let numbers = [1, 2, 3];
                let words = ["a", "b"];
                rsx! {
                    <Show value={7} items={&numbers}/>
                    <Show value={"x"} items={&words}/>
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("show 7 [1,2,3]").is_some());
    assert!(harness.query_by_label("show x [a,b]").is_some());
}
