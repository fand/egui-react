// Copyright 2023 The AccessKit Authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (found in
// the LICENSE-APACHE file) or the MIT license (found in
// the LICENSE-MIT file), at your option.

//! The way back: DOM events turned into `ActionRequest`s.
//!
//! The upstream prototype had none of this — it stored an `ActionHandler` and
//! never called it. The handler is not something the adapter implements; it is
//! the hole the adapter pushes requests out through.
//!
//! **Where the events come from.** The mirror is `pointer-events: none`, so a
//! human clicking the page hits the canvas, and egui handles that itself. A
//! `click` that lands on a mirror element can therefore only have been
//! synthesised, by a screen reader activating the element it is on. That is
//! what makes the double-fire problem Flutter solves with a 200 ms debouncer
//! (`ClickDebouncer`, flutter#130162) not arise here — until some element is
//! given `pointer-events: auto`, at which point it will.

use accesskit::{Action, ActionData, ActionHandler, ActionRequest, NodeId, TreeId, Uuid};
use accesskit_consumer::Node;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::Closure;
use web_sys::{Element, Event, HtmlElement, KeyboardEvent};

/// The node an event landed on, written on the element itself so that one set
/// of listeners on the host can serve the whole mirror.
const NODE_ATTRIBUTE: &str = "data-accesskit-node";
const TREE_ATTRIBUTE: &str = "data-accesskit-tree";

/// Roles that are activated by Enter or Space when they have keyboard focus.
///
/// A real `<button>` gets this from the browser; a `<div role="button">` has
/// to do it itself.
const KEY_ACTIVATED_ROLES: &[&str] = &[
    "button",
    "checkbox",
    "link",
    "menuitem",
    "menuitemcheckbox",
    "menuitemradio",
    "option",
    "radio",
    "switch",
    "tab",
    "treeitem",
];

/// Roles whose value is a number the client can set outright.
const NUMERIC_ROLES: &[&str] = &["slider", "spinbutton"];

/// The shared way out of the adapter. The listeners outlive any one call into
/// the adapter, so the handler cannot simply be borrowed from it.
pub(crate) type SharedActionHandler = Rc<RefCell<Box<dyn ActionHandler>>>;

/// Say which node an element stands for, so an event can be traced back.
pub(crate) fn tag_element(element: &HtmlElement, node: &Node<'_>) {
    let (node_id, tree_id) = node.locate();
    let _ = element.set_attribute(NODE_ATTRIBUTE, &node_id.0.to_string());
    let _ = element.set_attribute(TREE_ATTRIBUTE, &tree_id.0.as_u128().to_string());
}

/// Listen once, on the host, for everything the mirror can produce.
///
/// The returned closures own the Rust side of each listener and must be kept
/// alive for as long as the host is in the document.
pub(crate) fn listen(
    host: &HtmlElement,
    handler: &SharedActionHandler,
) -> Vec<Closure<dyn FnMut(Event)>> {
    [
        ("click", on_click as fn(&Event, &SharedActionHandler)),
        ("keydown", on_keydown),
        ("input", on_value_changed),
        ("change", on_value_changed),
        // `focus` does not bubble, so it cannot be delegated; `focusin` is the
        // same event that does.
        ("focusin", on_focus),
    ]
    .into_iter()
    .filter_map(|(name, callback)| {
        let handler = Rc::clone(handler);
        let closure =
            Closure::<dyn FnMut(Event)>::new(move |event: Event| callback(&event, &handler));
        host.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref())
            .ok()?;
        Some(closure)
    })
    .collect()
}

/// A screen reader activated an element: the same thing as a click on the
/// widget underneath.
fn on_click(event: &Event, handler: &SharedActionHandler) {
    let Some(element) = target_element(event) else {
        return;
    };
    send(handler, &element, Action::Click, None);
}

/// Enter and Space on a control that the browser does not activate for us.
fn on_keydown(event: &Event, handler: &SharedActionHandler) {
    let Some(event) = event.dyn_ref::<KeyboardEvent>() else {
        return;
    };
    let key = event.key();
    if key != "Enter" && key != " " {
        return;
    }
    let Some(element) = target_element(event) else {
        return;
    };
    if !role_is(&element, KEY_ACTIVATED_ROLES) {
        return;
    }
    // Space would otherwise scroll the page.
    event.prevent_default();
    send(handler, &element, Action::Click, None);
}

/// A real form control changed. The mirror is `<div>`s today, so this fires
/// only for the elements that will become `<input>`s (a range slider, a text
/// box); a `<div role="slider">` has no value to change.
fn on_value_changed(event: &Event, handler: &SharedActionHandler) {
    let Some(element) = target_element(event) else {
        return;
    };
    if !role_is(&element, NUMERIC_ROLES) {
        return;
    }
    let Some(value) = element
        .dyn_ref::<web_sys::HtmlInputElement>()
        .and_then(|input| input.value().parse::<f64>().ok())
    else {
        return;
    };
    send(
        handler,
        &element,
        Action::SetValue,
        Some(ActionData::NumericValue(value)),
    );
}

/// The browser moved focus into the mirror. Nothing here ever calls `focus()`,
/// so this is never an echo of the app's own focus (which is the loop Flutter
/// guards against with `_lastEvent == requestedFocus`).
fn on_focus(event: &Event, handler: &SharedActionHandler) {
    let Some(element) = target_element(event) else {
        return;
    };
    send(handler, &element, Action::Focus, None);
}

/// The innermost mirrored element the event passed through.
fn target_element(event: &Event) -> Option<Element> {
    let mut element = event.target()?.dyn_into::<Element>().ok()?;
    loop {
        if element.has_attribute(NODE_ATTRIBUTE) {
            return Some(element);
        }
        element = element.parent_element()?;
    }
}

fn role_is(element: &Element, roles: &[&str]) -> bool {
    element
        .get_attribute("role")
        .is_some_and(|role| roles.contains(&role.as_str()))
}

fn send(
    handler: &SharedActionHandler,
    element: &Element,
    action: Action,
    data: Option<ActionData>,
) {
    let Some(target_node) = element
        .get_attribute(NODE_ATTRIBUTE)
        .and_then(|id| id.parse::<u64>().ok())
        .map(NodeId)
    else {
        return;
    };
    let Some(target_tree) = element
        .get_attribute(TREE_ATTRIBUTE)
        .and_then(|id| id.parse::<u128>().ok())
        .map(|id| TreeId(Uuid::from_u128(id)))
    else {
        return;
    };
    handler.borrow_mut().do_action(ActionRequest {
        action,
        target_tree,
        target_node,
        data,
    });
}
