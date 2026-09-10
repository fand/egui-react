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
//! shared image from the egui-reactor side first (`--features snapshot
//! egui_reactor`), or the two tests in a pair race to write the same file.

#![cfg(feature = "snapshot")]

use egui_kittest::kittest::Queryable as _;
use egui_kittest::{Harness, SnapshotOptions};
use egui_reactor::prelude::*;
use egui_reactor_app::{root_id, root_style};

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

/// A egui-reactor view inside the runner's own root container, so the comparison
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
/// because what differs is edges. taffy positions in floats and egui's own
/// layout rounds to whole points, so a shared edge can land a fraction of a
/// point apart: a glyph is rasterised one pixel over, and a filled box has a
/// one-pixel seam down its side. The numbers below are the measured
/// differences with headroom — a real layout regression runs to tens of
/// thousands (the background bug was 110k, and a chip that stretched where it
/// should not was 108k), so they still fail loudly. See plan.md section 7.
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
            fn egui_reactor() {
                let mut harness = react($size, |cx| rsx! { <ExampleApp/> }.show(cx));
                harness.run();
                $drive(&mut harness);
                harness.snapshot_options(stringify!($name), &shared($allowed));
            }

            #[test]
            fn plain_egui() {
                let mut harness = harness($size, plain::PlainState::default(), |ui, state| {
                    // The egui-reactor root reserves the whole area and the panel
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

/// Two notes, so the list, the editor and the word count are all in the
/// picture. An empty notebook would prove little.
fn two_notes<S>(harness: &mut Harness<'_, S>) {
    for _ in 0..2 {
        harness.get_by_label("new").click_accesskit();
        // One pass to apply the write, one to draw with it.
        harness.run();
        harness.run();
    }
}

/// Two clicks of "add sample", so the sparkline has a line to draw.
fn two_samples<S>(harness: &mut Harness<'_, S>) {
    for _ in 0..2 {
        harness.get_by_label("add sample").click();
        // One pass to apply the write, one to draw with it.
        harness.run();
        harness.run();
    }
}

/// One example with no plain version: just a picture of it, so a change to how
/// it draws is noticed.
macro_rules! single {
    ($name:ident, $size:expr) => {
        single!($name, $size, as_it_opens);
    };
    ($name:ident, $size:expr, $drive:path) => {
        mod $name {
            use super::*;
            use ::$name::App as ExampleApp;

            #[test]
            fn egui_reactor() {
                let mut harness = react($size, |cx| rsx! { <ExampleApp/> }.show(cx));
                harness.run();
                $drive(&mut harness);
                harness.snapshot(stringify!($name));
            }
        }
    };
}

single!(notes, egui::vec2(700.0, 460.0), two_notes);
single!(counter, egui::vec2(400.0, 300.0));
same!(todo, egui::vec2(400.0, 400.0), 100, two_items);
same!(form, egui::vec2(420.0, 420.0), 0, edited);
single!(theme, egui::vec2(420.0, 420.0));
// The module is the crate's lib name, so the image is `custom_hook.png`.
single!(custom_hook, egui::vec2(420.0, 620.0));
single!(escape_hatch, egui::vec2(420.0, 620.0), two_samples);
// `shader` has no picture here. Its canvas is a wgpu paint callback, and the
// pipeline behind it is built by `Options::setup` from eframe's render state —
// which `WgpuTestRenderer` does not hand out, so the callback would find no
// `ShaderResources` and draw nothing. On top of that the app asks for a repaint
// every frame, so the picture would never be the same twice. Checked by eye
// instead; see plan.md section 3.

/// The clock, written out rather than through [`single!`], because its picture
/// has to be pinned to a time.
///
/// `App` takes the wall clock as a prop for exactly this. The stopwatch needs
/// nothing: it reads `i.time` and starts stopped, so it shows `00:00.00`
/// however many frames the harness runs.
mod clock {
    use super::*;
    use ::clock::App as ExampleApp;

    #[test]
    fn egui_reactor() {
        let mut harness = react(egui::vec2(420.0, 520.0), |cx| {
            rsx! { <ExampleApp now={12 * 3600 + 34 * 60 + 56}/> }.show(cx)
        });
        harness.run();
        harness.snapshot("clock");
    }
}
/// The two long lists, written out rather than through [`same!`], because both
/// sides need telling how many rows to show. A hundred: ten thousand would look
/// the same and take a second to draw.
///
/// Separate images, unlike the other pairs. The two are the same list, but one
/// draws every row and the other draws the dozen the viewport covers and
/// reserves the rest, and the small differences that follow — where a row sits
/// inside the scrolled area, a point of padding here and there — repeat once
/// per visible row. Closing them would mean writing the plain version to match
/// taffy's arithmetic rather than to be read, which is the line plan.md
/// section 5 draws.
mod list_10k {
    use super::*;
    use ::list_10k::App as ExampleApp;
    use ::list_10k::plain::{self, PlainState};

    const SIZE: egui::Vec2 = egui::vec2(520.0, 420.0);
    const COUNT: usize = 100;

    #[test]
    fn egui_reactor() {
        let mut harness = react(SIZE, |cx| {
            rsx! { <ExampleApp initial_count={COUNT}/> }.show(cx)
        });
        harness.run();
        harness.snapshot("list_10k_react");
    }

    #[test]
    fn plain_egui() {
        let mut harness = harness(SIZE, PlainState::with_count(COUNT), |ui, state| {
            ui.set_min_size(ui.available_size());
            plain::ui(ui, state);
        });
        harness.run();
        harness.snapshot("list_10k_plain");
    }
}

// 2500, where the other pairs need a few hundred: every chip in the tour is
// filled, so each of the ~50 boxes can differ down one edge rather than only
// at its text.
same!(layout, egui::vec2(520.0, 900.0), 2500, as_it_opens);
single!(styles, egui::vec2(560.0, 2900.0));

/// The board, in two images rather than one, for the same reason as
/// [`list_10k`]: the egui-reactor columns are taffy nodes with a `gap`, and the
/// plain ones come from `ui.columns` and `ui.horizontal`, so the two agree on
/// what they draw and disagree by a point or two on where. Closing that would
/// mean writing `plain.rs` to reproduce taffy's arithmetic rather than to be
/// read, which is the line plan.md section 5 draws.
///
/// Neither image is committed yet: this container has no GPU and no software
/// Vulkan, so the first run on a machine that has one writes them
/// (`UPDATE_SNAPSHOTS=1`). See docs/tasks/board/plan.md section 8.
mod board {
    use super::*;
    use ::board::App as ExampleApp;
    use ::board::plain::{self, PlainState};

    const SIZE: egui::Vec2 = egui::vec2(900.0, 560.0);

    #[test]
    fn egui_reactor() {
        let mut harness = react(SIZE, |cx| rsx! { <ExampleApp/> }.show(cx));
        harness.run();
        harness.snapshot("board_react");
    }

    #[test]
    fn plain_egui() {
        let mut harness = harness(SIZE, PlainState::default(), |ui, state| {
            ui.set_min_size(ui.available_size());
            plain::ui(ui, state);
        });
        harness.run();
        harness.snapshot("board_plain");
    }
}
