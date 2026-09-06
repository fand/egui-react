//! The layout engine: one taffy tree per `<View>` root, drawn in one egui pass.
//!
//! This is what egui-react uses instead of `egui_taffy`. The behaviour is the
//! same — the measure function, the node reuse rule, the sweep and the two
//! pass protocol are ports of `egui_taffy` 0.14 with the fork's two fixes
//! (skip the discard when the layout did not move; compute the layout before
//! drawing when only the root rect resized) built in. What is gone is the
//! per-node egui `Ui`: a node here is a rect, not a `Ui`, and only a leaf gets
//! a `Ui` of its own. egui-react draws no backgrounds and no interactive
//! containers on a `<View>`, so nothing is lost and a row costs three `Ui`s
//! instead of nine.
//!
//! One tree per root lives in the [`Store`], not in egui memory: one map
//! lookup per frame, and a tree left behind by an unmounted subtree is dropped
//! by [`Store::end_pass`] instead of growing egui's `IdTypeMap` forever.

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::rc::Rc;

use egui::{Pos2, Rect, UiBuilder, Vec2, Vec2b};
use taffy::{AvailableSpace, Layout, NodeId, Size, TaffyTree, TraversePartialTree as _};

use crate::store::Store;

/// What a leaf reported about its size the last time it was drawn.
///
/// This is `egui_taffy`'s `Context`, the value the taffy measure function
/// turns into a size. A leaf that fills whatever it is given (`leaf_fill`)
/// reports `infinite`, which the measure function clamps to the root rect.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Measure {
    min_size: Vec2,
    max_size: Vec2,
    infinite: Vec2b,
}

/// One node of a tree, as looked up by its [`egui::Id`].
struct NodeSlot {
    node: NodeId,
    /// Was this node visited in the frame being drawn? Cleared by the sweep.
    keep: bool,
}

/// One taffy tree: everything a `<View>` root keeps between frames.
pub(crate) struct Tree {
    taffy: TaffyTree<Measure>,
    nodes: HashMap<egui::Id, NodeSlot>,
    /// The outermost node, as of the last finished frame.
    ///
    /// [`Tree::layout_first_on_resize`] needs it before the frame builds the
    /// tree again.
    root: Option<NodeId>,
    /// The size of the rect the tree was given when it was last laid out.
    last_size: Vec2,
    /// Was a node added during the frame being drawn?
    ///
    /// A new node drew in an invisible sizing pass, so it always needs a
    /// second pass, whatever the layout says.
    created_this_frame: bool,
    /// The pass number of the last [`Tree::visit`], for the store's sweep.
    last_visited: u64,
}

impl Tree {
    fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            nodes: HashMap::new(),
            root: None,
            last_size: Vec2::ZERO,
            created_this_frame: false,
            last_visited: 0,
        }
    }

    /// Mark the tree as drawn in `pass`.
    pub(crate) fn visit(&mut self, pass: u64) {
        self.last_visited = pass;
    }

    /// The pass this tree was last drawn in.
    pub(crate) fn last_visited(&self) -> u64 {
        self.last_visited
    }

    /// Add or reuse the node keyed by `key`, and put it at `child_index` under
    /// `parent`.
    ///
    /// Returns the node, the layout it was left with by the last computation
    /// (which is what its children and its own `Ui` are placed from), and
    /// whether it was created just now.
    fn add_child_node(
        &mut self,
        key: egui::Id,
        style: taffy::Style,
        parent: Option<NodeId>,
        child_index: usize,
    ) -> (NodeId, Layout, bool) {
        let Self { taffy, nodes, .. } = self;
        let mut first_frame = false;

        let node = match nodes.entry(key) {
            Entry::Occupied(mut entry) => {
                let slot = entry.get_mut();
                if slot.keep {
                    log::error!("egui-react: two layout nodes share the id {key:?}");
                }
                slot.keep = true;
                let node = slot.node;
                // Setting a style marks the node dirty, so only do it when the
                // style really changed.
                if taffy.style(node).unwrap() != &style {
                    taffy.set_style(node, style).unwrap();
                }
                node
            }
            Entry::Vacant(entry) => {
                first_frame = true;
                self.created_this_frame = true;
                let node = taffy.new_leaf(style).unwrap();
                entry.insert(NodeSlot { node, keep: true });
                node
            }
        };

        if let Some(parent) = parent {
            if child_index < taffy.child_count(parent) {
                if taffy.child_at_index(parent, child_index).unwrap() != node {
                    // The order changed. Removing the whole tail and rebuilding
                    // it is much cheaper than moving nodes one at a time.
                    let count = taffy.child_count(parent);
                    taffy
                        .remove_children_range(parent, child_index..count)
                        .unwrap();
                    taffy.add_child(parent, node).unwrap();
                }
            } else {
                taffy.add_child(parent, node).unwrap();
            }
        }

        (node, *taffy.layout(node).unwrap(), first_frame)
    }

    /// Drop the children of `node` past the `used` that were drawn this frame.
    fn trim_children(&mut self, node: NodeId, used: usize) {
        let count = self.taffy.child_count(node);
        if count > used {
            self.taffy.remove_children_range(node, used..count).unwrap();
        }
    }

    /// Record what a leaf measured. Only writes when it changed, because a
    /// write marks the node dirty.
    fn set_measure(&mut self, node: NodeId, measure: Measure) {
        if self.taffy.get_node_context(node) != Some(&measure) {
            self.taffy.set_node_context(node, Some(measure)).unwrap();
        }
    }

    /// The layout of `node` as of the last computation.
    fn layout(&self, node: NodeId) -> &Layout {
        self.taffy.layout(node).unwrap()
    }

    /// Compute the layout of the tree rooted at `node`.
    ///
    /// The measure function turns the size a leaf reported while it was drawn
    /// into the size taffy asks for. `root_size` is the size of the rect the
    /// whole tree was given; a leaf that says it is infinite in one direction
    /// is clamped to it. Ported from `egui_taffy` unchanged, so that existing
    /// layouts come out the same.
    fn compute(&mut self, node: NodeId, available_space: Size<AvailableSpace>, root_size: Vec2) {
        self.taffy
            .compute_layout_with_measure(
                node,
                available_space,
                |_known_size: Size<Option<f32>>,
                 available_space: Size<AvailableSpace>,
                 _id,
                 context,
                 _style|
                 -> Size<f32> {
                    let context = context.copied().unwrap_or(Measure {
                        min_size: Vec2::ZERO,
                        max_size: Vec2::ZERO,
                        infinite: Vec2b::FALSE,
                    });

                    let Measure {
                        mut min_size,
                        mut max_size,
                        infinite,
                    } = context;

                    if min_size.any_nan() {
                        min_size = Vec2::ZERO;
                    }
                    if max_size.any_nan() {
                        max_size = root_size;
                    }

                    let max_size = Vec2 {
                        x: if infinite.x { root_size.x } else { max_size.x },
                        y: if infinite.y { root_size.y } else { max_size.y },
                    };

                    let width = match available_space.width {
                        AvailableSpace::Definite(num) => {
                            num.clamp(min_size.x, max_size.x.max(min_size.x))
                        }
                        AvailableSpace::MinContent => min_size.x,
                        AvailableSpace::MaxContent => max_size.x,
                    };
                    let height = match available_space.height {
                        AvailableSpace::Definite(num) => {
                            num.clamp(min_size.y, max_size.y.max(min_size.y))
                        }
                        AvailableSpace::MinContent => min_size.y,
                        AvailableSpace::MaxContent => max_size.y,
                    };

                    Size { width, height }
                },
            )
            .unwrap();
    }

    /// Compute the layout before the frame draws, if only the root rect resized.
    ///
    /// The normal order is: draw every child at last frame's layout, collect
    /// what they measured, compute, ask egui for another pass. When the window
    /// resizes, that costs one pass per level of nesting: a tree inside a leaf
    /// of another tree learns its new root rect only after the outer tree has
    /// settled.
    ///
    /// Nothing forces that order when the only input that changed is the size
    /// of the rect the tree was given. The tree is still the one the last frame
    /// built and every leaf's measurement is still on its node, so the layout
    /// can be computed here, before the children are drawn again. They then
    /// draw at their new places in this pass, and nested trees pick their new
    /// root rect up in the same pass.
    ///
    /// Skipped when the tree is dirty: a measurement or a style changed, so
    /// the result would be thrown away by [`Tree::finish`] anyway.
    fn layout_first_on_resize(&mut self, root_rect: Rect, available_space: Size<AvailableSpace>) {
        let Some(root) = self.root else {
            // First frame of this tree: there is no layout to reuse.
            return;
        };
        if self.last_size == root_rect.size() {
            return;
        }
        if self.taffy.dirty(root).unwrap() {
            return;
        }
        self.compute(root, available_space, root_rect.size());
        self.last_size = root_rect.size();
    }

    /// Sweep the nodes nothing drew, recompute if needed, and ask egui for a
    /// second pass only if the frame on screen is now wrong.
    ///
    /// A recomputation does not always move anything. A tree recomputes when
    /// its root rect changed size or when any node became dirty, and both
    /// happen on frames where the result is the layout the nodes were already
    /// drawn with: a row inside a scroll area gets a shorter root rect on every
    /// scrolled frame, and a leaf whose text got wider still fits in the space
    /// its style gave it. So the layout of every node is copied before the
    /// computation and compared with the result.
    fn finish(
        &mut self,
        root: NodeId,
        root_rect: Rect,
        available_space: Size<AvailableSpace>,
        ctx: &egui::Context,
    ) {
        self.root = Some(root);

        let Self { taffy, nodes, .. } = self;
        let mut removed = false;
        nodes.retain(|_, slot| {
            if slot.keep {
                slot.keep = false;
                return true;
            }
            removed = true;
            if let Some(parent) = taffy.parent(slot.node) {
                taffy.remove_child(parent, slot.node).unwrap();
            }
            taffy.remove(slot.node).unwrap();
            false
        });

        let created = std::mem::take(&mut self.created_this_frame);

        if !self.taffy.dirty(root).unwrap() && self.last_size == root_rect.size() {
            return;
        }

        // The layout every node still in the tree was drawn with. "Drawn with"
        // stays true when `layout_first_on_resize` already computed the layout
        // at the top of the frame: every child read its rect after that
        // computation. It is taken here, not there, so that nodes created
        // during the frame are included.
        let old: Vec<(NodeId, Layout)> = self
            .nodes
            .values()
            .map(|slot| (slot.node, *self.taffy.layout(slot.node).unwrap()))
            .collect();

        self.last_size = root_rect.size();
        self.compute(root, available_space, root_rect.size());

        let taffy = &self.taffy;
        let moved = old.iter().any(|(node, old)| {
            let overflow = taffy.style(*node).unwrap().overflow;
            let content_size_matters = *node == root
                || overflow.x == taffy::Overflow::Scroll
                || overflow.y == taffy::Overflow::Scroll;
            layout_moved(old, taffy.layout(*node).unwrap(), content_size_matters)
        });

        // A removed node counts as a change: it was part of the tree the
        // surviving nodes were drawn with, and it is already out of the map
        // above, so the comparison cannot tell what dropping it did.
        if created || removed || moved {
            ctx.request_discard("egui-react: layout changed");
        }
    }
}

/// Did the computation change anything the node is drawn from?
///
/// Every field but `content_size` places the node, so a difference there is a
/// difference on screen. `content_size` is read in two places only: on the root
/// node, where it is the space the whole tree takes in the surrounding
/// [`egui::Ui`], and on a scrollable node, where it is the size of the scrolled
/// content. Anywhere else it is just what the node measured, and that changes
/// without moving anything — a label that got wider inside a node that grows to
/// fill its row is the usual case — so it must not cost a pass.
fn layout_moved(old: &Layout, new: &Layout, content_size_matters: bool) -> bool {
    let Layout {
        order,
        location,
        size,
        content_size,
        scrollbar_size,
        border,
        padding,
        margin,
    } = new;

    old.order != *order
        || old.location != *location
        || old.size != *size
        || old.scrollbar_size != *scrollbar_size
        || old.border != *border
        || old.padding != *padding
        || old.margin != *margin
        || (content_size_matters && old.content_size != *content_size)
}

#[inline]
fn sum_axis(rect: &taffy::Rect<f32>) -> Size<f32> {
    Size {
        width: rect.left + rect.right,
        height: rect.top + rect.bottom,
    }
}

#[inline]
fn top_left(rect: &taffy::Rect<f32>) -> taffy::Point<f32> {
    taffy::Point {
        x: rect.left,
        y: rect.top,
    }
}

/// Where a node's border box sits on screen, given its parent's.
#[inline]
fn border_box_min(layout: &Layout, origin: Pos2) -> Pos2 {
    origin + egui::vec2(layout.location.x, layout.location.y)
}

/// The rect a leaf draws into: its box minus its border and padding.
#[inline]
fn content_rect(layout: &Layout, origin: Pos2) -> Rect {
    let pos = layout.location + top_left(&layout.padding) + top_left(&layout.border);
    let size = layout.size - sum_axis(&layout.padding) - sum_axis(&layout.border);
    Rect::from_min_size(
        origin + egui::vec2(pos.x, pos.y),
        egui::vec2(size.width, size.height),
    )
}

/// Where a [`crate::Cx`] is while it is building one tree.
///
/// This is the tree-mode half of `Surface`. It is deliberately a plain borrow
/// bundle rather than a handle with interior state: `Cx::scope` has to be able
/// to hand a child `Cx` the same position in the tree, and the child index has
/// to keep counting across such a scope, so it is a `&mut usize` owned by the
/// enclosing `container` call rather than a field of the `Tree`. That way the
/// count is per parent node, which is what taffy's child order needs, and
/// nesting `scope` calls cannot restart it.
pub(crate) struct TreeCx<'u> {
    /// The tree being built. An `Rc`, not a borrow, so that a leaf can open a
    /// tree of its own without the outer tree's borrow standing in the way.
    tree: &'u Rc<RefCell<Tree>>,
    /// The one `Ui` of this tree. Every leaf is a child of it.
    root_ui: &'u mut egui::Ui,
    /// The node children are added to.
    parent: NodeId,
    /// Where `parent`'s border box sits on screen.
    origin: Pos2,
    /// How many children have been added under `parent` so far.
    child_index: &'u mut usize,
}

impl TreeCx<'_> {
    /// Re-borrow for a shorter lifetime, keeping the position in the tree.
    pub(crate) fn reborrow(&mut self) -> TreeCx<'_> {
        TreeCx {
            tree: self.tree,
            root_ui: self.root_ui,
            parent: self.parent,
            origin: self.origin,
            child_index: self.child_index,
        }
    }

    /// The tree's own `Ui`, which is what `cx.ui()` returns in tree mode.
    pub(crate) fn root_ui(&mut self) -> &mut egui::Ui {
        self.root_ui
    }

    /// The egui context.
    pub(crate) fn ctx(&self) -> &egui::Context {
        self.root_ui.ctx()
    }

    /// Take the next child slot under the current parent.
    fn next_index(&mut self) -> usize {
        let index = *self.child_index;
        *self.child_index += 1;
        index
    }

    /// Add a container node and build its children inside it.
    ///
    /// No `Ui` and no widget is created: a `<View>` is a rect, and egui-react
    /// paints nothing on it.
    pub(crate) fn container<R>(
        &mut self,
        key: egui::Id,
        style: taffy::Style,
        f: impl FnOnce(&mut TreeCx<'_>) -> R,
    ) -> R {
        let index = self.next_index();
        let (node, layout, _first_frame) =
            self.tree
                .borrow_mut()
                .add_child_node(key, style, Some(self.parent), index);
        let origin = border_box_min(&layout, self.origin);

        let mut used = 0usize;
        let inner = {
            let mut child = TreeCx {
                tree: self.tree,
                root_ui: self.root_ui,
                parent: node,
                origin,
                child_index: &mut used,
            };
            f(&mut child)
        };
        self.tree.borrow_mut().trim_children(node, used);
        inner
    }

    /// Add a leaf node and draw one egui `Ui` in the rect taffy gave it.
    ///
    /// `prefix` keys the node (it walks with the layout id, so a reused list
    /// slot keeps its nodes) and `scope` salts the `Ui` (it walks with the hook
    /// scope, so widget ids stay with the row). `measured` is false for
    /// `leaf_fill`, which reports no content size and is sized by taffy alone.
    pub(crate) fn leaf<R>(
        &mut self,
        prefix: egui::Id,
        scope: egui::Id,
        style: taffy::Style,
        measured: bool,
        f: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        let index = self.next_index();
        let (node, layout, first_frame) = self.tree.borrow_mut().add_child_node(
            prefix.with(index),
            style,
            Some(self.parent),
            index,
        );

        let mut builder = UiBuilder::new()
            .max_rect(content_rect(&layout, self.origin))
            .id_salt(scope.with(index));
        if first_frame {
            // A node that has never been laid out has a zero rect, so its
            // first draw is a measurement: invisible, and in a sizing pass so
            // that widgets ask for as little space as they can.
            builder = builder.sizing_pass().invisible();
        }
        let mut ui = self.root_ui.new_child(builder);
        let inner = f(&mut ui);

        let measure = if measured {
            let min_size = ui.min_size().ceil();
            Measure {
                min_size,
                // `egui_taffy` reports the raw size as the maximum and then
                // raises it to the rounded minimum, which comes to the same
                // number. Spelled out, because the two are one field each in
                // taffy and only their equality keeps the node clean.
                max_size: ui.min_size().max(min_size),
                infinite: Vec2b::FALSE,
            }
        } else {
            Measure {
                min_size: Vec2::ZERO,
                max_size: Vec2::INFINITY,
                infinite: Vec2b::TRUE,
            }
        };
        self.tree.borrow_mut().set_measure(node, measure);

        inner
    }
}

/// Draw one tree, from the `Ui` it sits in.
///
/// The frame goes: reserve space, compute early if only the rect resized, let
/// `f` build the tree, sweep what nothing drew, recompute and decide about the
/// second pass, then tell the surrounding `Ui` how much room the tree took.
///
/// `all_space` reserves both axes (the runner's root, so the outermost
/// `<View>` fills the window); otherwise only the width is reserved and the
/// height is left as max-content, so a column of nested containers stacks
/// instead of each one claiming the whole height.
pub(crate) fn show<R>(
    store: &Store,
    ui: &mut egui::Ui,
    id: egui::Id,
    style: taffy::Style,
    all_space: bool,
    f: impl FnOnce(&mut TreeCx<'_>) -> R,
) -> R {
    // The same reservation `egui_taffy`'s `TuiInitializer` makes: a definite
    // width taken from the space that is left, and a minimum width on the `Ui`
    // so the surrounding layout knows about it. The height is `MinContent`
    // unless the caller asks for all the space.
    let width = ui.available_size().x;
    ui.set_min_width(width);
    let mut available_space = Size {
        width: AvailableSpace::Definite(width),
        height: AvailableSpace::MinContent,
    };
    if all_space {
        let height = ui.available_size().y;
        ui.set_min_height(height);
        available_space.height = AvailableSpace::Definite(height);
    }
    let root_rect = ui.available_rect_before_wrap();
    let ctx = ui.ctx().clone();

    let tree = store.tree(id);
    // A child `Ui`, as `egui_taffy` does, rather than the caller's own: this is
    // what `cx.ui()` hands out, and a `<Panel>` carves its space out of it.
    let mut root_ui = ui.new_child(UiBuilder::new());

    tree.borrow_mut()
        .layout_first_on_resize(root_rect, available_space);

    // The root node carries the container style the caller passed. It has no
    // parent, so it is never reordered, and its key is the tree's own id.
    let (root, root_layout, _first_frame) = tree.borrow_mut().add_child_node(id, style, None, 0);

    let mut used = 0usize;
    let inner = {
        let mut tc = TreeCx {
            tree: &tree,
            root_ui: &mut root_ui,
            parent: root,
            origin: border_box_min(&root_layout, root_rect.min),
            child_index: &mut used,
        };
        f(&mut tc)
    };

    let content_size = {
        let mut tree = tree.borrow_mut();
        // Before the sweep, unlike `egui_taffy`, which trims the root's tail
        // after it recomputes and so lays out one frame with the stale tail
        // still attached. Every other node is trimmed before, and doing the
        // same here is one rule instead of two.
        tree.trim_children(root, used);
        tree.finish(root, root_rect, available_space, &ctx);
        tree.layout(root).content_size
    };
    ui.allocate_space(egui::vec2(content_size.width, content_size.height));

    inner
}

/// Build an empty tree. Only [`Store`] calls this.
pub(crate) fn new_tree() -> Rc<RefCell<Tree>> {
    Rc::new(RefCell::new(Tree::new()))
}
