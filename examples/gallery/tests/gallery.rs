//! Plan test A-4: picking an example from the list runs it, and a tag narrows
//! the list.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use gallery::App;
use react_egui::layout::{ContainerStyle, ItemStyle};
use react_egui::prelude::*;

/// The runner's frame: a pass around a root taffy container that reserves the
/// whole area, so the gallery's three columns and their `grow` behave as they
/// do under `react_egui_app::run`.
fn run_app(ui: &mut egui::Ui, store: &mut Store) {
    let root = egui::Id::new("root");
    store.begin_pass(ui.ctx());
    {
        let store: &Store = store;
        let mut cx = Cx::new(store, ui, root);
        let view = rsx! { <App/> };
        let style = ContainerStyle::default()
            .direction("column")
            .merge(&ItemStyle::default());
        cx.root_container(root, style, |cx| view.show(cx));
    }
    store.end_pass();
}

/// Tall enough that the whole left column is on screen: a `ScrollArea` culls
/// what it does not draw, and a culled widget is not in the tree to be found.
fn harness<'a>() -> Harness<'a, Store> {
    Harness::builder()
        .with_size(egui::vec2(1200.0, 1000.0))
        .build_ui_state(run_app, Store::new())
}

#[test]
fn selecting_an_example_runs_it() {
    let mut harness = harness();
    harness.run();

    // The first example is the default, and "reset" is one of its buttons.
    assert!(harness.query_by_label("reset").is_some());
    assert!(harness.query_by_label("0 left").is_none());

    harness.get_by_label("todo").click();
    // One pass to apply the write, one to draw the new example.
    harness.run();
    harness.run();

    // The todo example is running, and counter's hooks went with its scope.
    assert!(harness.query_by_label("0 left").is_some());
    assert!(harness.query_by_label("reset").is_none());
}

/// A taffy leaf is measured from its first draw, which happens in a zero-width
/// `Ui`, so a widget left to wrap reports one character wide and stays that
/// way. The link used to be 9x225 and `align="center"` pushed the whole code
/// header halfway down the column.
#[test]
fn the_source_link_stays_on_one_line() {
    let mut harness = harness();
    harness.run();

    let link = harness.get_by_label("source on GitHub").rect();
    assert!(link.width() > link.height(), "the link wrapped: {link:?}");
}

#[test]
fn a_tag_narrows_the_list() {
    let mut harness = harness();
    harness.run();

    for name in ["counter", "todo", "layout", "fetch"] {
        assert!(harness.query_by_label(name).is_some(), "{name} missing");
    }

    // The chips wrap as a row, they do not shrink to one letter per line.
    let chip = harness.get_by_label("use_future").rect();
    assert!(chip.width() > chip.height(), "the tag wrapped: {chip:?}");

    // Only the fetch example uses `use_future`.
    harness.get_by_label("use_future").click();
    harness.run();
    harness.run();

    assert!(harness.query_by_label("fetch").is_some());
    for name in ["counter", "todo", "layout"] {
        assert!(
            harness.query_by_label(name).is_none(),
            "{name} not filtered"
        );
    }
    // Filtering the list does not change what is running.
    assert!(harness.query_by_label("reset").is_some());

    harness.get_by_label("clear").click();
    harness.run();
    harness.run();

    for name in ["counter", "todo", "layout", "fetch"] {
        assert!(
            harness.query_by_label(name).is_some(),
            "{name} still hidden"
        );
    }
}
