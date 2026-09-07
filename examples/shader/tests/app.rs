//! Plan test C-2: the sliders move their state, and `pause` stops the
//! repaints.
//!
//! Headless, like every other example test. The `<Canvas>` pushes an
//! `egui_wgpu::Callback` onto the painter, but the harness's default renderer
//! never executes paint callbacks — it only collects the shapes — so no wgpu
//! device is needed here and `gpu::setup` is never called. What is tested is
//! the part that is this crate's own: state reaching the callback, and the
//! repaint request that makes the animation run.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use egui_react::prelude::*;
use egui_react_app::{root_id, root_style};
use shader::App;

fn harness<'a>() -> Harness<'a, Store> {
    Harness::builder()
        .with_size(egui::vec2(420.0, 520.0))
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

/// A slider's own reading. The canvas has no text to check, so the value is
/// read from the accessibility tree rather than from the picture.
///
/// There are two sliders now, so the role alone no longer picks one out;
/// `<Slider label>` becomes the node's name, which does.
fn slider_value(harness: &Harness<'_, Store>, label: &str) -> f64 {
    harness
        .get_by_role_and_label(egui::accesskit::Role::Slider, label)
        .accesskit_node()
        .numeric_value()
        .unwrap_or_else(|| panic!("the {label} slider should report its value"))
}

/// Focus a slider and nudge it right. Dragging is fiddly headless; focusing and
/// nudging is the stable way.
fn nudge(harness: &mut Harness<'_, Store>, label: &str) {
    harness
        .get_by_role_and_label(egui::accesskit::Role::Slider, label)
        .focus();
    harness.step();
    harness.key_press(egui::Key::ArrowRight);
    // One pass to apply the write, one to draw with it.
    harness.step();
    harness.step();
}

#[test]
fn the_slider_moves_the_speed() {
    let mut harness = harness();
    harness.step();
    assert_eq!(slider_value(&harness, "speed"), 1.0);

    nudge(&mut harness, "speed");

    assert!(
        slider_value(&harness, "speed") > 1.0,
        "the slider did not change the speed: {}",
        slider_value(&harness, "speed"),
    );
}

/// The second slider is wired the same way, and moving it must not disturb the
/// first: two `use_state` hooks in one component, each with its own slot.
#[test]
fn the_other_slider_moves_the_mass_and_nothing_else() {
    let mut harness = harness();
    harness.step();
    let before = slider_value(&harness, "mass");

    nudge(&mut harness, "mass");

    assert!(
        slider_value(&harness, "mass") > before,
        "the slider did not change the mass: {}",
        slider_value(&harness, "mass"),
    );
    assert_eq!(
        slider_value(&harness, "speed"),
        1.0,
        "moving the mass slider moved the speed as well",
    );
}

/// The animation is nothing but a repaint request, so pausing is the absence
/// of one.
///
/// `step`, not `run`: a running animation asks for the next frame every frame,
/// and `run` would never decide it had settled.
#[test]
fn pausing_stops_the_repaint_requests() {
    let mut harness = harness();
    harness.step();
    assert!(
        harness.ctx.has_requested_repaint(),
        "the animation should ask for the next frame",
    );

    harness.get_by_label("pause").click();
    // One pass to apply the write, then let egui's own checkbox animation
    // finish; `run_ok` settles it and does not panic if it cannot.
    harness.step();
    harness.run_ok();
    harness.step();

    assert!(
        !harness.ctx.has_requested_repaint(),
        "a paused canvas kept asking for frames",
    );
}

/// The canvas takes the space that is left, not the whole window.
///
/// A `<Canvas>` reports the window as the size it *could* fill, so `grow` on
/// its own makes the column taller than the window and pushes the controls out
/// of sight. `h={0}` next to `grow` is what keeps them on screen.
#[test]
fn the_controls_stay_below_the_canvas() {
    const HEIGHT: f32 = 520.0;

    let mut harness = harness();
    harness.step();

    let slider = harness
        .get_by_role_and_label(egui::accesskit::Role::Slider, "speed")
        .rect();
    let pause = harness.get_by_label("pause").rect();
    assert!(
        slider.bottom() <= HEIGHT && pause.bottom() <= HEIGHT,
        "the canvas pushed the controls off the bottom: {slider:?} {pause:?}",
    );
}
