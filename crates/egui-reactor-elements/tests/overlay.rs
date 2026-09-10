//! Plan 9.4: `<Overlay>` floats in a layer of its own — anchored in a corner,
//! or a sized sheet that paints, takes the presses and roots a taffy tree.
//!
//! An area's first frame is egui's own sizing pass: invisible and not
//! interactable. So every test runs the harness before it reads a rect or
//! presses anything.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::prelude::*;
use egui_reactor_elements::prelude::*;

/// The window every test draws into.
const SIZE: egui::Vec2 = egui::vec2(400.0, 800.0);

/// A counter, so an overlay's children can be checked for their own state.
#[component]
fn Counter(cx: &mut Cx, name: &str) {
    let mut count = use_state(cx, || 0i32);
    cx.ui().label(format!("{name}: {}", *count));
    if cx.ui().button(format!("{name} +")).clicked() {
        *count += 1;
    }
}

/// A 400x800 harness drawing `app`, run once so the areas are past their
/// sizing pass.
fn harness_for(app: impl Fn(&mut Cx<'_, '_>) + 'static) -> Harness<'static, Store> {
    let mut harness = Harness::builder().with_size(SIZE).build_ui_state(
        move |ui, store: &mut Store| {
            run_app(ui, store, |cx| app(cx));
        },
        Store::new(),
    );
    harness.run();
    harness
}

#[test]
fn anchored_bottom_right_stays_in_the_corner() {
    let harness = harness_for(|cx| {
        rsx! {
            <View direction="column" w="100%">
                <Label>"behind"</Label>
                <View h={2000.0}/>
            </View>
            <Overlay anchor="bottom-right" offset={(-16.0, -16.0)}>
                <Button label="fab">"+"</Button>
            </Overlay>
        }
        .show(cx);
    });

    let fab = harness.get_by_label("fab").rect();
    assert!(
        fab.right() <= SIZE.x - 15.0,
        "not at the right edge: {fab:?}"
    );
    assert!(
        fab.bottom() <= SIZE.y - 15.0,
        "not at the bottom edge: {fab:?}"
    );
    // And in the corner, not merely inside the window.
    assert!(fab.left() > 200.0, "too far left: {fab:?}");
    assert!(fab.top() > 400.0, "too far up: {fab:?}");
}

#[test]
fn a_sized_overlay_takes_the_presses() {
    // A sheet over the whole window: the button under it never hears a press.
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);
    let mut covered = harness_for(move |cx| {
        let clicks = Rc::clone(&counted);
        rsx! {
            <Button label="under" on_click={|| clicks.set(clicks.get() + 1)}>"under"</Button>
            <Overlay w="100%" h="100%">
                <Label>"sheet"</Label>
            </Overlay>
        }
        .show(cx);
    });

    // kittest presses at the node's centre, where the sheet's layer is on top.
    covered.get_by_label("under").click();
    covered.run();
    assert_eq!(clicks.get(), 0, "the sheet let a press through");

    // The same app without the sheet: the press lands.
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);
    let mut bare = harness_for(move |cx| {
        let clicks = Rc::clone(&counted);
        rsx! {
            <Button label="under" on_click={|| clicks.set(clicks.get() + 1)}>"under"</Button>
        }
        .show(cx);
    });

    bare.get_by_label("under").click();
    bare.run();
    assert_eq!(clicks.get(), 1, "the bare button should be clickable");
}

#[test]
fn a_sized_overlay_roots_a_tree_that_fills_the_sheet() {
    let harness = harness_for(|cx| {
        rsx! {
            <Overlay w="100%" h="100%">
                <View direction="column" w="100%" h="100%" justify="space-between">
                    <Label>"head"</Label>
                    <Label>"foot"</Label>
                </View>
            </Overlay>
        }
        .show(cx);
    });

    let head = harness.get_by_label("head").rect();
    let foot = harness.get_by_label("foot").rect();
    assert!(head.top() < 30.0, "the head is not at the top: {head:?}");
    assert!(
        foot.bottom() > SIZE.y - 30.0,
        "the foot is not at the bottom: {foot:?}"
    );
}

#[test]
fn an_overlay_not_drawn_unmounts_its_children() {
    let show = Rc::new(Cell::new(true));
    let shown = Rc::clone(&show);
    let mut harness = harness_for(move |cx| {
        let show = shown.get();
        rsx! {
            if show {
                <Overlay anchor="center">
                    <Counter name="sheet"/>
                </Overlay>
            }
        }
        .show(cx);
    });

    assert!(harness.query_by_label("sheet: 0").is_some());
    harness.get_by_label("sheet +").click();
    harness.run();
    assert!(harness.query_by_label("sheet: 1").is_some());
    assert_eq!(harness.state().len(), 1, "one slot: the counter's state");

    // Not drawing the overlay unmounts the children through the usual sweep.
    show.set(false);
    harness.run();
    assert!(harness.query_by_label("sheet: 1").is_none());
    assert_eq!(harness.state().len(), 0, "the counter should be swept");

    // And it comes back fresh.
    show.set(true);
    harness.run();
    assert!(harness.query_by_label("sheet: 0").is_some());
}

#[test]
fn top_keeps_an_overlay_above_a_later_one() {
    /// Two overlays at the same place, each with a button of its own; the one
    /// that hears the press writes its name.
    fn stack(top: bool, pressed: &Rc<Cell<&'static str>>) -> Harness<'static, Store> {
        let pressed = Rc::clone(pressed);
        harness_for(move |cx| {
            let (first, second) = (Rc::clone(&pressed), Rc::clone(&pressed));
            rsx! {
                <Overlay pos={egui::pos2(50.0, 50.0)} w={200.0} h={200.0} top={top}>
                    <Button label="first" on_click={|| first.set("first")}>"first"</Button>
                </Overlay>
                <Overlay pos={egui::pos2(50.0, 50.0)} w={200.0} h={200.0}>
                    <Button label="second" on_click={|| second.set("second")}>"second"</Button>
                </Overlay>
            }
            .show(cx);
        })
    }

    // `top` puts the first overlay back on top every frame.
    let pressed = Rc::new(Cell::new("none"));
    let mut harness = stack(true, &pressed);
    harness.get_by_label("first").click();
    harness.run();
    assert_eq!(pressed.get(), "first");

    // Without it, egui's own rule wins: the area shown later is above.
    let pressed = Rc::new(Cell::new("none"));
    let mut harness = stack(false, &pressed);
    harness.get_by_label("first").click();
    harness.run();
    assert_eq!(pressed.get(), "second");
}
