// Copyright 2023 The AccessKit Authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (found in
// the LICENSE-APACHE file) or the MIT license (found in
// the LICENSE-MIT file), at your option.

use accesskit::{ActionHandler, ActivationHandler, TreeUpdate};
use accesskit_consumer::{FilterResult, Node, NodeId, Tree, TreeChangeHandler};
use std::collections::HashMap;
use std::fmt::Write as _;
use wasm_bindgen::JsCast as _;
use web_sys::{Document, Element, HtmlElement};

use crate::{filters::filter, node::NodeWrapper};

/// Where the mirror sits over the canvas, and at what scale.
#[derive(Clone, Copy, Debug)]
struct Viewport {
    /// The canvas's top-left corner, in CSS pixels, in the coordinates of the
    /// containing block the mirror host shares with it.
    offset: (f64, f64),
    /// Physical pixels per CSS pixel, as the tree's coordinates use them.
    pixels_per_point: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            offset: (0.0, 0.0),
            pixels_per_point: 1.0,
        }
    }
}

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
        host: HtmlElement,
        elements: HashMap<NodeId, HtmlElement>,
    },
}

/// A DOM mirror of an AccessKit tree.
///
/// One `<div>` per accessibility node, nested the way the tree is, under a
/// single host element the adapter appends to `parent`. Every node is placed
/// at its own bounding box, so a magnifier or a braille display can route to
/// the pixels the widget is actually drawn on.
///
/// The mirror is invisible and does not take the mouse: the canvas underneath
/// keeps drawing and keeps getting the pointer events.
pub struct Adapter {
    state: State,
    viewport: Viewport,
    debug: bool,
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
        let viewport = Viewport::default();

        let state = match activation_handler.request_initial_tree() {
            Some(initial_state) => {
                let tree = Box::new(Tree::new(initial_state, true));
                let (host, elements) = add_initial_tree(&document, parent, &tree, viewport, false);
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
            viewport,
            debug: false,
            action_handler: Box::new(action_handler),
        })
    }

    /// Take one frame's `TreeUpdate`. Never call this for a discarded pass:
    /// the layout it describes is not the one that will be drawn.
    pub fn update_if_active(&mut self, update_factory: impl FnOnce() -> TreeUpdate) {
        let debug = self.debug;
        match &mut self.state {
            State::Pending {
                is_host_focused,
                document,
                parent,
            } => {
                let tree = Box::new(Tree::new(update_factory(), *is_host_focused));
                let (host, elements) =
                    add_initial_tree(document, parent, &tree, self.viewport, debug);
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
                let mut handler = AdapterChangeHandler {
                    document,
                    elements,
                    debug,
                };
                tree.update_and_process_changes(update_factory(), &mut handler);
            }
        }
    }

    /// Whether the host (the canvas) has the browser's focus.
    pub fn update_host_focus_state(&mut self, is_focused: bool) {
        let debug = self.debug;
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
                let mut handler = AdapterChangeHandler {
                    document,
                    elements,
                    debug,
                };
                tree.update_host_focus_state_and_process_changes(is_focused, &mut handler);
            }
        }
    }

    /// Line the mirror up with the canvas.
    ///
    /// `offset` is the canvas's top-left corner in CSS pixels;
    /// `pixels_per_point` is what the tree's coordinates are in. The whole
    /// mirror is scaled by its reciprocal once, on the host, rather than every
    /// node dividing its own box — the same trick `<flt-semantics-host>` uses.
    pub fn set_viewport(&mut self, offset: (f64, f64), pixels_per_point: f64) {
        let viewport = Viewport {
            offset,
            pixels_per_point,
        };
        if self.viewport.offset == viewport.offset
            && self.viewport.pixels_per_point == viewport.pixels_per_point
        {
            return;
        }
        self.viewport = viewport;
        if let State::Active { host, .. } = &self.state {
            set_host_style(host, viewport, self.debug);
        }
    }

    /// Show the mirror: no transparency, and a green outline around every node.
    ///
    /// For looking at the mirror in devtools. An outline rather than a border,
    /// because a border would move the boxes it is drawn on.
    pub fn set_debug(&mut self, debug: bool) {
        if self.debug == debug {
            return;
        }
        self.debug = debug;
        if let State::Active {
            tree,
            host,
            elements,
            ..
        } = &self.state
        {
            set_host_style(host, self.viewport, debug);
            for (id, element) in elements {
                if let Some(node) = tree.state().node_by_id(*id) {
                    NodeWrapper { node, debug }.set_style(element);
                }
            }
        }
    }
}

/// The mirror host: transparent, out of the way of the mouse, and scaled from
/// the tree's physical pixels back to CSS pixels.
///
/// Being invisible is not the same as being hidden: `visibility: hidden` and
/// `display: none` take the elements out of the accessibility tree, which is
/// the one thing the mirror must not do. `filter: opacity(0%)` is stronger
/// than the `opacity` property, which some elements ignore. Both notes are
/// Flutter's, from the same CSS.
fn set_host_style(host: &HtmlElement, viewport: Viewport, debug: bool) {
    let scale = if viewport.pixels_per_point > 0.0 {
        1.0 / viewport.pixels_per_point
    } else {
        1.0
    };
    let mut style = String::new();
    let _ = write!(
        style,
        "position:absolute;left:{}px;top:{}px;width:0;height:0;overflow:visible;\
         transform-origin:0 0;transform:scale({scale});pointer-events:none;",
        viewport.offset.0, viewport.offset.1
    );
    if !debug {
        style.push_str("filter:opacity(0%);color:rgba(0,0,0,0);");
    }
    let _ = host.set_attribute("style", &style);
}

fn add_initial_tree(
    document: &Document,
    parent: &Element,
    tree: &Tree,
    viewport: Viewport,
    debug: bool,
) -> (HtmlElement, HashMap<NodeId, HtmlElement>) {
    let host = create_element(document);
    set_host_style(&host, viewport, debug);
    let _ = parent.append_child(&host);
    let mut elements = HashMap::new();
    let root_node = tree.state().root();
    add_element_recursive(document, &host, &root_node, &mut elements, debug);
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
    debug: bool,
) -> HtmlElement {
    let element = create_element(document);
    NodeWrapper { node: *node, debug }.set_all_attributes(&element);
    let _ = parent.append_child(&element);
    elements.insert(node.id(), element.clone());
    element
}

fn add_element_recursive(
    document: &Document,
    parent: &HtmlElement,
    node: &Node<'_>,
    elements: &mut HashMap<NodeId, HtmlElement>,
    debug: bool,
) {
    let element = add_element(document, parent, node, elements, debug);
    for child in node.filtered_children(&filter) {
        add_element_recursive(document, &element, &child, elements, debug);
    }
}

struct AdapterChangeHandler<'a> {
    document: &'a Document,
    elements: &'a mut HashMap<NodeId, HtmlElement>,
    debug: bool,
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
        add_element(
            self.document,
            &parent_element,
            node,
            self.elements,
            self.debug,
        );
    }

    fn node_updated(&mut self, old_node: &Node<'_>, new_node: &Node<'_>) {
        if filter(new_node) != FilterResult::Include {
            return;
        }
        let Some(element) = self.elements.get(&new_node.id()) else {
            return;
        };
        let debug = self.debug;
        let old_wrapper = NodeWrapper {
            node: *old_node,
            debug,
        };
        let new_wrapper = NodeWrapper {
            node: *new_node,
            debug,
        };
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
