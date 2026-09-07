//! Plan 1.7 / test 2-7: `cx.container` puts the children into a taffy node and
//! `cx.leaf` makes each of them a taffy leaf, nesting works, and `cx.scope`
//! separates both the hook ids and the egui ids inside a container.

mod common;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;

/// The style `<View direction=".." gap={..}>` will build in phase 4.
fn container_style(direction: &str) -> ContainerStyle {
    ContainerStyle::default().direction(direction).gap(4.0)
}

fn three_leaves(cx: &mut Cx<'_, '_>, direction: &str) {
    cx.container(
        egui::Id::new("row"),
        &container_style(direction),
        &ItemStyle::default(),
        |cx| {
            for label in ["one", "two", "three"] {
                cx.leaf(&ItemStyle::default(), |ui| ui.label(label));
            }
        },
    );
}

fn harness_for(direction: &'static str) -> Harness<'static, Store> {
    Harness::new_ui_state(
        move |ui, store: &mut Store| {
            run_app(ui, store, |cx| three_leaves(cx, direction));
        },
        Store::new(),
    )
}

#[test]
fn a_row_container_lays_leaves_out_horizontally() {
    let mut harness = harness_for("row");
    harness.run();

    let rects: Vec<egui::Rect> = ["one", "two", "three"]
        .iter()
        .map(|l| harness.get_by_label(l).rect())
        .collect();
    assert!(
        rects[0].left() < rects[1].left() && rects[1].left() < rects[2].left(),
        "x must increase: {rects:?}"
    );
    assert!(
        (rects[0].top() - rects[1].top()).abs() < 1.0,
        "a row keeps them on one line: {rects:?}"
    );
}

#[test]
fn a_column_container_lays_leaves_out_vertically() {
    let mut harness = harness_for("column");
    harness.run();

    let rects: Vec<egui::Rect> = ["one", "two", "three"]
        .iter()
        .map(|l| harness.get_by_label(l).rect())
        .collect();
    assert!(
        rects[0].top() < rects[1].top() && rects[1].top() < rects[2].top(),
        "y must increase: {rects:?}"
    );
}

#[test]
fn hooks_work_inside_nested_containers() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                cx.container(
                    egui::Id::new("outer"),
                    &container_style("column"),
                    &ItemStyle::default(),
                    |cx| {
                        let mut outer = use_state(cx, || 0i32);
                        cx.leaf(&ItemStyle::default(), |ui| {
                            if ui.button("outer +").clicked() {
                                *outer += 1;
                            }
                        });
                        cx.leaf(&ItemStyle::default(), |ui| {
                            ui.label(format!("outer: {}", *outer))
                        });

                        cx.container(
                            egui::Id::new("inner"),
                            &container_style("row"),
                            &ItemStyle::default(),
                            |cx| {
                                let mut inner = use_state(cx, || 100i32);
                                cx.leaf(&ItemStyle::default(), |ui| {
                                    if ui.button("inner +").clicked() {
                                        *inner += 1;
                                    }
                                });
                                cx.leaf(&ItemStyle::default(), |ui| {
                                    ui.label(format!("inner: {}", *inner))
                                });
                            },
                        );
                    },
                );
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("outer: 0").is_some());
    assert!(harness.query_by_label("inner: 100").is_some());

    harness.get_by_label("inner +").click();
    harness.run();
    assert!(harness.query_by_label("outer: 0").is_some());
    assert!(harness.query_by_label("inner: 101").is_some());

    harness.get_by_label("outer +").click();
    harness.run();
    assert!(harness.query_by_label("outer: 1").is_some());
    assert!(harness.query_by_label("inner: 101").is_some());
}

/// One subtree, drawn twice inside the same container under two `cx.scope`s.
///
/// Both the hook state (`use_state`) and the egui-side state (the collapsing
/// header) have to end up independent.
fn section(cx: &mut Cx<'_, '_>) {
    let mut count = use_state(cx, || 0i32);
    cx.leaf(&ItemStyle::default(), |ui| {
        if ui.button("bump").clicked() {
            *count += 1;
        }
        ui.collapsing("section", |ui| {
            ui.label("body");
        });
    });
    cx.leaf(&ItemStyle::default(), |ui| {
        ui.label(format!("n = {}", *count))
    });
}

#[test]
fn scope_separates_hook_and_egui_ids_inside_taffy() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                cx.container(
                    egui::Id::new("root"),
                    &container_style("column"),
                    &ItemStyle::default(),
                    |cx| {
                        cx.scope("first", section);
                        cx.scope("second", section);
                    },
                );
            });
        },
        Store::new(),
    );

    harness.run();
    assert_eq!(harness.query_all_by_label("n = 0").count(), 2);

    // The hook state of the first instance moves on its own.
    harness.get_all_by_label("bump").next().unwrap().click();
    harness.run();
    assert_eq!(harness.query_all_by_label("n = 1").count(), 1);
    assert_eq!(harness.query_all_by_label("n = 0").count(), 1);

    // So does the egui-side state of the collapsing header.
    assert_eq!(harness.query_all_by_label("body").count(), 0);
    harness.get_all_by_label("section").next().unwrap().click();
    harness.run();
    assert_eq!(harness.query_all_by_label("body").count(), 1);
}

#[test]
fn scope_inside_taffy_records_no_collision() {
    let seen = std::rc::Rc::new(std::cell::Cell::new(usize::MAX));
    let seen_in_app = std::rc::Rc::clone(&seen);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let seen = std::rc::Rc::clone(&seen_in_app);
            run_app(ui, store, |cx| {
                cx.container(
                    egui::Id::new("root"),
                    &container_style("column"),
                    &ItemStyle::default(),
                    |cx| {
                        cx.scope("first", section);
                        cx.scope("second", section);
                        seen.set(cx.store.collisions().len());
                    },
                );
            });
        },
        Store::new(),
    );

    harness.run();
    assert_eq!(seen.get(), 0);
}
