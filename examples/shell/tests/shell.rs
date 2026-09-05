//! The panels dock, the tree selects, the window toggles, and the log grows.
//!
//! `click_accesskit()` rather than `click()` for anything inside a `<View>`: a
//! simulated pointer press does not reach a widget in a nested taffy tree, and
//! the handler never fires. The file tree is drawn with `cx.leaf` straight into
//! the panel's `Ui`, so a plain `click()` works there.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use react_egui::prelude::*;
use react_egui_app::{root_id, root_style};
use shell::App;

fn harness<'a>() -> Harness<'a, Store> {
    Harness::builder()
        .with_size(egui::vec2(900.0, 600.0))
        .build_ui_state(
            |ui, store: &mut Store| {
                store.begin_pass(ui.ctx());
                {
                    let store: &Store = store;
                    let mut cx = Cx::new(store, ui, root_id());
                    let view = rsx! { <App/> };
                    cx.root_container(root_id(), root_style(), |cx| view.show(cx));
                }
                store.end_pass();
            },
            Store::new(),
        )
}

/// The editor's contents, which is the only multi-line text field on screen.
fn editor(harness: &Harness<'_, Store>) -> String {
    use egui_kittest::kittest::NodeT as _;
    harness
        .get_by_role(egui::accesskit::Role::MultilineTextInput)
        .accesskit_node()
        .value()
        .unwrap_or_default()
}

#[test]
fn picking_a_file_changes_the_editor() {
    let mut harness = harness();
    harness.run();

    assert!(editor(&harness).contains("println!"), "expected main.rs");
    // The path is on screen twice: the toolbar and the open inspector.
    assert_eq!(harness.get_all_by_label("src/main.rs").count(), 2);

    harness.get_by_label("README.md").click();
    // One pass to apply the write, one to draw the new file.
    harness.run();
    harness.run();

    assert!(editor(&harness).contains("An editor-shaped frame"));
    assert_eq!(harness.get_all_by_label("docs/README.md").count(), 2);
    assert!(harness.query_by_label("opened docs/README.md").is_some());
}

#[test]
fn the_inspector_window_toggles() {
    let mut harness = harness();
    harness.run();
    assert!(harness.query_by_label("3 lines").is_some(), "starts open");

    assert!(harness.query_by_label("37 characters").is_some());

    harness.get_by_label("toggle inspector").click_accesskit();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("3 lines").is_none());

    harness.get_by_label("toggle inspector").click_accesskit();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("3 lines").is_some());
}

#[test]
fn save_appends_to_the_log() {
    let mut harness = harness();
    harness.run();
    assert!(harness.query_by_label("saved src/main.rs").is_none());

    harness.get_by_label("save").click_accesskit();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("saved src/main.rs").is_some());
}

/// The panels really do carve the window up, in the order they are written.
#[test]
fn the_panels_dock() {
    let mut harness = harness();
    harness.run();

    let toolbar = harness.get_by_label("save").rect();
    let tree = harness.get_by_label("files").rect();
    let log = harness.get_by_label("log").rect();
    let editor = harness
        .get_by_role(egui::accesskit::Role::MultilineTextInput)
        .rect();

    assert!(toolbar.bottom() <= tree.top(), "top panel above the tree");
    assert!(
        tree.right() <= editor.left(),
        "left panel left of the editor"
    );
    // The log is below the editor's top edge and at the foot of the window.
    // Not below its *bottom* edge: the editor asks for the whole of the
    // central panel and egui clips what does not fit, so its reported rect
    // overhangs the panel by a few points.
    assert!(log.top() > editor.top(), "bottom panel below the editor");
    assert!(log.bottom() > 550.0, "bottom panel at the foot: {log:?}");
}
