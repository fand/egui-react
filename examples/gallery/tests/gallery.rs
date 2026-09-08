//! Plan test A-4: picking an example from the list runs it, and a tag narrows
//! the list. And the compact layout: one pane, a floating toggle and a menu.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use egui_react::prelude::*;
use egui_react_app::{root_id, root_style};
use gallery::{App, COMPACT_WIDTH, shown_source};

/// The runner's frame, minus eframe: one pass inside the real root container,
/// so the gallery's three columns are sized the way they are under
/// `egui_react_app::run`.
fn run_app(ui: &mut egui::Ui, store: &mut Store) {
    store.begin_pass(ui.ctx());
    {
        let store: &Store = store;
        let mut cx = Cx::new(store, ui, root_id());
        let view = rsx! { <App/> };
        cx.root_container(root_id(), root_style(), |cx| view.show(cx));
    }
    store.end_pass();
}

/// The window the tests measure against. Tall enough that the whole left
/// column is on screen: a `ScrollArea` culls what it does not draw, and a
/// culled widget is not in the tree to be found.
const WIDTH: f32 = 1280.0;
const HEIGHT: f32 = 1000.0;

fn harness<'a>() -> Harness<'a, Store> {
    Harness::builder()
        .with_size(egui::vec2(WIDTH, HEIGHT))
        .build_ui_state(run_app, Store::new())
}

/// A phone in portrait: well under [`COMPACT_WIDTH`], so the gallery is one
/// pane and a menu.
const PHONE: egui::Vec2 = egui::vec2(390.0, 844.0);
const _: () = assert!(PHONE.x < COMPACT_WIDTH);

fn phone_harness<'a>() -> Harness<'a, Store> {
    Harness::builder()
        .with_size(PHONE)
        .build_ui_state(run_app, Store::new())
}

#[test]
fn selecting_an_example_runs_it() {
    let mut harness = harness();
    harness.run();

    // `showcase` is the first example, and it opens on an empty notebook.
    assert!(harness.query_by_label("no notes").is_some());
    assert!(harness.query_by_label("0 left").is_none());

    harness.get_by_label("todo").click();
    // One pass to apply the write, one to draw the new example.
    harness.run();
    harness.run();

    // The todo example is running, and showcase's hooks went with its scope.
    assert!(harness.query_by_label("0 left").is_some());
    assert!(harness.query_by_label("no notes").is_none());
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

    for name in ["showcase", "counter", "todo", "layout", "fetch"] {
        assert!(harness.query_by_label(name).is_some(), "{name} missing");
    }

    // The chips wrap as a row, they do not shrink to one letter per line.
    let chip = harness.get_by_label("use_future").rect();
    assert!(chip.width() > chip.height(), "the tag wrapped: {chip:?}");

    // `fetch` and `patch` are the two examples that use `use_future`.
    harness.get_by_label("use_future").click();
    harness.run();
    harness.run();

    assert!(harness.query_by_label("fetch").is_some());
    assert!(harness.query_by_label("patch").is_some());
    for name in ["showcase", "counter", "todo", "layout"] {
        assert!(
            harness.query_by_label(name).is_none(),
            "{name} not filtered"
        );
    }
    // Filtering the list does not change what is running.
    assert!(harness.query_by_label("no notes").is_some());

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

/// All three columns fit inside the window, whatever the running example wants.
///
/// `layout` is the widest example, so it is the one that used to push the code
/// column off the right edge: the root node was sized by its content, which
/// left the row with no overflow to shrink away.
#[test]
fn every_column_stays_inside_the_window() {
    let mut harness = harness();
    harness.run();

    harness.get_by_label("layout").click();
    harness.run();
    harness.run();

    let link = harness.get_by_label("source on GitHub").rect();
    assert!(
        link.right() <= WIDTH,
        "the code column ran off the right edge: {link:?}",
    );

    // The code column is the last one, and it starts well right of the list.
    let lines = format!(
        "{} lines",
        shown_source(layout::META.source).lines().count()
    );
    let lines = harness.get_by_label(&lines).rect();
    assert!(
        lines.left() > 700.0,
        "the code column is not where it should be: {lines:?}",
    );
}

/// The egui-react / plain egui toggle: only where there is a plain version,
/// and back to egui-react when another example is picked.
#[test]
fn the_toggle_follows_the_example() {
    let mut harness = harness();
    harness.run();

    // `showcase` has no plain version, so there is no toggle to start with.
    assert!(harness.query_by_label("plain egui").is_none());
    harness.get_by_label("counter").click();
    harness.run();
    harness.run();

    // counter has one, so both buttons are there and egui-react is the one on.
    assert!(toggled(&harness, "egui-react"));
    assert!(!toggled(&harness, "plain egui"));

    harness.get_by_label("plain egui").click();
    harness.run();
    harness.run();
    assert!(toggled(&harness, "plain egui"));
    assert!(!toggled(&harness, "egui-react"));

    // Picking another example starts it on its own egui-react version.
    harness.get_by_label("todo").click();
    harness.run();
    harness.run();
    assert!(toggled(&harness, "egui-react"));

    // An example with no plain version shows no toggle at all. `fetch` is the
    // one, but selecting it here would send a real request, so this checks the
    // data the `if meta.plain.is_some()` branch reads.
    assert!(fetch::META.plain.is_none());
}

/// Whether the version chip with this label is the one on.
///
/// The chips, not any node with the label: the page header is titled
/// `egui-react` too, and a heading has no toggled state.
fn toggled(harness: &Harness<'_, Store>, label: &str) -> bool {
    let chip = harness
        .get_all_by_label(label)
        .find(|node| node.accesskit_node().toggled().is_some())
        .unwrap_or_else(|| panic!("no chip labelled {label:?}"));
    chip.accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)
}

/// The code is one selectable label per line. A drag that starts on one line
/// and ends on another selects across them, and copy joins the lines: egui's
/// `multi_widget_text_select`, which is on by default.
#[test]
fn a_drag_across_code_lines_copies_them() {
    // `patch` rather than the default: it has no plain version, so the code
    // column is the line-count row and then the lines, with no toggle row to
    // work around. It also animates, hence `run_steps` and not `run`.
    let mut harness = Harness::builder()
        .with_size(egui::vec2(WIDTH, HEIGHT))
        .build_ui_state(
            |ui, store: &mut Store| {
                store.begin_pass(ui.ctx());
                {
                    let store: &Store = store;
                    let mut cx = Cx::new(store, ui, root_id());
                    let view = rsx! { <App start="patch"/> };
                    cx.root_container(root_id(), root_style(), |cx| view.show(cx));
                }
                store.end_pass();
            },
            Store::new(),
        );
    harness.run_steps(3);

    // The pane is the right 40% of the window; a Monospace row is 15 points
    // at this style, and the first one starts under the line-count row, past
    // the column's 6-point gap.
    let counted = format!("{} lines", shown_source(patch::META.source).lines().count());
    let top = harness.get_by_label(&counted).rect().bottom() + 6.0;
    let x = WIDTH * 0.6 + 40.0;
    let line = |i: f32| egui::pos2(x, top + 15.0 * i + 7.0);

    harness.hover_at(line(1.0));
    harness.step();
    harness.drag_at(line(1.0));
    harness.step();
    harness.hover_at(line(3.0));
    harness.step();
    harness.step();
    harness.drop_at(line(3.0));
    harness.step();

    harness.input_mut().events.push(egui::Event::Copy);
    harness.step();

    let copied = harness
        .output()
        .platform_output
        .commands
        .iter()
        .find_map(|c| match c {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .expect("nothing was copied");
    // More than one label's text, joined at the line break, and each piece is
    // a piece of what the column shows.
    let shown = shown_source(patch::META.source);
    let lines: Vec<&str> = copied.lines().collect();
    assert!(lines.len() >= 2, "copied: {copied:?}");
    for line in &lines {
        assert!(shown.contains(line), "not from the shown source: {line:?}");
    }
}

/// On a phone there is one pane, and the floating button swaps it for the
/// code and back. The list and the tags are behind the menu button, not on
/// the page.
#[test]
fn a_phone_shows_one_pane_at_a_time() {
    let mut harness = phone_harness();
    harness.run();

    // The header: the title and the menu button. The wide layout has no menu
    // button, and this one has no list on the page.
    assert!(harness.query_by_label("menu").is_some());
    assert!(harness.query_by_label("examples").is_none());
    assert!(harness.query_by_label("use_future").is_none());

    // The example pane is up: showcase is running, and the code is not there.
    assert!(harness.query_by_label("no notes").is_some());
    assert!(harness.query_by_label("source on GitHub").is_none());

    // The toggle floats in the bottom-right corner, inside the window.
    let toggle = harness.get_by_label("show code").rect();
    assert!(
        toggle.right() <= PHONE.x && toggle.bottom() <= PHONE.y,
        "{toggle:?}"
    );
    assert!(
        toggle.left() > PHONE.x / 2.0 && toggle.top() > PHONE.y / 2.0,
        "{toggle:?}"
    );

    harness.get_by_label("show code").click();
    harness.run();
    harness.run();

    // The code pane, the full width of the window. The example is hidden, so
    // nothing it drew is in the tree any more, though it is still running.
    assert!(harness.query_by_label("no notes").is_none());
    let link = harness.get_by_label("source on GitHub").rect();
    assert!(
        link.right() <= PHONE.x,
        "the code pane ran off the right edge: {link:?}"
    );
    let counted = format!(
        "{} lines",
        shown_source(showcase::META.source).lines().count()
    );
    let counted = harness.get_by_label(&counted).rect();
    assert!(
        counted.left() < 40.0,
        "the code pane is not at the left edge: {counted:?}"
    );

    harness.get_by_label("show example").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("no notes").is_some());
    assert!(harness.query_by_label("source on GitHub").is_none());
}

/// A hidden example keeps running, so its state is still there when the code
/// pane is closed again.
///
/// `display="none"` rather than an unmount: what a tab is expected to do.
#[test]
fn the_example_keeps_its_state_across_a_trip_to_the_code_pane() {
    let mut harness = Harness::builder().with_size(PHONE).build_ui_state(
        |ui, store: &mut Store| {
            store.begin_pass(ui.ctx());
            {
                let store: &Store = store;
                let mut cx = Cx::new(store, ui, root_id());
                let view = rsx! { <App start="counter"/> };
                cx.root_container(root_id(), root_style(), |cx| view.show(cx));
            }
            store.end_pass();
        },
        Store::new(),
    );
    harness.run();

    harness.get_by_label("+").click();
    harness.run();
    harness.get_by_label("+").click();
    harness.run();
    assert!(
        harness.query_by_label("2").is_some(),
        "the count should be 2"
    );

    harness.get_by_label("show code").click();
    harness.run();
    harness.run();
    assert!(
        harness.query_by_label("2").is_none(),
        "the example is hidden"
    );

    harness.get_by_label("show example").click();
    harness.run();
    harness.run();
    assert!(
        harness.query_by_label("2").is_some(),
        "the counter should have kept its state"
    );
    assert!(
        harness.query_by_label("0").is_none(),
        "the counter should not have started over"
    );
}

/// The menu slides down over the page, lists every example and every tag,
/// and closes again when an example is picked or its own button is pressed.
#[test]
fn the_menu_opens_over_a_phone_and_closes_on_a_pick() {
    let mut harness = phone_harness();
    harness.run();

    harness.get_by_label("menu").click();
    // One pass to apply the write, then the slide: `run` steps until nothing
    // asks for a repaint, and the animation does until it is down.
    harness.run();
    harness.run();

    // The list and the tags are on the sheet, and the sheet is the whole
    // window: the toggle is under it, so it is not drawn.
    assert!(harness.query_by_label("examples").is_some());
    assert!(harness.query_by_label("use_future").is_some());
    assert!(harness.query_by_label("show code").is_none());
    let heading = harness.get_by_label("examples").rect();
    assert!(heading.top() >= 0.0 && heading.top() < 100.0, "{heading:?}");

    // A tag narrows the list without closing the menu.
    harness.get_by_label("use_future").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("examples").is_some());
    assert!(harness.query_by_label("patch").is_some());
    assert!(harness.query_by_label("todo").is_none());
    harness.get_by_label("clear").click();
    harness.run();
    harness.run();

    // Picking an example runs it and takes the menu away.
    harness.get_by_label("todo").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("0 left").is_some());
    assert!(harness.query_by_label("examples").is_none());
    assert!(harness.query_by_label("show code").is_some());

    // So does the button on the sheet itself.
    harness.get_by_label("menu").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("examples").is_some());
    harness.get_by_label("close menu").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("examples").is_none());
    // And the example under it is still the one that was picked.
    assert!(harness.query_by_label("0 left").is_some());
}

/// While the menu is down, a press lands on the sheet and not on the pane
/// beneath it.
#[test]
fn the_menu_covers_the_pane() {
    let mut harness = phone_harness();
    harness.run();

    // Where showcase's "new" button is with the menu away.
    let new = harness.get_by_label("new").rect().center();

    harness.get_by_label("menu").click();
    harness.run();
    harness.run();

    // A press and a release where the button was.
    harness.hover_at(new);
    harness.step();
    harness.drag_at(new);
    harness.step();
    harness.drop_at(new);
    harness.run();
    harness.run();
    // No note was made: the menu is still down and the notebook is still
    // empty behind it.
    assert!(harness.query_by_label("examples").is_some());
    assert!(harness.query_by_label("no notes").is_some());
}
