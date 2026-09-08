//! Plan 3.3 / test 4-2: the children of every container get a working `Cx`,
//! their state survives across frames, closing a `Window` unmounts them, and
//! `<Row>` starts a new grid row.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::run_app;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_react::prelude::*;
use egui_react_elements::prelude::*;

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

/// `#[component(shares_ui)]` is what makes docking work: a panel carves space
/// out of the `Ui` its siblings are drawn into, so it must not get a child `Ui`
/// of its own from `cx.scope`.
#[test]
fn panels_written_as_siblings_dock() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                rsx! {
                    <Panel side="left" default_size={80.0}>
                        <Label>"docked side"</Label>
                    </Panel>
                    <CentralPanel>
                        <Label>"docked central"</Label>
                    </CentralPanel>
                }
                .show(cx);
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
                        <Row>
                            <Label>"a1"</Label>
                            <Label>"b1"</Label>
                        </Row>
                        <Row>
                            <Label>"a2"</Label>
                            <Label>"b2"</Label>
                        </Row>
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
    assert!(a1.top() < a2.top(), "<Row> starts a new row: {a1:?} {a2:?}");
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
                            {::egui_react::view(|cx| {
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

/// A panel docks in the `Ui` the taffy tree was started in, not in a node of
/// the tree, so `<Panel>` inside a `<View>` reaches the window's edge instead
/// of taking a slice of the row it was written in.
///
/// Without that, four sibling panels each carve a little node of their own and
/// all draw at the same corner.
#[test]
fn a_panel_inside_a_view_docks_in_the_window() {
    let size = egui::vec2(400.0, 300.0);
    let mut harness = Harness::builder().with_size(size).build_ui_state(
        |ui, store: &mut Store| {
            let root = egui::Id::new("root");
            store.begin_pass(ui.ctx());
            {
                let store: &Store = store;
                let mut cx = Cx::new(store, ui, root);
                let style = ContainerStyle::default()
                    .direction("column")
                    .merge(&ItemStyle::default().w("100%").min_h("100%"));
                let view = rsx! {
                    <View direction="column" grow={1.0}>
                        <Panel side="left" default_size={80.0}>
                            <Label>"in the panel"</Label>
                        </Panel>
                        <CentralPanel>
                            <Label>"the rest"</Label>
                        </CentralPanel>
                    </View>
                };
                cx.root_container(root, style, |cx| view.show(cx));
            }
            store.end_pass();
        },
        Store::new(),
    );

    harness.run();
    let panel = harness.get_by_label("in the panel").rect();
    let rest = harness.get_by_label("the rest").rect();

    assert!(
        panel.left() < 24.0,
        "the panel should reach the window's left edge: {panel:?}",
    );
    assert!(
        panel.right() <= rest.left(),
        "the panel and the rest must not overlap: {panel:?} {rest:?}",
    );
}

#[test]
fn a_shadowed_frame_lays_out_like_a_plain_one() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                rsx! {
                    <View direction="column">
                        <Frame inner_margin={4.0}>
                            <Label>"framed"</Label>
                        </Frame>
                        <Frame shadow inner_margin={4.0}>
                            <Label>"framed"</Label>
                        </Frame>
                        <Frame custom_shadow={egui::Shadow::NONE} inner_margin={4.0}>
                            <Counter name="custom"/>
                        </Frame>
                    </View>
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    // The same text in both frames, so only the frames can move it: the labels
    // come back in tree order, the plain one first.
    let labels: Vec<_> = harness
        .get_all_by_label("framed")
        .map(|node| node.rect())
        .collect();
    assert_eq!(labels.len(), 2, "both frames should draw their label");
    let (plain, shadowed) = (labels[0], labels[1]);

    // A shadow is painted, not laid out: the two labels are the same size, at
    // the same left edge, one under the other.
    assert_eq!(plain.size(), shadowed.size(), "{plain:?} {shadowed:?}");
    assert_eq!(plain.left(), shadowed.left(), "{plain:?} {shadowed:?}");
    assert!(
        plain.bottom() <= shadowed.top(),
        "the frames should stack: {plain:?} {shadowed:?}",
    );

    // And the third frame's children keep their state.
    assert!(harness.query_by_label("custom: 0").is_some());
    harness.get_by_label("custom +").click();
    harness.run();
    assert!(harness.query_by_label("custom: 1").is_some());
}
