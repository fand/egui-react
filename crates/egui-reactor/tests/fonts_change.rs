//! A `<Text>` is laid out again when the fonts change.
//!
//! A galley carries texture coordinates into the glyph atlas of the `Fonts`
//! that laid it out, and egui throws that `Fonts` away and builds a new one
//! with a new atlas after `set_fonts`, after a change of visuals and when the
//! atlas gets full. A galley kept across that boundary paints whatever now
//! sits at its old coordinates: on the web, where a font arrives over HTTP
//! after the first frame, every label that did not change its text came out as
//! fragments of other glyphs.
//!
//! The rule is that the engine never keeps a galley across passes. Within a
//! pass it reuses the one it laid out; across passes it asks epaint's own
//! `GalleyCache`, which lives inside `Fonts` and dies with the atlas. These
//! tests pin that: while the `Fonts` lives, that cache hands back the very same
//! `Arc`, and a rebuilt atlas is never painted with a galley from the old one.

use std::sync::Arc;

use egui_reactor::prelude::*;
use egui_reactor::taffy;
use egui_reactor::taffy::prelude::*;

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
    let second = frame(&ctx, &mut store);

    assert!(
        Arc::ptr_eq(&first, &second),
        "nothing changed, so epaint's cache hands back the same galley"
    );
}

#[test]
fn the_galley_is_laid_out_again_after_set_fonts() {
    let ctx = egui::Context::default();
    let mut store = Store::new();

    let before = frame(&ctx, &mut store);

    // Takes effect at the start of the next pass, as it does for an app
    // whose font arrived over HTTP between two frames.
    ctx.set_fonts(other_fonts());
    let after = frame(&ctx, &mut store);

    assert!(
        !Arc::ptr_eq(&before, &after),
        "a galley laid out under the old fonts is not painted with the new atlas"
    );

    // And the new one is reused in turn.
    let again = frame(&ctx, &mut store);
    assert!(Arc::ptr_eq(&after, &again));
}

#[test]
fn the_galley_is_laid_out_again_after_a_change_of_visuals() {
    let ctx = egui::Context::default();
    let mut store = Store::new();

    let dark = frame(&ctx, &mut store);

    // `Visuals::light` carries other `TextOptions` than `Visuals::dark`
    // (coverage maps to alpha differently on a light ground), and epaint
    // builds a new atlas for them at the start of the next pass. The font
    // definitions are the same, so nothing about them says so: this is what
    // garbled the `theme` and `showcase` examples on their dark / light
    // switch.
    ctx.set_visuals(egui::Visuals::light());
    let light = frame(&ctx, &mut store);

    assert!(
        !Arc::ptr_eq(&dark, &light),
        "a galley laid out for the dark atlas is not painted with the light one"
    );

    // The same visuals again change nothing.
    ctx.set_visuals(egui::Visuals::light());
    let again = frame(&ctx, &mut store);
    assert!(Arc::ptr_eq(&light, &again));
}
