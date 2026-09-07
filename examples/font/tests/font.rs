//! The example renders with its three stacks, and the bundled one is a family
//! egui can draw Japanese with.

use egui::{FontFamily, FontId};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;
use egui_react_app::fonts::Outcome;
use egui_react_app::{root_id, root_style};
use font::{App, SAMPLES, STACKS, fonts};

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

    // The three stacks are offered, the samples are drawn, and the report
    // lists every stack.
    for name in STACKS {
        assert!(harness.query_by_label(name).is_some(), "{name} missing");
        let header = format!("stack \"{name}\"");
        assert!(
            harness.query_by_label(&header).is_some(),
            "{header} missing from the report"
        );
    }
    for sample in SAMPLES {
        assert!(harness.query_by_label(sample).is_some(), "{sample} missing");
    }

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
    harness.get_by_label("web").click();
    // One pass to apply the write, one to draw with the new value.
    harness.run();
    harness.run();
    assert!(harness.query_by_label("<Text font=\"web\">").is_some());
    assert!(harness.query_by_label("<Text font=\"bundled\">").is_none());
}
