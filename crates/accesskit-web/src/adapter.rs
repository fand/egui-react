// Copyright 2023 The AccessKit Authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (found in
// the LICENSE-APACHE file) or the MIT license (found in
// the LICENSE-MIT file), at your option.

use accesskit::{ActionHandler, ActivationHandler, TreeUpdate};
use accesskit_consumer::{FilterResult, Node, NodeId, Tree, TreeChangeHandler};
use std::collections::HashMap;
use wasm_bindgen::JsCast as _;
use web_sys::{Document, Element, HtmlElement};

use crate::{filters::filter, node::NodeWrapper};

/// Before the first tree arrives there is nothing to mirror, so the adapter
/// only remembers where the mirror will go.
enum State {
    Pending {
        is_host_focused: bool,
        document: Document,
        parent: Element,
    },
    Active {
        // Boxed: `Tree` is several hundred bytes and `Pending` is three words.
        tree: Box<Tree>,
        document: Document,
        #[expect(dead_code, reason = "read once the mirror is positioned")]
        host: HtmlElement,
        elements: HashMap<NodeId, HtmlElement>,
    },
}

/// A DOM mirror of an AccessKit tree.
///
/// One `<div>` per accessibility node, nested the way the tree is, under a
/// single host element the adapter appends to `parent`.
pub struct Adapter {
    state: State,
    #[expect(dead_code, reason = "wired up in the DOM -> ActionRequest step")]
    action_handler: Box<dyn ActionHandler>,
}

impl Adapter {
    /// Build the mirror under `parent`, which should be the element the canvas
    /// sits in, so that the mirror shares the canvas's containing block.
    pub fn new(
        parent: &Element,
        mut activation_handler: impl ActivationHandler,
        action_handler: impl 'static + ActionHandler,
    ) -> Option<Self> {
        let document = web_sys::window()?.document()?;

        let state = match activation_handler.request_initial_tree() {
            Some(initial_state) => {
                let tree = Box::new(Tree::new(initial_state, true));
                let (host, elements) = add_initial_tree(&document, parent, &tree);
                State::Active {
                    tree,
                    document,
                    host,
                    elements,
                }
            }
            None => State::Pending {
                is_host_focused: true,
                document,
                parent: parent.clone(),
            },
        };
        Some(Self {
            state,
            action_handler: Box::new(action_handler),
        })
    }

    /// Take one frame's `TreeUpdate`. Never call this for a discarded pass:
    /// the layout it describes is not the one that will be drawn.
    pub fn update_if_active(&mut self, update_factory: impl FnOnce() -> TreeUpdate) {
        match &mut self.state {
            State::Pending {
                is_host_focused,
                document,
                parent,
            } => {
                let tree = Box::new(Tree::new(update_factory(), *is_host_focused));
                let (host, elements) = add_initial_tree(document, parent, &tree);
                self.state = State::Active {
                    tree,
                    document: document.clone(),
                    host,
                    elements,
                };
            }
            State::Active {
                tree,
                document,
                elements,
                ..
            } => {
                let mut handler = AdapterChangeHandler { document, elements };
                tree.update_and_process_changes(update_factory(), &mut handler);
            }
        }
    }

    /// Whether the host (the canvas) has the browser's focus.
    pub fn update_host_focus_state(&mut self, is_focused: bool) {
        match &mut self.state {
            State::Pending {
                is_host_focused, ..
            } => *is_host_focused = is_focused,
            State::Active {
                tree,
                document,
                elements,
                ..
            } => {
                let mut handler = AdapterChangeHandler { document, elements };
                tree.update_host_focus_state_and_process_changes(is_focused, &mut handler);
            }
        }
    }
}

fn add_initial_tree(
    document: &Document,
    parent: &Element,
    tree: &Tree,
) -> (HtmlElement, HashMap<NodeId, HtmlElement>) {
    let host = create_element(document);
    let _ = parent.append_child(&host);
    let mut elements = HashMap::new();
    let root_node = tree.state().root();
    add_element_recursive(document, &host, &root_node, &mut elements);
    (host, elements)
}

fn create_element(document: &Document) -> HtmlElement {
    document
        .create_element("div")
        .expect("a document can always make a <div>")
        .unchecked_into::<HtmlElement>()
}

fn add_element(
    document: &Document,
    parent: &HtmlElement,
    node: &Node<'_>,
    elements: &mut HashMap<NodeId, HtmlElement>,
) -> HtmlElement {
    let element = create_element(document);
    let wrapper = NodeWrapper(*node);
    wrapper.set_all_attributes(&element);
    let _ = parent.append_child(&element);
    elements.insert(node.id(), element.clone());
    element
}

fn add_element_recursive(
    document: &Document,
    parent: &HtmlElement,
    node: &Node<'_>,
    elements: &mut HashMap<NodeId, HtmlElement>,
) {
    let element = add_element(document, parent, node, elements);
    for child in node.filtered_children(&filter) {
        add_element_recursive(document, &element, &child, elements);
    }
}

struct AdapterChangeHandler<'a> {
    document: &'a Document,
    elements: &'a mut HashMap<NodeId, HtmlElement>,
}

impl TreeChangeHandler for AdapterChangeHandler<'_> {
    fn node_added(&mut self, node: &Node<'_>) {
        if filter(node) != FilterResult::Include {
            return;
        }
        if self.elements.contains_key(&node.id()) {
            return;
        }
        let Some(parent) = node.filtered_parent(&filter) else {
            return;
        };
        let Some(parent_element) = self.elements.get(&parent.id()).cloned() else {
            return;
        };
        add_element(self.document, &parent_element, node, self.elements);
    }

    fn node_updated(&mut self, old_node: &Node<'_>, new_node: &Node<'_>) {
        if filter(new_node) != FilterResult::Include {
            return;
        }
        let Some(element) = self.elements.get(&new_node.id()) else {
            return;
        };
        let old_wrapper = NodeWrapper(*old_node);
        let new_wrapper = NodeWrapper(*new_node);
        new_wrapper.update_attributes(element, &old_wrapper);
    }

    fn focus_moved(&mut self, _old_node: Option<&Node<'_>>, _new_node: Option<&Node<'_>>) {
        // Moving the browser's focus onto a mirror element is the one thing
        // that is known not to work yet: eframe treats "the canvas is not the
        // active element" as "the app lost focus" and stops taking keys
        // (plan.md 1.3). The upstream prototype called `element.focus()` here,
        // and `blur()` on the way out, which Flutter's semantics layer warns
        // against. Both are left out until the focus design is settled.
    }

    fn node_removed(&mut self, node: &Node<'_>) {
        if let Some(element) = self.elements.remove(&node.id()) {
            element.remove();
        }
    }
}
