//! ARCHITECTURE.md 10, item 4: `use_context` hands out a `Handle`, which can
//! coexist with a `State` guard the provider is holding on another slot, and a
//! write from a child is visible to the parent later in the same pass.

mod common;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use react_egui::prelude::*;

/// A theme value, so the context is keyed by a type of our own.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Theme(String);

fn child(cx: &mut Cx<'_, '_>) {
    let theme = use_context::<Theme>(cx).expect("theme must be provided");
    cx.ui().label(format!("child sees: {}", theme.get().0));
    if cx.ui().button("go dark").clicked() {
        theme.set(Theme(String::from("dark")));
    }
}

fn app(cx: &mut Cx<'_, '_>) {
    // A guard on one slot, held across the whole subtree...
    let mut clicks = use_state(cx, || 0i32);
    // ...and a `Handle` on another, which is what gets provided.
    let theme = use_handle(cx, || Theme(String::from("light")));

    cx.ui().label(format!("clicks: {}", *clicks));
    if cx.ui().button("bump").clicked() {
        *clicks += 1;
    }

    provide_context(cx, theme, child);

    // The parent reads the child's write later in the same pass, while its own
    // guard is still alive.
    cx.ui()
        .label(format!("parent sees: {} ({})", theme.get().0, *clicks));
}

#[test]
fn context_handle_coexists_with_a_parent_guard() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, app);
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("child sees: light").is_some());
    assert!(harness.query_by_label("parent sees: light (0)").is_some());

    // The parent's own guard still works while the context is provided.
    harness.get_by_label("bump").click();
    harness.run();
    assert!(harness.query_by_label("parent sees: light (1)").is_some());

    // One pass: the child writes, the parent reads the new value after the
    // children, the child's own label still shows the old one (5.7).
    harness.get_by_label("go dark").click();
    harness.step();
    assert!(harness.query_by_label("child sees: light").is_some());
    assert!(harness.query_by_label("parent sees: dark (1)").is_some());

    // Next frame everything agrees.
    harness.run();
    assert!(harness.query_by_label("child sees: dark").is_some());
    assert!(harness.query_by_label("parent sees: dark (1)").is_some());
}

#[test]
fn use_context_without_a_provider_is_none() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                let found = use_context::<Theme>(cx).is_some();
                cx.ui().label(format!("found: {found}"));
            });
        },
        Store::new(),
    );
    harness.run();
    assert!(harness.query_by_label("found: false").is_some());
}
