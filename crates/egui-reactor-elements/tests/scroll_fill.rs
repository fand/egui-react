//! `<ScrollArea>` inside a `<View>`: it must take the space taffy gives it and
//! follow the window when it resizes.
//!
//! Two regressions found with `examples/layout`. A content-measured leaf pins
//! the scroll area at its first-frame size (it fills what it is given and
//! reports that back), so only the first lines were ever visible. And the
//! `<View>` inside the scroll area is a second taffy tree that needs a third
//! pass to pick up the outer tree's new size after a resize.

use std::cell::Cell;
use std::num::NonZeroUsize;
use std::rc::Rc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::layout::{ContainerStyle, ItemStyle};
use egui_reactor::prelude::*;
use egui_reactor_elements::prelude::*;

#[component]
fn Chip(cx: &mut Cx, label: &str) {
    rsx! { <View p={6}><Text>{label}</Text></View> }
}

/// The shape of `examples/layout`: a growing scroll area around a column of
/// full-width rows. `content_height` records the height the scroll area's
/// content `Ui` was given.
#[component]
fn App(cx: &mut Cx, content_height: Rc<Cell<f32>>) {
    rsx! {
        <ScrollArea grow={1.0}>
            {view(|cx| content_height.set(cx.ui().max_rect().height()))}
            <View direction="column" p={12} w="100%">
                <Text strong>"title"</Text>
                for justify in ["start", "center", "end"] {
                    <View key={justify} direction="row" justify={justify} w="100%" mb={4}>
                        <Chip label="a"/>
                        <Chip label="b"/>
                    </View>
                }
                for i in 0..20 {
                    <Text key={i}>{format!("line {i}")}</Text>
                }
            </View>
        </ScrollArea>
    }
}

/// The runner's frame: `CentralPanel` + a root column that takes all space.
fn frame(ui: &mut egui::Ui, store: &mut Store, content_height: &Rc<Cell<f32>>) {
    egui::CentralPanel::default().show(ui, |ui| {
        store.begin_pass(ui.ctx());
        {
            let store: &Store = store;
            let mut cx = Cx::new(store, ui, egui::Id::new("root"));
            let style = ContainerStyle::default()
                .direction("column")
                .merge(&ItemStyle::default());
            let content_height = Rc::clone(content_height);
            cx.root_container(egui::Id::new("root"), style, |cx| {
                rsx! { <App content_height={content_height}/> }.show(cx)
            });
        }
        store.end_pass();
        let ctx = ui.ctx();
        if ctx.output(|o| o.requested_discard()) && !ctx.will_discard() {
            ctx.request_repaint();
        }
    });
}

fn harness(content_height: &Rc<Cell<f32>>) -> Harness<'static, Store> {
    let content_height = Rc::clone(content_height);
    let harness = Harness::new_ui_state(
        move |ui, store: &mut Store| frame(ui, store, &content_height),
        Store::new(),
    );
    harness
        .ctx
        .options_mut(|o| o.max_passes = NonZeroUsize::new(3).unwrap());
    harness
}

#[test]
fn growing_scroll_area_fills_the_window() {
    let content_height = Rc::new(Cell::new(0.0));
    let mut harness = harness(&content_height);
    harness.set_size(egui::vec2(400.0, 500.0));
    harness.run();

    // The scroll area got (almost) the whole 500px window, not the height of
    // whatever it drew in its first frame.
    assert!(
        content_height.get() > 400.0,
        "scroll area content height {} should fill the window",
        content_height.get()
    );
    // Content past the first lines exists and is laid out below the top.
    let last = harness.get_by_label("line 19").rect();
    assert!(last.top() > 300.0, "line 19 at {last:?}");
}

#[test]
fn nested_taffy_tree_follows_a_resize() {
    let content_height = Rc::new(Cell::new(0.0));
    let mut harness = harness(&content_height);
    // First frames run at kittest's default 800x600, then the window shrinks.
    harness.run();
    harness.set_size(egui::vec2(400.0, 500.0));
    harness.run();

    // The `justify="end"` row must end inside the 400px window: the inner
    // taffy tree re-laid out for the new width.
    let b = harness
        .query_all_by_label("b")
        .map(|n| n.rect())
        .collect::<Vec<_>>();
    assert_eq!(b.len(), 3);
    for rect in &b {
        assert!(
            rect.right() < 400.0,
            "chip at {rect:?} is outside the window"
        );
    }
    // And the rows really are justified: start < center < end.
    assert!(
        b[0].left() < b[1].left() && b[1].left() < b[2].left(),
        "{b:?}"
    );
}
