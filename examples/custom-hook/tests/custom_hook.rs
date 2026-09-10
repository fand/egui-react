//! `#[hook]` keys a hook's state by call site, so two callers keep their own.

use custom_hook::App;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::prelude::*;
use egui_reactor_app::{root_id, root_style};

fn harness<'a>() -> Harness<'a, Store> {
    Harness::builder()
        .with_size(egui::vec2(420.0, 620.0))
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

/// Pin egui's clock, so the debounce settles when the test says so and not
/// when the machine happens to be slow.
fn set_time(harness: &mut Harness<'_, Store>, time: f64) {
    harness.input_mut().time = Some(time);
}

/// Type into the `nth` text field on screen: 0 is the search box, 1 the mirror.
fn type_into(harness: &mut Harness<'_, Store>, nth: usize, text: &str) {
    let field = harness
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .nth(nth)
        .expect("a text field");
    field.focus();
    harness.run();
    harness
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .nth(nth)
        .expect("a text field")
        .type_text(text);
    harness.run();
}

#[test]
fn two_callers_of_one_hook_keep_their_own_state() {
    let mut harness = harness();
    set_time(&mut harness, 0.0);
    harness.run();

    assert!(harness.query_by_label("live: ").is_some());
    assert!(harness.query_by_label("mirror live: hello").is_some());

    // Type in the search box only.
    type_into(&mut harness, 0, "abc");
    set_time(&mut harness, 10.0);
    harness.run();

    assert!(harness.query_by_label("live: abc").is_some());
    assert!(harness.query_by_label("settled: abc").is_some());
    // The mirror's own debounce never saw any of that.
    assert!(harness.query_by_label("mirror live: hello").is_some());
    assert!(harness.query_by_label("mirror settled: hello").is_some());
}

#[test]
fn the_debounce_waits_before_it_settles() {
    let mut harness = harness();
    set_time(&mut harness, 0.0);
    harness.run();

    type_into(&mut harness, 0, "ab");
    // Still inside the delay: the live reading has moved, the settled one has
    // not.
    set_time(&mut harness, 0.1);
    harness.step();
    harness.step();
    assert!(harness.query_by_label("live: ab").is_some());
    assert!(harness.query_by_label("settled: ab").is_none());
    assert!(harness.query_by_label("settled: ").is_some());

    // Past the delay.
    set_time(&mut harness, 5.0);
    harness.step();
    harness.step();
    assert!(harness.query_by_label("settled: ab").is_some());
}

#[test]
fn use_previous_reports_the_last_distinct_value() {
    let mut harness = harness();
    set_time(&mut harness, 0.0);
    harness.run();

    assert!(harness.query_by_label("count 0, was -").is_some());

    harness.get_by_label("+").click();
    harness.run();
    assert!(harness.query_by_label("count 1, was 0").is_some());

    harness.get_by_label("+").click();
    harness.run();
    assert!(harness.query_by_label("count 2, was 1").is_some());

    harness.get_by_label("-").click();
    harness.run();
    assert!(harness.query_by_label("count 1, was 2").is_some());
}

/// The same hook again, feeding a layout attribute rather than a label.
#[test]
fn the_window_size_hook_drives_the_layout() {
    let mut harness = harness();
    set_time(&mut harness, 0.0);
    harness.run();

    assert!(harness.query_by_label("window: 420 x 620").is_some());
    assert!(harness.query_by_label("layout: column").is_some());
}
