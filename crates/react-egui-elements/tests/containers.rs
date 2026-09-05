//! Plan 3.3 / test 4-2: the children of every container get a working `Cx`,
//! their state survives across frames, closing a `Window` unmounts them, and
//! `row()` starts a new grid row.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use react_egui::prelude::*;
use react_egui_elements::prelude::*;

/// A counter, so each container's children can be checked for their own state.
#[component]
fn Counter(cx: &mut Cx, name: &str) {
    let mut count = use_state(cx, || 0i32);
    cx.ui().label(format!("{name}: {}", *count));
    if cx.ui().button(format!("{name} +")).clicked() {
        *count += 1;
    }
}

#[test]
fn every_container_hosts_hooks_that_survive_frames() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                rsx! {
                    <ScrollArea max_h={200.0}>
                        <Counter name="scroll"/>
                    </ScrollArea>
                    <Collapsing header="section" default_open>
                        <Counter name="collapsing"/>
                    </Collapsing>
                    <Frame inner_margin={4.0} corner_radius={2.0}>
                        <Counter name="frame"/>
                    </Frame>
                    <Vertical>
                        <Counter name="vertical"/>
                    </Vertical>
                    <Horizontal>
                        <Counter name="horizontal"/>
                    </Horizontal>
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    for name in ["scroll", "collapsing", "frame", "vertical", "horizontal"] {
        assert!(
            harness.query_by_label(&format!("{name}: 0")).is_some(),
            "{name} was not drawn"
        );
    }

    for name in ["scroll", "collapsing", "frame", "vertical", "horizontal"] {
        harness.get_by_label(&format!("{name} +")).click();
        harness.run();
    }
    for name in ["scroll", "collapsing", "frame", "vertical", "horizontal"] {
        assert!(
            harness.query_by_label(&format!("{name}: 1")).is_some(),
            "{name} lost its state"
        );
    }

    // Nothing clicked: every value survives the frame.
    harness.run();
    for name in ["scroll", "collapsing", "frame", "vertical", "horizontal"] {
        assert!(harness.query_by_label(&format!("{name}: 1")).is_some());
    }
}

#[test]
fn a_closed_window_unmounts_its_children() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                let mut open = use_state(cx, || true);
                rsx! {
                    <Window title="dialog" open={open.bind()}>
                        <Counter name="window"/>
                    </Window>
                }
                .show(cx);
                cx.ui().label(format!("open: {}", *open));
                if cx.ui().button("toggle").clicked() {
                    *open = !*open;
                }
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("window: 0").is_some());
    harness.get_by_label("window +").click();
    harness.run();
    assert!(harness.query_by_label("window: 1").is_some());

    // Closing the window stops drawing the children, so the sweep drops them.
    harness.get_by_label("toggle").click();
    harness.run();
    assert!(harness.query_by_label("window: 1").is_none());

    // Reopening starts from the initial value.
    harness.get_by_label("toggle").click();
    harness.run();
    assert!(harness.query_by_label("window: 0").is_some());
}

#[test]
fn panels_host_children_and_keep_their_state() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                rsx! {
                    <Panel side="left" default_size={80.0}>
                        <Counter name="side"/>
                    </Panel>
                    <CentralPanel>
                        <Counter name="central"/>
                    </CentralPanel>
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("side: 0").is_some());
    assert!(harness.query_by_label("central: 0").is_some());

    // The docked panel really did constrain its children to its own width.
    let side = harness.get_by_label("side: 0").rect();
    assert!(
        side.right() < 100.0,
        "the side panel is 80pt wide: {side:?}"
    );

    harness.get_by_label("side +").click();
    harness.run();
    assert!(harness.query_by_label("side: 1").is_some());
    assert!(harness.query_by_label("central: 0").is_some());
}

/// A docked panel carves space out of *its own* `Ui`, and `rsx!` gives every
/// element a `Ui` of its own (`cx.scope` -> `Ui::push_id`). Two panels written
/// as sibling elements therefore stack instead of docking; called without a
/// scope in between they dock the way plain egui does.
#[test]
fn panels_dock_when_they_share_one_ui() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                Panel(
                    cx,
                    react_egui::props_builder(&Panel)
                        .side("left")
                        .default_size(80.0)
                        .children(view(|cx: &mut Cx<'_, '_>| {
                            cx.ui().label("docked side");
                        }))
                        .build(),
                );
                CentralPanel(
                    cx,
                    react_egui::props_builder(&CentralPanel)
                        .children(view(|cx: &mut Cx<'_, '_>| {
                            cx.ui().label("docked central");
                        }))
                        .build(),
                );
            });
        },
        Store::new(),
    );

    harness.run();
    let side = harness.get_by_label("docked side").rect();
    let central = harness.get_by_label("docked central").rect();
    assert!(
        side.right() <= central.left(),
        "the left panel must sit left of the central panel: {side:?} {central:?}"
    );
}

#[test]
fn grid_rows_are_separated_by_row() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                rsx! {
                    <Grid cols={2} striped>
                        <Label>"a1"</Label>
                        <Label>"b1"</Label>
                        {row()}
                        <Label>"a2"</Label>
                        <Label>"b2"</Label>
                    </Grid>
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    let a1 = harness.get_by_label("a1").rect();
    let b1 = harness.get_by_label("b1").rect();
    let a2 = harness.get_by_label("a2").rect();

    assert!(
        a1.left() < b1.left(),
        "a1 and b1 share a row: {a1:?} {b1:?}"
    );
    assert!(
        (a1.top() - b1.top()).abs() < 1.0,
        "a1 and b1 share a row: {a1:?} {b1:?}"
    );
    assert!(a1.top() < a2.top(), "row() starts a new row: {a1:?} {a2:?}");
}

#[test]
fn a_container_inside_a_view_behaves_as_one_leaf() {
    let seen = Rc::new(Cell::new(false));
    let seen_in_app = Rc::clone(&seen);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let seen = Rc::clone(&seen_in_app);
            run_app(ui, store, move |cx| {
                rsx! {
                    <View direction="row" gap={8}>
                        <Text>"before"</Text>
                        <Vertical>
                            <Counter name="inside"/>
                            {::react_egui::view(|cx| {
                                seen.set(!cx.in_taffy());
                            })}
                        </Vertical>
                        <Text>"after"</Text>
                    </View>
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("inside: 0").is_some());
    assert!(
        seen.get(),
        "the children of an egui-native container are drawn in Ui mode"
    );

    let before = harness.get_by_label("before").rect();
    let after = harness.get_by_label("after").rect();
    assert!(
        before.right() < after.left(),
        "the container took one taffy slot between them: {before:?} {after:?}"
    );
}
