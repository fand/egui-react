// Copyright 2023 The AccessKit Authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (found in
// the LICENSE-APACHE file) or the MIT license (found in
// the LICENSE-MIT file), at your option.

//! What the mirror puts in the document, checked against a handmade tree.
//!
//! There is no egui here: the adapter only ever sees an `accesskit::TreeUpdate`.
//!
//! These need a browser, so the file is empty everywhere else and
//! `cargo test --workspace` has nothing to run. To run them:
//!
//! ```sh
//! wasm-pack test --headless --chrome crates/accesskit-web
//! ```

#![cfg(target_arch = "wasm32")]

use accesskit::{
    Action, Node as AkNode, NodeId, Rect, Role, Toggled, Tree, TreeId, TreeUpdate, Uuid,
};
use accesskit_web::Adapter;
use wasm_bindgen::JsCast as _;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{Element, HtmlInputElement};

wasm_bindgen_test_configure!(run_in_browser);

const ROOT: NodeId = NodeId(0);
const BUTTON: NodeId = NodeId(1);
const CHECKBOX: NodeId = NodeId(2);
const LABEL: NodeId = NodeId(3);
const SLIDER: NodeId = NodeId(4);

/// A window with a button, a checkbox, a piece of text and a slider, all with
/// boxes.
fn initial_tree() -> TreeUpdate {
    let mut root = AkNode::new(Role::Window);
    root.set_bounds(Rect::new(0.0, 0.0, 200.0, 100.0));
    root.set_children(vec![BUTTON, CHECKBOX, LABEL, SLIDER]);

    let mut button = AkNode::new(Role::Button);
    button.set_label("increment");
    button.set_bounds(Rect::new(10.0, 20.0, 60.0, 40.0));
    button.add_action(Action::Click);
    button.add_action(Action::Focus);

    let mut checkbox = AkNode::new(Role::CheckBox);
    checkbox.set_label("done");
    checkbox.set_toggled(Toggled::True);
    checkbox.set_bounds(Rect::new(10.0, 50.0, 30.0, 70.0));

    let mut label = AkNode::new(Role::Label);
    label.set_label("hello");
    label.set_bounds(Rect::new(70.0, 20.0, 120.0, 40.0));

    let mut slider = AkNode::new(Role::Slider);
    slider.set_label("volume");
    slider.set_numeric_value(50.0);
    slider.set_min_numeric_value(0.0);
    slider.set_max_numeric_value(100.0);
    slider.set_numeric_value_step(1.0);
    slider.set_bounds(Rect::new(10.0, 75.0, 110.0, 95.0));
    slider.add_action(Action::SetValue);

    TreeUpdate {
        nodes: vec![
            (ROOT, root),
            (BUTTON, button),
            (CHECKBOX, checkbox),
            (LABEL, label),
            (SLIDER, slider),
        ],
        tree: Some(Tree::new(ROOT)),
        tree_id: TreeId::ROOT,
        focus: ROOT,
    }
}

/// A fresh element under `<body>` to hang one test's mirror on.
fn container() -> Element {
    let document = web_sys::window().unwrap().document().unwrap();
    let container = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&container).unwrap();
    container
}

fn mirror(container: &Element) -> Adapter {
    struct NoActivation;
    impl accesskit::ActivationHandler for NoActivation {
        fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
            None
        }
    }
    struct DropActions;
    impl accesskit::ActionHandler for DropActions {
        fn do_action(&mut self, _request: accesskit::ActionRequest) {}
    }
    Adapter::new(container, NoActivation, DropActions).unwrap()
}

fn query(container: &Element, selector: &str) -> Option<Element> {
    container.query_selector(selector).unwrap()
}

fn style_of(element: &Element) -> String {
    element.get_attribute("style").unwrap_or_default()
}

#[wasm_bindgen_test]
fn roles_labels_and_values_reach_the_dom() {
    let container = container();
    let mut adapter = mirror(&container);
    adapter.update_if_active(initial_tree);

    let button = query(&container, "[role=\"button\"]").expect("the button is mirrored");
    assert_eq!(
        button.get_attribute("aria-label").as_deref(),
        Some("increment")
    );
    // Focusable nodes are Tab stops, in tree order.
    assert_eq!(button.get_attribute("tabindex").as_deref(), Some("0"));

    let checkbox = query(&container, "[role=\"checkbox\"]").expect("the checkbox is mirrored");
    assert_eq!(
        checkbox.get_attribute("aria-checked").as_deref(),
        Some("true")
    );
    assert_eq!(
        checkbox.get_attribute("aria-label").as_deref(),
        Some("done")
    );

    // Text is text content, not `aria-label`, and says what it is so that
    // Safari does not merge it into its neighbours.
    let label = query(&container, "[role=\"paragraph\"]").expect("the text is mirrored");
    assert_eq!(label.text_content().as_deref(), Some("hello"));
    assert!(label.get_attribute("aria-label").is_none());
}

#[wasm_bindgen_test]
fn nodes_are_placed_at_their_bounding_boxes() {
    let container = container();
    let mut adapter = mirror(&container);
    adapter.update_if_active(initial_tree);
    adapter.set_viewport((0.0, 0.0), 2.0);

    // The host takes the whole mirror back to CSS pixels in one go, and lets
    // the mouse through to the canvas underneath.
    let host = container
        .first_element_child()
        .expect("the mirror has a host");
    let host_style = style_of(&host);
    assert!(host_style.contains("transform:scale(0.5)"), "{host_style}");
    assert!(host_style.contains("pointer-events:none"), "{host_style}");

    // The window is the root of the tree, so its own box is where the mirror
    // starts.
    let window = query(&container, "[role=\"window\"]").expect("the root is mirrored");
    let window_style = style_of(&window);
    assert!(window_style.contains("left:0px;top:0px;"), "{window_style}");
    assert!(
        window_style.contains("width:200px;height:100px;"),
        "{window_style}"
    );

    // A child's box is relative to its parent's: an absolutely positioned
    // parent is the containing block of its absolutely positioned children.
    let button = query(&container, "[role=\"button\"]").expect("the button is mirrored");
    let button_style = style_of(&button);
    assert!(
        button_style.contains("position:absolute;"),
        "{button_style}"
    );
    assert!(
        button_style.contains("left:10px;top:20px;"),
        "{button_style}"
    );
    assert!(
        button_style.contains("width:50px;height:20px;"),
        "{button_style}"
    );
}

#[wasm_bindgen_test]
fn an_update_changes_only_what_changed() {
    let container = container();
    let mut adapter = mirror(&container);
    adapter.update_if_active(initial_tree);

    // The button is renamed and moves; the checkbox goes away.
    adapter.update_if_active(|| {
        let mut root = AkNode::new(Role::Window);
        root.set_bounds(Rect::new(0.0, 0.0, 200.0, 100.0));
        root.set_children(vec![BUTTON, LABEL]);

        let mut button = AkNode::new(Role::Button);
        button.set_label("decrement");
        button.set_bounds(Rect::new(10.0, 25.0, 60.0, 45.0));
        button.add_action(Action::Click);
        button.add_action(Action::Focus);

        let mut label = AkNode::new(Role::Label);
        label.set_label("hello");
        label.set_bounds(Rect::new(70.0, 20.0, 120.0, 40.0));

        TreeUpdate {
            nodes: vec![(ROOT, root), (BUTTON, button), (LABEL, label)],
            tree: Some(Tree::new(ROOT)),
            tree_id: TreeId::ROOT,
            focus: ROOT,
        }
    });

    let button = query(&container, "[role=\"button\"]").expect("the button is still mirrored");
    assert_eq!(
        button.get_attribute("aria-label").as_deref(),
        Some("decrement")
    );
    assert!(
        style_of(&button).contains("top:25px;"),
        "{}",
        style_of(&button)
    );

    assert!(
        query(&container, "[role=\"checkbox\"]").is_none(),
        "a node that left the tree leaves the mirror"
    );
    // Untouched nodes keep their element.
    let label = query(&container, "[role=\"paragraph\"]").expect("the text is still mirrored");
    assert_eq!(label.text_content().as_deref(), Some("hello"));
}

#[wasm_bindgen_test]
fn elements_say_which_node_they_stand_for() {
    let container = container();
    let mut adapter = mirror(&container);
    adapter.update_if_active(initial_tree);

    // What turns a DOM event back into an `ActionRequest`.
    let button = query(&container, "[role=\"button\"]").expect("the button is mirrored");
    assert_eq!(
        button.get_attribute("data-accesskit-node").as_deref(),
        Some(BUTTON.0.to_string().as_str())
    );
    assert_eq!(
        button.get_attribute("data-accesskit-tree").as_deref(),
        Some(Uuid::nil().as_u128().to_string().as_str())
    );
}

#[wasm_bindgen_test]
fn a_slider_is_a_real_range_input() {
    let container = container();
    let mut adapter = mirror(&container);
    adapter.update_if_active(initial_tree);

    // A `<div role="slider">` has no value to change, so `input`/`change`
    // would never fire and `Action::SetValue` could never leave the DOM.
    let slider = query(&container, "[role=\"slider\"]").expect("the slider is mirrored");
    assert_eq!(slider.tag_name(), "INPUT");
    assert_eq!(slider.get_attribute("type").as_deref(), Some("range"));
    assert_eq!(slider.get_attribute("min").as_deref(), Some("0"));
    assert_eq!(slider.get_attribute("max").as_deref(), Some("100"));
    assert_eq!(slider.get_attribute("step").as_deref(), Some("1"));

    // The value is a property, not an attribute: once assistive technology
    // has moved the range, the attribute is only its default.
    let input = slider.unchecked_ref::<HtmlInputElement>();
    assert_eq!(input.value(), "50");

    // The ARIA pair is written too, for the roles that stay `<div>`s and for
    // anything that reads the node rather than the control.
    assert_eq!(slider.get_attribute("aria-valuenow").as_deref(), Some("50"));
    assert_eq!(slider.get_attribute("aria-valuemin").as_deref(), Some("0"));
    assert_eq!(
        slider.get_attribute("aria-valuemax").as_deref(),
        Some("100")
    );
    assert_eq!(
        slider.get_attribute("aria-label").as_deref(),
        Some("volume")
    );
}

#[wasm_bindgen_test]
fn a_slider_follows_the_app() {
    let container = container();
    let mut adapter = mirror(&container);
    adapter.update_if_active(initial_tree);

    let slider = query(&container, "[role=\"slider\"]").expect("the slider is mirrored");
    let input = slider.unchecked_ref::<HtmlInputElement>();
    // Assistive technology moved it; the app has not agreed yet.
    input.set_value("80");

    adapter.update_if_active(|| {
        let mut update = initial_tree();
        for (id, node) in &mut update.nodes {
            if *id == SLIDER {
                node.set_numeric_value(70.0);
            }
        }
        update
    });

    assert_eq!(input.value(), "70", "the app has the last word");
}
