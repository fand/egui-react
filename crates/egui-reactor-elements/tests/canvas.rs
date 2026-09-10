//! Plan 3.3: `<Canvas>` gets its rect from taffy and reports pointer events.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::prelude::*;
use egui_reactor_elements::prelude::*;

/// The rect `paint` was last called with.
type Painted = Rc<Cell<Option<egui::Rect>>>;

/// Draw `app` in a 300x300 window and hand the test its harness.
fn harness_for(app: impl Fn(&mut Cx<'_, '_>) + 'static) -> Harness<'static, Store> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 300.0))
        .build_ui_state(
            move |ui, store: &mut Store| {
                run_app(ui, store, |cx| app(cx));
            },
            Store::new(),
        );
    harness.run();
    harness
}

#[test]
fn the_painted_rect_is_the_size_the_style_asks_for() {
    let painted: Painted = Rc::default();
    let seen = Rc::clone(&painted);
    let _harness = harness_for(move |cx| {
        let seen = Rc::clone(&seen);
        rsx! {
            <View direction="column" w={300.0} h={300.0}>
                <Canvas
                    w={120.0}
                    h={60.0}
                    paint={move |_ui: &mut egui::Ui, rect: egui::Rect| seen.set(Some(rect))}
                />
            </View>
        }
        .show(cx);
    });

    let rect = painted.get().expect("paint should have run");
    assert!(
        (rect.width() - 120.0).abs() < 1.0 && (rect.height() - 60.0).abs() < 1.0,
        "w/h should reach the rect: {rect:?}",
    );
}

#[test]
fn grow_takes_the_space_the_others_leave() {
    let painted: Painted = Rc::default();
    let seen = Rc::clone(&painted);
    let harness = harness_for(move |cx| {
        let seen = Rc::clone(&seen);
        rsx! {
            <View direction="column" w={300.0} h={280.0} gap={0}>
                <Text>"header"</Text>
                <Canvas
                    grow={1.0}
                    paint={move |_ui: &mut egui::Ui, rect: egui::Rect| seen.set(Some(rect))}
                />
            </View>
        }
        .show(cx);
    });

    let rect = painted.get().expect("paint should have run");
    let header = harness.get_by_label("header").rect();
    assert!(
        (rect.width() - 300.0).abs() < 1.0,
        "the column is 300 wide: {rect:?}",
    );
    assert!(
        rect.height() > 280.0 - header.height() - 2.0,
        "grow should take the rest of the 280: {rect:?} under {header:?}",
    );
    assert!(
        rect.top() >= header.bottom() - 1.0,
        "the canvas sits below the header: {rect:?} {header:?}",
    );
}

#[test]
fn dragging_reports_the_delta() {
    let painted: Painted = Rc::default();
    let dragged: Rc<Cell<egui::Vec2>> = Rc::default();
    let seen = Rc::clone(&painted);
    let moved = Rc::clone(&dragged);
    let mut harness = harness_for(move |cx| {
        let seen = Rc::clone(&seen);
        let moved = Rc::clone(&moved);
        rsx! {
            <View direction="column" w={300.0} h={300.0}>
                <Canvas
                    grow={1.0}
                    sense={egui::Sense::drag()}
                    on_drag={|d: egui::Vec2| moved.set(moved.get() + d)}
                    paint={move |_ui: &mut egui::Ui, rect: egui::Rect| seen.set(Some(rect))}
                />
            </View>
        }
        .show(cx);
    });

    let center = painted.get().expect("paint should have run").center();
    harness.hover_at(center);
    harness.drag_at(center);
    harness.run();
    harness.hover_at(center + egui::vec2(30.0, 20.0));
    harness.run();
    harness.drop_at(center + egui::vec2(30.0, 20.0));
    harness.run();

    let delta = dragged.get();
    assert!(
        (delta.x - 30.0).abs() < 1.0 && (delta.y - 20.0).abs() < 1.0,
        "the drag delta should add up to the pointer's move: {delta:?}",
    );
}

#[test]
fn hovering_reports_the_pointer_position() {
    let painted: Painted = Rc::default();
    let hovered: Rc<Cell<Option<egui::Pos2>>> = Rc::default();
    let seen = Rc::clone(&painted);
    let at = Rc::clone(&hovered);
    let mut harness = harness_for(move |cx| {
        let seen = Rc::clone(&seen);
        let at = Rc::clone(&at);
        rsx! {
            <View direction="column" w={300.0} h={300.0}>
                <Canvas
                    grow={1.0}
                    on_hover={|pos: egui::Pos2| at.set(Some(pos))}
                    paint={move |_ui: &mut egui::Ui, rect: egui::Rect| seen.set(Some(rect))}
                />
            </View>
        }
        .show(cx);
    });

    let rect = painted.get().expect("paint should have run");
    assert!(hovered.get().is_none(), "nothing hovered yet");

    let pos = rect.center();
    harness.hover_at(pos);
    harness.run();

    let seen = hovered.get().expect("the pointer is over the canvas");
    assert!(
        (seen - pos).length() < 1.0,
        "the reported position is the pointer's: {seen:?} vs {pos:?}",
    );
    assert!(rect.contains(seen), "and it is inside the rect: {rect:?}");
}
