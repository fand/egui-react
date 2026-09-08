//! The example renders with its four stacks, starts on the one an app would use
//! on this target, and the bundled one is a family egui can draw Japanese with.

use egui::{FontFamily, FontId};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;
use egui_react_app::fonts::Outcome;
use egui_react_app::{root_id, root_style};
use font::{App, LOADING, SAMPLES, STACKS, fonts, typical};

fn harness<'a>() -> Harness<'a, Store> {
    Harness::builder()
        .with_size(egui::vec2(900.0, 900.0))
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

/// One test rather than several: `font::fonts()` is one `Fonts` per process,
/// and `apply` calls `set_fonts` only when the definitions changed, so a
/// second `Context` in the same process (the tests of one file run on
/// parallel threads) would never be handed them.
#[test]
fn the_example_draws_japanese_with_the_bundled_stack() {
    let mut harness = harness();
    // As `Options::setup` does: before the first frame.
    fonts().apply(&harness.ctx);
    harness.run();

    // Every stack is offered, the samples are drawn, and the report lists every
    // stack. The buttons are matched by role: the matrix under them draws the
    // stack names as plain text too.
    let button = egui::accesskit::Role::Button;
    // The typical stack's button carries the star, and which one that is
    // depends on the target.
    let stack_button = |name: &str| {
        if name == typical() {
            format!("{name} ★")
        } else {
            name.to_string()
        }
    };
    for name in STACKS {
        let label = stack_button(name);
        assert!(
            harness.query_by_role_and_label(button, &label).is_some(),
            "the {label} button is missing"
        );
        let header = format!("stack \"{name}\"");
        assert!(
            harness.query_by_label(&header).is_some(),
            "{header} missing from the report"
        );
    }
    assert!(
        harness
            .query_by_role_and_label(button, &format!("{} ★", typical()))
            .is_some(),
        "no stack is marked as the typical one"
    );
    assert!(
        harness
            .query_by_label("where each stack gets its bytes")
            .is_some(),
        "the target matrix is missing"
    );
    for sample in SAMPLES {
        assert!(harness.query_by_label(sample).is_some(), "{sample} missing");
    }

    // The example opens on the stack an app would use here, not on the first of
    // the list.
    assert!(
        harness
            .query_by_label(&format!("<Text font=\"{}\">", typical()))
            .is_some(),
        "the first frame does not draw with the typical stack"
    );

    // The Japanese checks below are about the bundled subset, so draw with it.
    harness
        .get_by_role_and_label(button, &stack_button("bundled"))
        .click();
    // One pass to apply the write, one to draw with the new value.
    harness.run();
    harness.run();
    assert!(harness.query_by_label("<Text font=\"bundled\">").is_some());

    // The subset was loaded under the family name it declares. The `web`
    // entry is `Pending` or `Failed` here (headless, no server) and the
    // `system` ones depend on the machine, so neither is asserted on.
    let report = fonts().report();
    let bundled = report
        .iter()
        .find(|stack| &*stack.name == "bundled")
        .expect("the bundled stack is reported");
    match &bundled.entries[0].1 {
        Outcome::Loaded { family, .. } => assert_eq!(family, "Noto Sans JP"),
        other => panic!("the bundled subset did not load: {other:?}"),
    }

    // And egui can draw Japanese with it, which it cannot with its own fonts
    // (`FontDefinitions::default()` has no CJK face; see the theme example).
    let has_japanese = |family: FontFamily| {
        harness
            .ctx
            .fonts_mut(|f| f.has_glyphs(&FontId::new(20.0, family), "日本語"))
    };
    assert!(has_japanese(FontFamily::Name("bundled".into())));
    // The monospace default too: the gallery's source pane draws this file in
    // `Monospace`, and the sample strings in it must not come out as boxes.
    assert!(has_japanese(FontFamily::Monospace));
    assert!(has_japanese(FontFamily::Name("code".into())));

    // Picking another stack changes what the samples are drawn with.
    harness
        .get_by_role_and_label(button, &stack_button("web"))
        .click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("<Text font=\"web\">").is_some());
    assert!(harness.query_by_label("<Text font=\"bundled\">").is_none());

    // font-display. `swap` is the default and the samples above were drawn
    // under it. `block` hides them only while a URL is in flight, and here the
    // relative URL has no server, so ehttp fails it within a frame or two;
    // wait that out, then the samples are back and the placeholder is not
    // drawn. (Catching the pending frame is a race in a headless test, so it
    // is left to the browser.)
    harness.get_by_role_and_label(button, "block").click();
    harness.run();
    for _ in 0..200 {
        if !fonts().pending() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        harness.run();
    }
    assert!(!fonts().pending(), "the web URL never finished");
    harness.run();
    assert!(harness.query_by_label(LOADING).is_none());
    for sample in SAMPLES {
        assert!(harness.query_by_label(sample).is_some(), "{sample} missing");
    }
}
