//! Plan 9.1: `use_animate` is 0 while `on` is false, 1 while it is true, and
//! moves between them over `time` seconds. The value lives in egui's
//! `AnimationManager`, so a step of the harness is a step of the animation.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_react::prelude::*;

/// A frame every twentieth of a second: with `time` 0.5 the value moves by
/// 0.1 per step, so it takes ten steps to go from 0 to 1.
const STEP_DT: f32 = 0.05;

/// A harness whose app writes `use_animate(on, time)` into `value` each frame.
fn animating<'a>(on: &Rc<Cell<bool>>, value: &Rc<Cell<f32>>, time: f32) -> Harness<'a, Store> {
    let on = Rc::clone(on);
    let value = Rc::clone(value);
    Harness::builder().with_step_dt(STEP_DT).build_ui_state(
        move |ui, store: &mut Store| {
            let (on, value) = (Rc::clone(&on), Rc::clone(&value));
            run_app(ui, store, move |cx| {
                let v = use_animate(cx, on.get(), time);
                value.set(v);
                cx.ui().label(format!("v: {v}"));
            });
        },
        Store::new(),
    )
}

#[test]
fn off_is_zero_and_on_rises_to_one_over_time() {
    let on = Rc::new(Cell::new(false));
    let value = Rc::new(Cell::new(-1.0));
    let mut harness = animating(&on, &value, 0.5);

    harness.step();
    assert_eq!(value.get(), 0.0, "an animation that is off sits at zero");

    // The first step after the flip is on the way up, not there yet.
    on.set(true);
    harness.step();
    let first = value.get();
    assert!(0.0 < first && first < 1.0, "mid-animation, got {first}");

    // Collect the rest of the climb: never going back, ending exactly at 1.
    let mut values = vec![first];
    for _ in 0..12 {
        if values[values.len() - 1] == 1.0 {
            break;
        }
        harness.step();
        values.push(value.get());
    }
    for pair in values.windows(2) {
        assert!(pair[0] <= pair[1], "the value went back down: {values:?}");
    }
    assert_eq!(values[values.len() - 1], 1.0, "never arrived: {values:?}");
    assert!(values.len() >= 8, "arrived too fast: {values:?}");

    // And back down the same way.
    on.set(false);
    harness.step();
    let falling = value.get();
    assert!(falling < 1.0, "still at the top, got {falling}");
    for _ in 0..12 {
        harness.step();
    }
    assert_eq!(value.get(), 0.0, "never got back to zero");
}

#[test]
fn zero_time_flips_at_once() {
    let on = Rc::new(Cell::new(false));
    let value = Rc::new(Cell::new(-1.0));
    let mut harness = animating(&on, &value, 0.0);

    harness.step();
    assert_eq!(value.get(), 0.0);

    on.set(true);
    harness.step();
    assert_eq!(value.get(), 1.0, "a zero-second animation has no middle");
}
