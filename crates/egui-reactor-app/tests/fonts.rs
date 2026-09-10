//! `fonts::Fonts` end to end, through egui: a stack applied before the first
//! frame is a `FontFamily::Name` egui can lay text out with, and `<Text font>`
//! draws with it.

use egui::FontFamily;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::prelude::*;
use egui_reactor_app::fonts::{FontSource, Fonts, Generic, Outcome};
use egui_reactor_app::{root_id, root_style};
use egui_reactor_elements::prelude::*;
use epaint_default_fonts::HACK_REGULAR;

#[component]
fn App(cx: &mut Cx) {
    rsx! {
        <View direction="column" gap={8} p={12}>
            <Text font="code">"code text"</Text>
            <Text>"plain text"</Text>
        </View>
    }
}

/// The runner's frame, minus eframe: one pass inside the root container.
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

/// Recursively find the galley that draws `text`.
fn galley_of(
    shapes: &[egui::epaint::ClippedShape],
    text: &str,
) -> Option<std::sync::Arc<egui::Galley>> {
    fn walk(shape: &egui::Shape, text: &str) -> Option<std::sync::Arc<egui::Galley>> {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == text => {
                Some(std::sync::Arc::clone(&t.galley))
            }
            egui::Shape::Vec(v) => v.iter().find_map(|s| walk(s, text)),
            _ => None,
        }
    }
    shapes.iter().find_map(|c| walk(&c.shape, text))
}

#[test]
fn a_bundled_stack_is_a_family_egui_can_use() {
    let fonts = Fonts::new().stack(
        "code",
        [
            FontSource::Bundled(HACK_REGULAR),
            FontSource::Generic(Generic::SansSerif),
        ],
    );
    let mut harness = Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui_state(run_app, Store::new());
    // As `Options::setup` would: before the first frame.
    fonts.apply(&harness.ctx);
    harness.run();

    let code = FontFamily::Name("code".into());
    let list = harness
        .ctx
        .fonts(|f| f.definitions().families.get(&code).cloned())
        .expect("the stack is registered under its name");
    assert_eq!(list[0], "Hack-Regular#0", "{list:?}");
    // The rest of the list, and the two default families it is compared
    // against, are egui's own fonts: nothing to check when they are not in the
    // build (`default-features = false`), where every family here is the
    // bundled Hack alone.
    #[cfg(feature = "default_fonts")]
    {
        assert!(list.contains(&String::from("Ubuntu-Light")), "{list:?}");
        // `FontsView::has_glyphs` is not usable as the check here: epaint
        // answers "no" for any character owned by the same face that supplies
        // the replacement glyph, and Hack has U+FFFD, so `has_glyphs("hello")`
        // is false for every Hack-first family (egui's own `Monospace`
        // included). Glyph widths tell the faces apart instead: Hack's `W` is
        // Hack's `W`.
        let width = |family: FontFamily| {
            harness
                .ctx
                .fonts_mut(|f| f.glyph_width(&egui::FontId::new(14.0, family), 'W'))
        };
        assert_eq!(width(code.clone()), width(FontFamily::Monospace));
        assert_ne!(width(code.clone()), width(FontFamily::Proportional));
    }

    // The report says the same thing.
    let report = fonts.report();
    assert_eq!(&*report[0].name, "code");
    assert_eq!(
        report[0].entries[0].1,
        Outcome::Loaded {
            key: "Hack-Regular#0".into(),
            family: "Hack".into()
        }
    );
    assert_eq!(fonts.generation(), 1);

    // And `<Text font="code">` drew with it, while the plain one did not.
    harness.get_by_label("code text");
    let shapes = &harness.output().shapes;
    let code_galley = galley_of(shapes, "code text").expect("the code text was painted");
    assert_eq!(code_galley.job.sections[0].format.font_id.family, code);
    let plain_galley = galley_of(shapes, "plain text").expect("the plain text was painted");
    assert_eq!(
        plain_galley.job.sections[0].format.font_id.family,
        FontFamily::Proportional
    );
}

#[test]
fn a_default_proportional_stack_changes_plain_text() {
    let fonts = Fonts::new()
        .stack("code", [FontSource::Bundled(HACK_REGULAR)])
        .default_proportional("code");
    let mut harness = Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui_state(run_app, Store::new());
    fonts.apply(&harness.ctx);
    harness.run();

    let proportional = harness
        .ctx
        .fonts(|f| f.definitions().families[&FontFamily::Proportional].clone());
    assert_eq!(proportional[0], "Hack-Regular#0");
    // Applying again with nothing new changes nothing.
    fonts.apply(&harness.ctx);
    harness.run();
    assert_eq!(fonts.generation(), 1);
}
