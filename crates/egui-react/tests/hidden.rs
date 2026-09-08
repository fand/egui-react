//! Plan 9.6: `display="none"` takes a subtree out of the layout without
//! unmounting it.
//!
//! The container's node stays, so keys and child indices do not move and the
//! components inside keep their hooks; taffy lays the subtree out at zero, its
//! leaves draw into an invisible sizing `Ui` under one hidden accesskit node,
//! and a `<Text>` registers nothing at all.
//!
//! Core APIs only (`cx.container`, `cx.leaf`, `cx.text`, `use_state`), as
//! `taffy_cx.rs` does.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use egui_react::prelude::*;

/// The gap between the row's leaves, which is what the hidden node must not add
/// a second time.
const GAP: f32 = 8.0;

/// A row: leaf `a`, a container that is hidden or not, leaf `b`.
///
/// Inside the container: a counter (state, a button, a `<Text>`), a plain
/// `<Text>`, and a leaf that opens a tree of its own.
fn app(cx: &mut Cx<'_, '_>, shown: bool) {
    let row = ContainerStyle::default().direction("row").gap(GAP);
    cx.container(egui::Id::new("row"), &row, &ItemStyle::default(), |cx| {
        cx.scope("a", |cx| {
            cx.leaf(&ItemStyle::default(), |ui| ui.button("a"));
        });

        let inner = ContainerStyle::default()
            .direction("column")
            .display(if shown { "flex" } else { "none" });
        cx.scope("inner", |cx| {
            let id = cx.layout_id();
            cx.container(id, &inner, &ItemStyle::default(), |cx| {
                cx.scope("counter", |cx| {
                    let mut count = use_state(cx, || 0i32);
                    let clicked = cx.leaf(&ItemStyle::default(), |ui| ui.button("hidden +"));
                    if clicked.clicked() {
                        *count += 1;
                    }
                    let label = format!("count: {}", *count);
                    cx.text(&ItemStyle::default(), label.into(), false, Some(false));
                });
                cx.scope("text", |cx| {
                    cx.text(
                        &ItemStyle::default(),
                        "hidden text".into(),
                        false,
                        Some(false),
                    );
                });
                // A leaf that opens a tree of its own: it starts hidden too,
                // through the store's counter.
                cx.scope("nested", |cx| {
                    let (store, scope) = (cx.store, cx.scope_id());
                    cx.leaf(&ItemStyle::default(), move |ui| {
                        let mut cx = Cx::new(store, ui, scope);
                        let column = ContainerStyle::default().direction("column");
                        cx.container(scope.with("tree"), &column, &ItemStyle::default(), |cx| {
                            cx.text(
                                &ItemStyle::default(),
                                "nested text".into(),
                                false,
                                Some(false),
                            );
                        });
                    });
                });
            });
        });

        cx.scope("b", |cx| {
            cx.leaf(&ItemStyle::default(), |ui| ui.button("b"));
        });
    });
}

fn harness_for(shown: &Rc<Cell<bool>>) -> Harness<'static, Store> {
    let shown = Rc::clone(shown);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let shown = shown.get();
            run_app(ui, store, move |cx| app(cx, shown));
        },
        Store::new(),
    );
    harness.run();
    harness
}

#[test]
fn a_hidden_view_takes_no_space_and_reports_no_text() {
    let shown = Rc::new(Cell::new(true));
    let mut harness = harness_for(&shown);

    // Drawn: every text in the subtree is in the tree kittest queries.
    assert!(harness.query_by_label("count: 0").is_some());
    assert!(harness.query_by_label("hidden text").is_some());
    assert!(harness.query_by_label("nested text").is_some());

    // Hidden: one frame to lay the row out again, one to draw it.
    shown.set(false);
    harness.run();
    harness.run();
    assert!(harness.query_by_label("count: 0").is_none());
    assert!(harness.query_by_label("hidden text").is_none());
    assert!(
        harness.query_by_label("nested text").is_none(),
        "a tree opened inside a hidden leaf is hidden too"
    );

    // The row closes over the gap the hidden node used to take.
    let a = harness.get_by_label("a").rect();
    let b = harness.get_by_label("b").rect();
    assert!(
        (b.left() - (a.right() + GAP)).abs() <= 1.0,
        "the hidden node still takes room: {a:?} {b:?}"
    );

    // A hidden widget is still registered by egui (5.8), so what keeps it away
    // from assistive technology is the hidden node its `Ui` hangs from.
    let button = harness.get_by_label("hidden +");
    let parent = button
        .accesskit_node()
        .parent()
        .expect("the leaf's `Ui` node");
    assert!(parent.is_hidden(), "the leaf's node should be hidden");
}

#[test]
fn state_survives_a_hide_and_a_show() {
    let shown = Rc::new(Cell::new(true));
    let mut harness = harness_for(&shown);

    harness.get_by_label("hidden +").click();
    harness.run();
    assert!(harness.query_by_label("count: 1").is_some());
    let slots = harness.state().len();
    assert_eq!(slots, 1, "one slot: the counter's state");

    // Hidden: the component still runs, so its slot is visited and swept by
    // nobody.
    shown.set(false);
    harness.run();
    harness.run();
    assert_eq!(harness.state().len(), slots, "the state was swept away");

    // Pressing where the hidden button reports itself does nothing.
    harness.get_by_label("hidden +").click();
    harness.run();

    shown.set(true);
    harness.run();
    harness.run();
    assert!(
        harness.query_by_label("count: 1").is_some(),
        "the count should come back as it was"
    );
}
