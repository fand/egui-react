//! `<Text font>`: a known family reaches the galley, an unknown one falls
//! back to the default and warns once, and nothing ever panics.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use common::run_app;
use egui::FontFamily;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;
use egui_react_elements::prelude::*;

/// Recursively find the galley that draws `text` in the last frame's shapes.
fn galley_of(harness: &Harness<'_, Store>, text: &str) -> Arc<egui::Galley> {
    fn walk(shape: &egui::Shape, text: &str) -> Option<Arc<egui::Galley>> {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == text => Some(Arc::clone(&t.galley)),
            egui::Shape::Vec(v) => v.iter().find_map(|s| walk(s, text)),
            _ => None,
        }
    }
    harness
        .output()
        .shapes
        .iter()
        .find_map(|c| walk(&c.shape, text))
        .unwrap_or_else(|| panic!("{text:?} was not painted"))
}

fn family_of(harness: &Harness<'_, Store>, text: &str) -> FontFamily {
    galley_of(harness, text).job.sections[0]
        .format
        .font_id
        .family
        .clone()
}

fn harness_for(app: impl Fn(&mut Cx<'_, '_>) + 'static) -> Harness<'static, Store> {
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            run_app(ui, store, |cx| app(cx));
        },
        Store::new(),
    );
    harness.run();
    harness
}

#[test]
fn monospace_and_proportional_reach_the_galley() {
    let harness = harness_for(|cx| {
        rsx! {
            <View direction="column">
                <Text>"plain"</Text>
                <Text font="monospace">"mono"</Text>
                <Text font="proportional" strong>"prop"</Text>
            </View>
        }
        .show(cx);
    });
    assert_eq!(family_of(&harness, "plain"), FontFamily::Proportional);
    assert_eq!(family_of(&harness, "mono"), FontFamily::Monospace);
    // `strong` makes the text a `RichText`; the family is applied to it too.
    assert_eq!(family_of(&harness, "prop"), FontFamily::Proportional);
}

#[test]
fn an_unknown_name_draws_with_the_default_and_does_not_panic() {
    let harness = harness_for(|cx| {
        rsx! {
            <View direction="column">
                <Text font="nope">"fallback text"</Text>
            </View>
        }
        .show(cx);
    });
    harness.get_by_label("fallback text");
    assert_eq!(
        family_of(&harness, "fallback text"),
        FontFamily::Proportional
    );
}

/// Outside a `<View>` the text is an `egui::Label`; the prop applies there
/// too.
#[test]
fn the_prop_applies_in_ui_mode_as_well() {
    let harness = harness_for(|cx| {
        rsx! {
            <Text font="monospace">"ui mono"</Text>
        }
        .show(cx);
    });
    assert_eq!(family_of(&harness, "ui mono"), FontFamily::Monospace);
}

/// Counts the warnings that mention a name. `log` has one global logger, so
/// the tests in this binary share it and look for their own names.
struct Warnings;

static WARNINGS: Mutex<Vec<String>> = Mutex::new(Vec::new());
static INSTALLED: AtomicUsize = AtomicUsize::new(0);

impl log::Log for Warnings {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            WARNINGS.lock().unwrap().push(record.args().to_string());
        }
    }

    fn flush(&self) {}
}

fn install_logger() {
    if INSTALLED.swap(1, Ordering::SeqCst) == 0 {
        // Another test binary's logger cannot be here: one process, one file.
        log::set_logger(&Warnings).expect("no logger installed before this one");
        log::set_max_level(log::LevelFilter::Warn);
    }
}

#[test]
fn the_warning_fires_once_per_name() {
    install_logger();
    let mut harness = harness_for(|cx| {
        rsx! {
            <View direction="column">
                <Text font="nope-twice">"one"</Text>
                <Text font="nope-twice">"two"</Text>
            </View>
        }
        .show(cx);
    });
    harness.run();
    harness.run();
    let count = WARNINGS
        .lock()
        .unwrap()
        .iter()
        .filter(|m| m.contains("nope-twice"))
        .count();
    assert_eq!(count, 1, "{:?}", WARNINGS.lock().unwrap());
}
