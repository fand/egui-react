//! A `<Text>` is laid out again when the fonts change.
//!
//! The engine caches a `<Text>`'s galley across frames, keyed by the wrap
//! width and the pixels per point. A galley carries texture coordinates into
//! the glyph atlas of the `Fonts` that laid it out, and `Context::set_fonts`
//! builds a new `Fonts` with a new atlas at the start of the next pass. A
//! galley kept across that boundary paints whatever now sits at its old
//! coordinates: on the web, where a font arrives over HTTP after the first
//! frame, every label that did not change its text came out as fragments of
//! other glyphs. So the store notes the fonts once per pass and the cache key
//! carries that generation.

use std::sync::Arc;

use egui_react::prelude::*;
use egui_react::taffy;
use egui_react::taffy::prelude::*;

fn root_style() -> taffy::Style {
    taffy::Style {
        display: taffy::Display::Flex,
        flex_direction: taffy::FlexDirection::Column,
        size: Size {
            width: length(300.0),
            height: length(60.0),
        },
        ..Default::default()
    }
}

/// One frame of a column holding one `<Text>`; the galley it painted.
fn frame(ctx: &egui::Context, store: &mut Store) -> Arc<egui::Galley> {
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        store.begin_pass(ui.ctx());
        {
            let store: &Store = store;
            let mut cx = Cx::new(store, ui, egui::Id::new("root"));
            cx.root_container(egui::Id::new("column"), root_style(), |cx| {
                cx.text(&ItemStyle::default(), "hello".into(), false, None);
            });
        }
        store.end_pass();
    });
    let galley = output
        .shapes
        .iter()
        .find_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) => Some(Arc::clone(&text.galley)),
            _ => None,
        })
        .expect("the text was painted");
    output.drop_without_applying_deltas();
    galley
}

/// `FontDefinitions::default()` with one more font behind `Proportional`:
/// not equal to the current definitions, so `set_fonts` rebuilds.
fn other_fonts() -> egui::FontDefinitions {
    let mut defs = egui::FontDefinitions::default();
    let hack = Arc::clone(&defs.font_data["Hack"]);
    defs.font_data.insert(String::from("Hack-again"), hack);
    defs.families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .push(String::from("Hack-again"));
    defs
}

#[test]
fn the_galley_is_kept_while_the_fonts_stay() {
    let ctx = egui::Context::default();
    let mut store = Store::new();

    let first = frame(&ctx, &mut store);
    let generation = store.fonts_generation();
    let second = frame(&ctx, &mut store);

    assert!(
        Arc::ptr_eq(&first, &second),
        "nothing changed, so the cached galley is painted again"
    );
    assert_eq!(store.fonts_generation(), generation);
}

#[test]
fn the_galley_is_laid_out_again_after_set_fonts() {
    let ctx = egui::Context::default();
    let mut store = Store::new();

    let before = frame(&ctx, &mut store);
    let generation = store.fonts_generation();

    // Takes effect at the start of the next pass, as it does for an app
    // whose font arrived over HTTP between two frames.
    ctx.set_fonts(other_fonts());
    let after = frame(&ctx, &mut store);

    assert_eq!(
        store.fonts_generation(),
        generation + 1,
        "the store noticed the new fonts"
    );
    assert!(
        !Arc::ptr_eq(&before, &after),
        "a galley laid out under the old fonts is not painted with the new atlas"
    );

    // And the new one is cached in turn.
    let again = frame(&ctx, &mut store);
    assert!(Arc::ptr_eq(&after, &again));
}
