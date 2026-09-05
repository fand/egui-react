//! Plan test A-3: the two versions of an example put the same thing on screen.
//!
//! Where they agree pixel for pixel, both are compared with **one** image, and
//! a green run is the claim the gallery makes out loud: same UI, different
//! code. Where they do not, each gets its own image and the reason is written
//! down here and in plan.md section 7 — a snapshot that quietly tolerated a
//! real difference would be worth nothing.
//!
//! Behind the `snapshot` feature because it needs a GPU (or a software Vulkan
//! driver) through `egui_kittest`'s wgpu renderer:
//!
//! ```sh
//! cargo test -p gallery --features snapshot
//! ```
//!
//! Regenerate the images with `UPDATE_SNAPSHOTS=1` and commit them. Generate a
//! shared image from the react-egui side first (`--features snapshot
//! react_egui`), or the two tests in a pair race to write the same file.

#![cfg(feature = "snapshot")]

use egui_kittest::kittest::Queryable as _;
use egui_kittest::{Harness, SnapshotOptions};
use react_egui::prelude::*;
use react_egui_app::{root_id, root_style};

fn harness<'a, S: 'a>(
    size: egui::Vec2,
    state: S,
    app: impl FnMut(&mut egui::Ui, &mut S) + 'a,
) -> Harness<'a, S> {
    Harness::builder()
        .with_size(size)
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::default())
        .build_ui_state(app, state)
}

/// A react-egui view inside the runner's own root container, so the comparison
/// measures the examples and not the harness.
fn react<'a>(size: egui::Vec2, view: impl Fn(&mut Cx<'_, '_>) + 'a) -> Harness<'a, Store> {
    harness(size, Store::new(), move |ui, store: &mut Store| {
        store.begin_pass(ui.ctx());
        {
            let store: &Store = store;
            let mut cx = Cx::new(store, ui, root_id());
            cx.root_container(root_id(), root_style(), |cx| view(cx));
        }
        store.end_pass();
    })
}

/// How much of a difference is still the same picture.
///
/// `threshold` is egui_kittest's own default; the allowance is a pixel count,
/// because what differs is glyph edges. taffy positions in floats and egui's
/// own layout rounds to whole points, so a shared edge can land a fraction of
/// a point apart and the text is rasterised one pixel over. The numbers below
/// are the measured differences with headroom — a real layout regression runs
/// to thousands (the background bug was 110k), so they still fail loudly. See
/// plan.md section 7.
fn shared(max_failed_pixels: usize) -> SnapshotOptions {
    SnapshotOptions::default().max_failed_pixels(max_failed_pixels)
}

/// Nothing to do before the picture is taken.
fn as_it_opens<S>(_harness: &mut Harness<'_, S>) {}

/// A name typed in and a box ticked, so the summary line and the log are both
/// in the picture. The defaults alone would prove little.
fn edited<S>(harness: &mut Harness<'_, S>) {
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .focus();
    harness.run();
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .type_text("ada");
    harness.run();

    harness
        .get_all_by_role(egui::accesskit::Role::CheckBox)
        .next()
        .expect("the notify checkbox")
        .click();
    // One pass to apply the write, one to draw the summary and the log.
    harness.run();
    harness.run();
}

/// Two items, the first ticked off, so the list, the counter and the collapsed
/// "done" section are all in the picture. An empty list would prove little.
fn two_items<S>(harness: &mut Harness<'_, S>) {
    for text in ["milk", "eggs"] {
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .focus();
        harness.run();
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .type_text(text);
        harness.run();
        harness.key_press(egui::Key::Enter);
        // One pass to apply the write, one to draw the new list.
        harness.run();
        harness.run();
    }

    harness
        .get_all_by_role(egui::accesskit::Role::CheckBox)
        .next()
        .expect("a checkbox for the first item")
        .click();
    harness.run();
    harness.run();
    // The "done" section stays collapsed in both versions, which is the
    // default on both sides.
}

/// The two versions of one example, compared with one image.
macro_rules! same {
    ($name:ident, $size:expr, $allowed:expr, $drive:path) => {
        mod $name {
            use super::*;
            use ::$name::App as ExampleApp;
            use ::$name::plain;

            #[test]
            fn react_egui() {
                let mut harness = react($size, |cx| rsx! { <ExampleApp/> }.show(cx));
                harness.run();
                $drive(&mut harness);
                harness.snapshot_options(stringify!($name), &shared($allowed));
            }

            #[test]
            fn plain_egui() {
                let mut harness = harness($size, plain::PlainState::default(), |ui, state| {
                    // The react-egui root reserves the whole area and the panel
                    // behind it paints that far. Claim the same space, so the
                    // two images differ in their content rather than in how
                    // much background got painted.
                    ui.set_min_size(ui.available_size());
                    plain::ui(ui, state)
                });
                harness.run();
                $drive(&mut harness);
                // The same name as above: one image, two implementations.
                harness.snapshot_options(stringify!($name), &shared($allowed));
            }
        }
    };
}

/// One example with no plain version: just a picture of it, so a change to how
/// it draws is noticed.
macro_rules! single {
    ($name:ident, $size:expr) => {
        mod $name {
            use super::*;
            use ::$name::App as ExampleApp;

            #[test]
            fn react_egui() {
                let mut harness = react($size, |cx| rsx! { <ExampleApp/> }.show(cx));
                harness.run();
                harness.snapshot(stringify!($name));
            }
        }
    };
}

same!(counter, egui::vec2(400.0, 300.0), 200, as_it_opens);
same!(todo, egui::vec2(400.0, 400.0), 100, two_items);
same!(form, egui::vec2(420.0, 420.0), 0, edited);
single!(theme, egui::vec2(420.0, 420.0));
same!(layout, egui::vec2(520.0, 900.0), 1000, as_it_opens);
