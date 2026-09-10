//! Plan 1.1 / test 2-1: every `View` impl draws, and `view(|cx| ..)` infers the
//! closure argument type without an annotation.

mod common;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::prelude::*;

/// Takes anything that is a view, which is how a `children` prop will look.
fn draw(cx: &mut Cx<'_, '_>, child: impl View) {
    child.show(cx);
}

#[test]
fn every_view_impl_draws() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                // `()` draws nothing.
                draw(cx, ());
                draw(cx, "a &str");
                draw(cx, String::from("a String"));
                draw(cx, Some("an Option"));
                draw(cx, Option::<&str>::None);
                draw(cx, vec!["a Vec", "a Vec too"]);
                draw(cx, ["an array", "an array too"]);
                // No `|cx: &mut Cx|` annotation: `view` pins the argument type.
                draw(
                    cx,
                    view(|cx| {
                        cx.ui().label("a closure");
                    }),
                );
                // A bare closure is a view as well, once its type is known.
                let closure = |cx: &mut Cx<'_, '_>| {
                    cx.ui().label("a bare closure");
                };
                draw(cx, closure);
            });
        },
        Store::new(),
    );

    harness.run();
    for label in [
        "a &str",
        "a String",
        "an Option",
        "a Vec",
        "a Vec too",
        "an array",
        "an array too",
        "a closure",
        "a bare closure",
    ] {
        assert!(
            harness.query_by_label(label).is_some(),
            "{label} was not drawn"
        );
    }
}

#[test]
fn nested_views_compose() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                // A view whose body draws further views, the shape `rsx!`
                // children take.
                draw(
                    cx,
                    view(|cx| {
                        "outer".show(cx);
                        view(|cx| {
                            "inner".show(cx);
                        })
                        .show(cx);
                    }),
                );
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("outer").is_some());
    assert!(harness.query_by_label("inner").is_some());
}
