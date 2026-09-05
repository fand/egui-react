//! The controls narrow the list, in both versions.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use list_10k::App;
use list_10k::plain::{self, PlainState};
use react_egui::prelude::*;
use react_egui_app::{root_id, root_style};

const SIZE: egui::Vec2 = egui::vec2(520.0, 520.0);
/// Small enough that a test frame is quick; the slider still reaches 10k.
const COUNT: usize = 100;

fn react<'a>() -> Harness<'a, Store> {
    react_with(false)
}

/// The same list, drawn by `<VirtualList>` instead of `<ScrollArea>` + `for`.
fn virtualised<'a>() -> Harness<'a, Store> {
    react_with(true)
}

fn react_with<'a>(virtualise: bool) -> Harness<'a, Store> {
    Harness::builder().with_size(SIZE).build_ui_state(
        move |ui, store: &mut Store| {
            store.begin_pass(ui.ctx());
            {
                let store: &Store = store;
                let mut cx = Cx::new(store, ui, root_id());
                let view = rsx! { <App initial_count={COUNT} virtualise={virtualise}/> };
                cx.root_container(root_id(), root_style(), |cx| view.show(cx));
            }
            store.end_pass();
        },
        Store::new(),
    )
}

fn plain<'a>() -> Harness<'a, PlainState> {
    Harness::builder().with_size(SIZE).build_ui_state(
        |ui, state: &mut PlainState| plain::ui(ui, state),
        PlainState::with_count(COUNT),
    )
}

/// "alpha" is every eighth row, so 100 rows leave 13 of them.
fn filter_to_alpha<S>(harness: &mut Harness<'_, S>) {
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .focus();
    harness.run();
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .type_text("alpha");
    harness.run();
    harness.run();
}

#[test]
fn the_filter_narrows_the_react_egui_list() {
    let mut harness = react();
    harness.run();
    assert!(harness.query_by_label("showing 100").is_some());

    filter_to_alpha(&mut harness);
    assert!(harness.query_by_label("showing 13").is_some());
}

#[test]
fn the_filter_narrows_the_plain_egui_list_the_same() {
    let mut harness = plain();
    harness.run();
    assert!(harness.query_by_label("showing 100").is_some());

    filter_to_alpha(&mut harness);
    assert!(harness.query_by_label("showing 13").is_some());
}

/// Removing a row takes one off the count. Only the react-egui version is
/// driven here: the plain version's "x" is the same three lines, and its rows
/// are virtualised, so which one is on screen depends on the scroll offset.
#[test]
fn removing_a_row_shortens_the_list() {
    let mut harness = react();
    harness.run();

    // `click_accesskit`, not `click`: a simulated pointer press inside a
    // `ScrollArea` is swallowed before it reaches the row, and the button never
    // fires. Asking accessibility to activate it works.
    harness
        .get_all_by_label("x")
        .next()
        .expect("a remove button")
        .click_accesskit();
    // One pass to apply the write, one to rebuild the list.
    harness.run();
    harness.run();

    assert!(harness.query_by_label("showing 99").is_some());
}

/// The slider is the row count, and the count label follows it.
#[test]
fn the_slider_sets_the_row_count() {
    let mut harness = react();
    harness.run();
    assert!(harness.query_by_label("showing 100").is_some());

    harness.get_by_role(egui::accesskit::Role::Slider).focus();
    harness.run();
    harness.key_press(egui::Key::ArrowRight);
    harness.run();
    harness.run();

    assert!(
        harness.query_by_label("showing 100").is_none(),
        "the list did not follow the slider",
    );
}

/// The switch changes how the rows are drawn and nothing else: the filter still
/// narrows the list and "x" still removes a row.
#[test]
fn virtualising_keeps_the_behaviour() {
    let mut harness = virtualised();
    harness.run();
    assert!(harness.query_by_label("showing 100").is_some());
    // Only the rows in view exist, so the last one is not in the tree.
    assert!(harness.query_by_label("#0").is_some());
    assert!(harness.query_by_label("#99").is_none());

    filter_to_alpha(&mut harness);
    assert!(harness.query_by_label("showing 13").is_some());

    harness
        .get_all_by_label("x")
        .next()
        .expect("a remove button")
        .click_accesskit();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("showing 12").is_some());
}

/// The two ways of drawing show the same rows at the top of the list.
#[test]
fn both_ways_show_the_same_first_rows() {
    let mut plain_scroll = react();
    plain_scroll.run();
    let mut virtual_scroll = virtualised();
    virtual_scroll.run();

    for i in 0..10 {
        let label = format!("row {i} {}", list_10k::WORDS[i % list_10k::WORDS.len()]);
        assert!(plain_scroll.query_by_label(&label).is_some(), "{label}");
        assert!(virtual_scroll.query_by_label(&label).is_some(), "{label}");
    }
}
