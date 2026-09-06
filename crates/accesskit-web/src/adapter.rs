// Copyright 2023 The AccessKit Authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (found in
// the LICENSE-APACHE file) or the MIT license (found in
// the LICENSE-MIT file), at your option.

use accesskit::{ActionHandler, ActivationHandler, TreeUpdate};
use accesskit_consumer::{FilterResult, Node, NodeId, Tree, TreeChangeHandler};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::Closure;
use web_sys::{Document, Element, Event, HtmlElement};

use crate::action::{self, SharedActionHandler};
use crate::node::ElementKind;
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
    /// The element that keeps the browser's focus — the canvas. The mirror
    /// never takes focus itself; instead this element's
    /// `aria-activedescendant` says which mirrored node the app has focused.
    focus_owner: Element,
    /// What every element of this mirror's ids start with. Unique per
    /// adapter, so two mirrors on one page cannot collide.
    prefix: String,
    /// The node `aria-activedescendant` currently names, so that setting it
    /// to the same node again can be dropped.
    active_descendant: Option<NodeId>,
    action_handler: SharedActionHandler,
    /// The Rust side of the host's event listeners. Dropping these unhooks
    /// them, so they live exactly as long as the adapter.
    _listeners: Vec<Closure<dyn FnMut(Event)>>,
}

impl Adapter {
    /// Build the mirror under `parent`, which should be the element the canvas
    /// sits in, so that the mirror shares the canvas's containing block.
    pub fn new(
        parent: &Element,
        focus_owner: &Element,
        mut activation_handler: impl ActivationHandler,
        action_handler: impl 'static + ActionHandler,
    ) -> Option<Self> {
        let document = web_sys::window()?.document()?;
        let viewport = Viewport::default();
        let prefix = next_prefix();
        let action_handler: SharedActionHandler = Rc::new(RefCell::new(
            Box::new(action_handler) as Box<dyn ActionHandler>
        ));

        let mut listeners = Vec::new();
        let state = match activation_handler.request_initial_tree() {
            Some(initial_state) => {
                let tree = Box::new(Tree::new(initial_state, true));
                let (host, elements) = add_initial_tree(
                    &document,
                    parent,
                    focus_owner,
                    &prefix,
                    &tree,
                    viewport,
                    false,
                );
                listeners = action::listen(&host, &action_handler);
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
            focus_owner: focus_owner.clone(),
            prefix,
            active_descendant: None,
            action_handler,
            _listeners: listeners,
        })
    }

    /// Take one frame's `TreeUpdate`. Never call this for a discarded pass:
    /// the layout it describes is not the one that will be drawn.
    pub fn update_if_active(&mut self, update_factory: impl FnOnce() -> TreeUpdate) {
        let Self {
            state,
            viewport,
            debug,
            focus_owner,
            prefix,
            active_descendant,
            action_handler,
            _listeners,
        } = self;
        let debug = *debug;
        match state {
            State::Pending {
                is_host_focused,
                document,
                parent,
            } => {
                let tree = Box::new(Tree::new(update_factory(), *is_host_focused));
                let (host, elements) = add_initial_tree(
                    document,
                    parent,
                    focus_owner,
                    prefix,
                    &tree,
                    *viewport,
                    debug,
                );
                *_listeners = action::listen(&host, action_handler);
                let focused = tree.state().focus_id();
                *state = State::Active {
                    tree,
                    document: document.clone(),
                    host,
                    elements,
                };
                if let State::Active { elements, .. } = state {
                    settle_focus(focus_owner, active_descendant, prefix, elements, focused);
                }
            }
            State::Active {
                tree,
                document,
                elements,
                ..
            } => {
                let moved_to = {
                    let mut handler = AdapterChangeHandler {
                        document,
                        elements,
                        debug,
                        prefix,
                        focus_moved_to: None,
                    };
                    tree.update_and_process_changes(update_factory(), &mut handler);
                    handler.focus_moved_to
                };
                settle_focus(
                    focus_owner,
                    active_descendant,
                    prefix,
                    elements,
                    moved_to.flatten(),
                );
            }
        }
    }

    /// Whether the host (the canvas) has the browser's focus.
    pub fn update_host_focus_state(&mut self, is_focused: bool) {
        let Self {
            state,
            debug,
            focus_owner,
            prefix,
            active_descendant,
            ..
        } = self;
        let debug = *debug;
        match state {
            State::Pending {
                is_host_focused, ..
            } => *is_host_focused = is_focused,
            State::Active {
                tree,
                document,
                elements,
                ..
            } => {
                let moved_to = {
                    let mut handler = AdapterChangeHandler {
                        document,
                        elements,
                        debug,
                        prefix,
                        focus_moved_to: None,
                    };
                    tree.update_host_focus_state_and_process_changes(is_focused, &mut handler);
                    handler.focus_moved_to
                };
                // Losing focus reports a move to nothing, and `settle_focus`
                // leaves the attribute where it was; regaining it reports the
                // node again and writes the same value back.
                settle_focus(
                    focus_owner,
                    active_descendant,
                    prefix,
                    elements,
                    moved_to.flatten(),
                );
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
/// Ids are handed out per adapter, not per page, so that a document with two
/// mirrors in it (a test file, say) keeps them apart.
fn next_prefix() -> String {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    format!("accesskit-mirror-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

/// The id of the element standing for a node, which is what
/// `aria-activedescendant` names.
fn element_id(prefix: &str, id: NodeId) -> String {
    format!("{prefix}-{}", u128::from(id))
}

/// Point the focus owner at the mirror.
///
/// `aria-activedescendant` may only name an element the referencing element
/// owns — a DOM descendant, or one named by `aria-owns` (ARIA 1.2, and the
/// APG's "Developing a Keyboard Interface"). The host is the canvas's
/// *sibling*, so `aria-owns` is what makes the reference legal. The attribute
/// is also only allowed on a few roles, none of which a bare `<canvas>` has,
/// so the canvas is given `role="application"` — the one that fits a widget
/// tree drawn by the app, and the one that puts a screen reader in the mode
/// where `aria-activedescendant` means anything.
fn own_the_mirror(focus_owner: &Element, host_id: &str) {
    if !focus_owner.has_attribute("role") {
        let _ = focus_owner.set_attribute("role", "application");
    }
    let _ = focus_owner.set_attribute("aria-owns", host_id);
}

/// Say which node the app has focused, after the DOM it names is in place.
///
/// Three rules, all Flutter's, from a semantics layer that had to learn them
/// (plan.md 1.4):
///
/// (a) apply the focus only once the tree update is fully applied — hence
///     this runs after `update_and_process_changes` returns, not inside
///     `focus_moved`;
/// (b) never take focus away. `blur()` is the mistake Flutter warns about;
///     here its equivalent is clearing `aria-activedescendant` when the app
///     reports no focus, so that is exactly what is not done. The canvas
///     losing the browser's focus is not the app forgetting where it was;
/// (c) drop a repeat of the node already named.
fn settle_focus(
    focus_owner: &Element,
    active_descendant: &mut Option<NodeId>,
    prefix: &str,
    elements: &HashMap<NodeId, HtmlElement>,
    moved_to: Option<NodeId>,
) {
    if let Some(new_focus) = moved_to
        && *active_descendant != Some(new_focus)
        && elements.contains_key(&new_focus)
    {
        *active_descendant = Some(new_focus);
        let _ = focus_owner.set_attribute("aria-activedescendant", &element_id(prefix, new_focus));
    }
    // A reference to an element that has left the document is worse than
    // none: this is the one case where the attribute does come off.
    if let Some(current) = *active_descendant
        && !elements.contains_key(&current)
    {
        *active_descendant = None;
        let _ = focus_owner.remove_attribute("aria-activedescendant");
    }
}

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
    focus_owner: &Element,
    prefix: &str,
    tree: &Tree,
    viewport: Viewport,
    debug: bool,
) -> (HtmlElement, HashMap<NodeId, HtmlElement>) {
    let host = create_element(document, ElementKind::Div);
    let host_id = format!("{prefix}-host");
    host.set_id(&host_id);
    // The mirror is never a tab stop: the canvas is the one place the
    // browser's focus is allowed to be, because eframe reads anything else as
    // "the app lost focus" and stops taking keys (plan.md 1.3).
    let _ = host.set_attribute("tabindex", "-1");
    set_host_style(&host, viewport, debug);
    let _ = parent.append_child(&host);
    own_the_mirror(focus_owner, &host_id);
    let mut elements = HashMap::new();
    let root_node = tree.state().root();
    add_element_recursive(document, &host, prefix, &root_node, &mut elements, debug);
    (host, elements)
}

fn create_element(document: &Document, kind: ElementKind) -> HtmlElement {
    let element = document
        .create_element(kind.tag_name())
        .expect("a document can always make a <div> or an <input>")
        .unchecked_into::<HtmlElement>();
    kind.init(&element);
    element
}

fn add_element(
    document: &Document,
    parent: &HtmlElement,
    prefix: &str,
    node: &Node<'_>,
    elements: &mut HashMap<NodeId, HtmlElement>,
    debug: bool,
) -> HtmlElement {
    let wrapper = NodeWrapper { node: *node, debug };
    let element = create_element(document, wrapper.element_kind());
    // `aria-activedescendant` names an element by id, so every node needs one.
    element.set_id(&element_id(prefix, node.id()));
    wrapper.set_all_attributes(&element);
    action::tag_element(&element, node);
    let _ = parent.append_child(&element);
    elements.insert(node.id(), element.clone());
    element
}

fn add_element_recursive(
    document: &Document,
    parent: &HtmlElement,
    prefix: &str,
    node: &Node<'_>,
    elements: &mut HashMap<NodeId, HtmlElement>,
    debug: bool,
) {
    let element = add_element(document, parent, prefix, node, elements, debug);
    for child in node.filtered_children(&filter) {
        add_element_recursive(document, &element, prefix, &child, elements, debug);
    }
}

struct AdapterChangeHandler<'a> {
    document: &'a Document,
    elements: &'a mut HashMap<NodeId, HtmlElement>,
    debug: bool,
    prefix: &'a str,
    /// Where focus went, recorded rather than applied: the outer `Option` is
    /// "was there a move at all", the inner one is the node it went to.
    focus_moved_to: Option<Option<NodeId>>,
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
            self.prefix,
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
        // A `<div>` cannot become an `<input>`, so a node whose role crossed
        // that line needs a new element. Its children move over rather than
        // being rebuilt, so every other entry in the map stays valid.
        if old_wrapper.element_kind() != new_wrapper.element_kind() {
            let replacement = create_element(self.document, new_wrapper.element_kind());
            replacement.set_id(&element_id(self.prefix, new_node.id()));
            while let Some(child) = element.first_child() {
                let _ = replacement.append_child(&child);
            }
            new_wrapper.set_all_attributes(&replacement);
            action::tag_element(&replacement, new_node);
            let _ = element.replace_with_with_node_1(&replacement);
            self.elements.insert(new_node.id(), replacement);
            return;
        }
        new_wrapper.update_attributes(element, &old_wrapper);
    }

    fn focus_moved(&mut self, _old_node: Option<&Node<'_>>, new_node: Option<&Node<'_>>) {
        // Nothing here calls `element.focus()`. Real DOM focus stays on the
        // canvas, because eframe reads "the canvas is not the active element"
        // as "the app lost focus" and stops taking keys (plan.md 1.3); the
        // canvas's `aria-activedescendant` says where the app's focus is
        // instead. Only record it: the elements it may name are not all built
        // yet at this point in the update.
        self.focus_moved_to = Some(new_node.map(|node| node.id()));
    }

    fn node_removed(&mut self, node: &Node<'_>) {
        if let Some(element) = self.elements.remove(&node.id()) {
            element.remove();
        }
    }
}
