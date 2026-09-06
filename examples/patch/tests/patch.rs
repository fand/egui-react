//! Plan tests P-1 .. P-6: what the editor does to the shader, and what it does
//! not do.
//!
//! P-5 is the one the example exists for. Everything is driven through the UI —
//! the palette's buttons, a drag from one port circle to another, the slider in
//! the inspector — and read back out of the `wgsl` tab, which shows exactly the
//! text that was compiled. "The reducer bumped the right counter" is a unit
//! test in `graph.rs`; the claim here is that the screen behaves accordingly.
//!
//! Headless, like every other example test. The preview's `<Canvas>` pushes an
//! `egui_wgpu::Callback` that the harness's renderer never executes, so no GPU
//! is needed and `gpu::setup` is never called.

use std::cell::Cell;
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use patch::App;
use react_egui::prelude::*;
use react_egui_app::{root_id, root_style};

/// Wide enough for the three columns, tall enough for the whole preset.
///
/// The canvas clips its nodes, and a clipped port cannot be dragged, so this
/// also has to leave room for the node the palette drops to the right of the
/// preset.
const SIZE: egui::Vec2 = egui::vec2(1320.0, 820.0);

/// The runner's frame, minus eframe.
fn run_app(ui: &mut egui::Ui, store: &mut Store) {
    store.begin_pass(ui.ctx());
    {
        let store: &Store = store;
        let mut cx = Cx::new(store, ui, root_id());
        let view = rsx! { <App/> };
        cx.root_container(root_id(), root_style(), |cx| view.show(cx));
    }
    store.end_pass();
}

fn harness() -> Harness<'static, Store> {
    Harness::builder()
        .with_size(SIZE)
        .build_ui_state(run_app, Store::new())
}

/// A few passes: one to apply a write, one to draw with it, and one more for
/// the taffy tree inside a node to settle.
fn settle(harness: &mut Harness<'static, Store>) {
    for _ in 0..4 {
        harness.step();
    }
}

/// Step until `ready` is true, for at most two seconds.
///
/// The preset is parsed on a thread of its own (that is what `use_future`
/// does), so how many frames it takes is not this test's business.
fn wait_for(
    harness: &mut Harness<'static, Store>,
    what: &str,
    ready: fn(&Harness<'static, Store>) -> bool,
) {
    for _ in 0..200 {
        harness.step();
        if ready(harness) {
            return;
        }
        sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {what}");
}

/// A harness with the preset loaded and the preview paused.
///
/// Pausing matters: a running preview asks for a repaint on every frame, which
/// is what an animation is, and a test that waited for the app to go idle
/// would wait forever.
fn loaded() -> Harness<'static, Store> {
    let mut harness = harness();
    wait_for(&mut harness, "the preset", |harness| {
        harness.query_by_label("play").is_some()
    });
    harness.get_by_label("play").click();
    settle(&mut harness);
    harness
}

/// Add a node from the palette.
///
/// By role as well as label: a node of that kind writes its kind in its own
/// box, and that is a label too.
fn add(harness: &mut Harness<'static, Store>, kind: &str) {
    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, kind)
        .click();
    settle(harness);
}

fn click(harness: &mut Harness<'static, Store>, label: &str) {
    harness.get_by_label(label).click();
    settle(harness);
}

/// Drag from one port circle to another, as a pointer does it: press, move
/// (which is what makes egui call it a drag), move again (the frame that
/// offers the slot under the pointer to the drag session), release.
fn wire(harness: &mut Harness<'static, Store>, from: &str, to: &str) {
    let from = harness.get_by_label(from).rect().center();
    let to = harness.get_by_label(to).rect().center();
    harness.hover_at(from);
    harness.step();
    harness.drag_at(from);
    harness.step();
    harness.hover_at(to);
    harness.step();
    harness.hover_at(to);
    harness.step();
    harness.drop_at(to);
    settle(harness);
}

/// The program the `wgsl` tab is showing.
///
/// An egui `Label` keeps its text in the accessibility node's *value*, which is
/// also how `kittest` matches one by label.
fn wgsl(harness: &Harness<'static, Store>) -> String {
    harness
        .get_by_label_contains("fn fs_main")
        .accesskit_node()
        .value()
        .expect("the wgsl tab shows the program")
}

/// Open the `wgsl` tab and read it, then leave the tab as it was found.
fn program(harness: &mut Harness<'static, Store>) -> String {
    click(harness, "wgsl");
    let source = wgsl(harness);
    click(harness, "params");
    source
}

/// P-1: the palette adds a node, and the node is on the canvas.
#[test]
fn the_palette_adds_a_node() {
    let mut harness = loaded();

    assert!(
        harness.query_by_label("7 / 32 nodes").is_some(),
        "the preset"
    );
    assert!(harness.query_by_label("hsv1").is_none());

    add(&mut harness, "hsv");

    assert!(harness.query_by_label("8 / 32 nodes").is_some());
    let node = harness
        .query_by_label("hsv1")
        .expect("the new node is on the canvas")
        .rect();
    assert!(
        node.min.x > 0.0 && node.max.x < SIZE.x,
        "on screen: {node:?}"
    );

    // Clicking its title selects it, and the inspector follows.
    harness.get_by_label("hsv1").click();
    settle(&mut harness);
    // By role as well as label: an `egui::Slider` is a slider *and* the drag
    // value that shows its reading, and both carry the name.
    assert!(
        harness
            .query_by_role_and_label(egui::accesskit::Role::Slider, "hue")
            .is_some(),
        "the inspector shows the selected node's parameters"
    );
}

/// P-2: wiring a node to the output puts it in the program.
#[test]
fn wiring_a_node_puts_it_in_the_shader() {
    let mut harness = loaded();
    add(&mut harness, "hsv");

    let before = program(&mut harness);
    assert!(
        !before.contains("fn n8("),
        "the new node is not wired up yet"
    );

    // hsv1 (node 8) between the mix and the output.
    wire(&mut harness, "mix1 out", "hsv1 in 0");
    wire(&mut harness, "hsv1 out", "out1 in 0");

    let after = program(&mut harness);
    assert!(
        after.contains("fn n8(uv"),
        "the node has a function:\n{after}"
    );
    assert!(
        after.contains("return n8(uv);"),
        "and the output calls it:\n{after}"
    );
    assert!(
        after.contains("let src = n7(uv);"),
        "fed by the mix:\n{after}"
    );
}

/// P-3: a node's own state stays with the node.
///
/// The same claim as `board`'s B-2, in a deeper tree: here the state is inside
/// a component, inside a `Cx` built around a child `Ui`, inside the canvas
/// leaf. What keys it is the `cx.scope(node.id, ..)` the canvas writes by hand.
///
/// Being collapsed is that state. Nothing outside a node knows about it, and
/// the patch itself does not record it, so it can only have come from the
/// node's own hooks.
#[test]
fn node_state_survives_its_neighbours() {
    let mut harness = loaded();

    // A collapsed node draws no body, and its delete button is in that body.
    let collapsed = |harness: &Harness<'static, Store>, name: &str| {
        harness
            .query_by_label(format!("delete {name}").as_str())
            .is_none()
    };

    click(&mut harness, "collapse level1");
    click(&mut harness, "collapse transform1");
    assert!(collapsed(&harness, "level1"));
    assert!(collapsed(&harness, "transform1"));

    // A third node is deleted, which unplugs it and rewires nothing else.
    click(&mut harness, "delete grayscale1");
    assert!(harness.query_by_label("grayscale1").is_none());
    assert!(
        collapsed(&harness, "level1") && collapsed(&harness, "transform1"),
        "deleting a neighbour left both of them collapsed"
    );

    // Dragging a node raises it to the end of the list, which is a different
    // position in `graph.nodes` and the same node.
    let at = harness.get_by_label("shader1").rect().center();
    harness.hover_at(at);
    harness.step();
    harness.drag_at(at);
    harness.step();
    harness.hover_at(at + egui::vec2(0.0, 40.0));
    harness.step();
    harness.drop_at(at + egui::vec2(0.0, 40.0));
    settle(&mut harness);

    assert!(
        collapsed(&harness, "level1") && collapsed(&harness, "transform1"),
        "reordering the nodes did not move anyone's state"
    );
    // And the node that moved is still there, still open.
    assert!(harness.query_by_label("delete shader1").is_some());
}

/// P-4: a cycle is a message, and the last program that worked stays.
#[test]
fn a_cycle_keeps_the_last_good_program() {
    let mut harness = loaded();
    let before = program(&mut harness);

    // level1 already feeds the mix through transform1; feeding it back into
    // transform1 closes the loop.
    wire(&mut harness, "level1 out", "transform1 in 0");

    assert!(
        harness.query_by_label_contains("a cycle").is_some(),
        "the inspector says what is wrong"
    );
    assert_eq!(
        program(&mut harness),
        before,
        "and the program on the GPU is the one that still works"
    );

    // Unplugging the offending wire brings the picture back.
    click(&mut harness, "transform1 in 0");
    assert!(harness.query_by_label_contains("a cycle").is_none());
}

/// P-5, the one this example is for: a parameter is not part of the program.
#[test]
fn a_slider_does_not_recompile_and_a_wire_does() {
    let mut harness = loaded();

    harness.get_by_label("level1").click();
    settle(&mut harness);
    let before = program(&mut harness);

    // The inspector's sliders are the wide version of the same components the
    // node draws; "bright" is `level1`'s first parameter.
    let slider = harness.get_by_role_and_label(egui::accesskit::Role::Slider, "bright");
    let was = slider.accesskit_node().numeric_value();
    slider.focus();
    harness.step();
    harness.key_press(egui::Key::ArrowRight);
    settle(&mut harness);

    let now = harness
        .get_by_role_and_label(egui::accesskit::Role::Slider, "bright")
        .accesskit_node()
        .numeric_value();
    assert!(now > was, "the slider moved: {was:?} -> {now:?}");
    assert_eq!(
        program(&mut harness),
        before,
        "and not one character of the shader changed"
    );

    // A wire, on the other hand, is a different program.
    wire(&mut harness, "shader2 out", "level1 in 0");
    let after = program(&mut harness);
    assert_ne!(after, before, "rewiring regenerated the shader");
    assert!(
        after.contains("let src = n5(uv);"),
        "level1 now reads shader2:\n{after}"
    );
}

/// Editing a `Shader` node's expression regenerates the program, and a broken
/// one is a message rather than a broken picture.
///
/// This is the other half of P-4: a cycle is the editor's mistake to catch, and
/// an expression that is not WGSL is naga's. Both end up as text in the
/// inspector, and neither reaches a device.
#[test]
fn editing_an_expression_recompiles_it() {
    let mut harness = loaded();
    harness.get_by_label("shader1").click();
    settle(&mut harness);
    let before = program(&mut harness);

    source_field(&harness).focus();
    harness.step();
    source_field(&harness).type_text(" ");
    settle(&mut harness);

    let edited = program(&mut harness);
    assert_ne!(edited, before, "the expression is part of the program");
    assert!(
        harness.query_by_label_contains("error").is_none(),
        "and it still compiles"
    );

    // Reading the `wgsl` tab moved the focus; take it back.
    source_field(&harness).focus();
    harness.step();
    source_field(&harness).type_text("@@@");
    settle(&mut harness);

    assert!(
        harness.query_by_label_contains("error").is_some(),
        "naga's message reaches the inspector"
    );
    assert_eq!(
        program(&mut harness),
        edited,
        "and the last program that compiled is still the one being drawn"
    );
}

/// Two nodes of the same kind do not share a hook slot.
///
/// The canvas keys its nodes by hand (`cx.scope(node.id, ..)` around one
/// `rsx!` call site), which is exactly where a missing key would show up as an
/// id collision — the thing the store records and, in a debug build, draws in
/// the corner.
#[test]
fn every_node_gets_a_slot_of_its_own() {
    let mut harness = loaded();
    add(&mut harness, "hsv");
    add(&mut harness, "hsv");
    assert!(harness.query_by_label("hsv2").is_some(), "two of a kind");

    let collisions = harness.state().collisions();
    assert!(collisions.is_empty(), "hook ids collided: {collisions:?}");
}

/// P-7: undo puts the old program back, and an edit after an undo is still a
/// new one.
///
/// The derived shader has to follow the history as well as the edits, which is
/// the half of the two-memo design that is easy to get wrong: the revision
/// counters rewind with the patch, and `PatchView` pairs them with an epoch
/// that does not. See the comment there for what the pairing is worth.
#[test]
fn undo_puts_the_old_program_back() {
    let mut harness = loaded();
    let first = program(&mut harness);

    wire(&mut harness, "shader2 out", "level1 in 0");
    let rewired = program(&mut harness);
    assert_ne!(rewired, first);

    click(&mut harness, "undo");
    assert_eq!(program(&mut harness), first, "undo restored the program");

    // A different edit, at the same revision number the first one had.
    wire(&mut harness, "shader2 out", "transform1 in 0");
    let again = program(&mut harness);
    assert_ne!(again, first, "the second edit regenerated the shader");
    assert_ne!(again, rewired, "and it is not the edit that was undone");
    assert!(
        again.contains("fn n3(uv: vec2<f32>) -> vec4<f32> {\n    let p = U.p[1];\n    var q"),
        "transform1 is still a transform:\n{again}"
    );
}

/// The three columns stay inside the window, whatever the patch is doing.
///
/// A `<Canvas>` and a `ScrollArea` both report the whole window as the size
/// they could fill, so `grow` without `h={0}` next to it would push everything
/// below them off the bottom (plan.md section 8).
#[test]
fn the_columns_stay_inside_the_window() {
    let mut harness = loaded();
    harness.get_by_label("level1").click();
    settle(&mut harness);

    for label in ["play", "wgsl", "recentre"] {
        let rect = harness.get_by_label(label).rect();
        assert!(
            rect.max.y <= SIZE.y && rect.max.x <= SIZE.x,
            "{label} is off screen: {rect:?}",
        );
    }
}

/// P-6: the preset is loaded behind a `<Suspense>`, and only part of the
/// screen waits for it.
///
/// Two knobs are needed to catch a boundary in the act, and both are about the
/// test rather than about the app. `Harness` runs the app until it settles as
/// it is built, so the app draws nothing at all until this test says go; and
/// egui is held to one pass for that first frame, because with its usual three
/// the future — which is parsed on a thread and takes no time — can arrive and
/// the boundary can resolve inside the same frame. One pass is enough to be
/// sure of what is on screen: a future cannot be ready on the pass that starts
/// it.
#[test]
fn the_preset_arrives_through_a_boundary() {
    let running = Rc::new(Cell::new(false));
    let passes = Rc::new(Cell::new(1usize));
    let (started, limit) = (Rc::clone(&running), Rc::clone(&passes));

    let mut harness = Harness::builder().with_size(SIZE).build_ui_state(
        move |ui, store: &mut Store| {
            ui.ctx().options_mut(|options| {
                options.max_passes = NonZeroUsize::new(limit.get()).expect("at least one pass");
            });
            if started.get() {
                run_app(ui, store);
            }
        },
        Store::new(),
    );

    running.set(true);
    harness.step();

    assert!(shown(&harness, "loading the preset"), "the fallback");
    assert!(!shown(&harness, "shader1"), "and no canvas under it yet");
    // The palette is outside the boundary and does not wait.
    assert!(shown(&harness, "patch"), "the title");
    assert!(shown(&harness, "level"), "and the palette's buttons");

    passes.set(3);
    wait_for(&mut harness, "the preset", |harness| {
        harness.query_by_label("shader1").is_some()
    });
    settle(&mut harness);

    assert!(!shown(&harness, "loading the preset"));
    for name in ["shader1", "transform1", "level1", "mix1", "out1"] {
        assert!(shown(&harness, name), "{name} is on the canvas");
    }
}

/// The inspector's copy of a shader's source editor, told apart from the ones
/// in the nodes by which column it is in.
fn source_field<'h>(harness: &'h Harness<'static, Store>) -> egui_kittest::Node<'h> {
    harness
        .get_all_by_role(egui::accesskit::Role::MultilineTextInput)
        .find(|node| node.rect().min.x > 900.0)
        .expect("the inspector shows the selected shader's source")
}

/// Whether something is drawn where the user could see it.
///
/// Suspended children are drawn into an invisible `Ui` far off screen, and egui
/// puts their widgets in the accessibility tree all the same, so "not shown"
/// is a question about position.
fn shown(harness: &Harness<'static, Store>, label: &str) -> bool {
    harness
        .query_by_label(label)
        .is_some_and(|node| node.rect().min.x > 0.0)
}
