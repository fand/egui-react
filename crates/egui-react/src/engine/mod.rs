//! The layout engine: one taffy tree per `<View>` root, drawn in one egui pass.
//!
//! This is what egui-react uses instead of `egui_taffy`. The behaviour is the
//! same — the measure function, the node reuse rule, the sweep and the two
//! pass protocol are ports of `egui_taffy` 0.14 with the fork's two fixes
//! (skip the discard when the layout did not move; compute the layout before
//! drawing when only the root rect resized) built in. What is gone is the
//! per-node egui `Ui`: a node here is a rect, not a `Ui`, and only a leaf gets
//! a `Ui` of its own. A node's look is not a `Ui` either: the engine paints
//! what its [`PaintStyle`] says — a shadow, a background, a border — straight
//! onto the tree's own `Ui`, into shape slots claimed in draw order and filled
//! once the layout for this frame is final (see [`Tree::paint_boxes`]). So a
//! `<View>` with a background is still a rect, and a row costs three `Ui`s
//! instead of nine.
//!
//! A `<Text>` does not even cost that: its galley is laid out inside the taffy
//! measure function and painted onto the tree's own `Ui` (see [`TextCtx`] and
//! [`Tree::paint_texts`]), so a row of `<View>` and `<Text>` opens one `Ui` for
//! the whole tree and is right on its first frame.
//!
//! One tree per root lives in the [`Store`], not in egui memory: one map
//! lookup per frame, and a tree left behind by an unmounted subtree is dropped
//! by [`Store::end_pass`] instead of growing egui's `IdTypeMap` forever.
//!
//! There is a second, smaller path beside this one: [`lite`], which lays a
//! `<VirtualList>` row out with a flex solver of its own instead of a taffy
//! tree. It reuses this module's measure function, its per pass galley reuse
//! and its `<Text>` painting, so the two agree on everything but who solves the
//! boxes.

pub(crate) mod lite;

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::rc::Rc;
use std::sync::Arc;

use egui::layers::ShapeIdx;
use egui::text::LayoutJob;
use egui::{Galley, Pos2, Rect, Shape, UiBuilder, Vec2, Vec2b};
use taffy::{AvailableSpace, Layout, NodeId, Size, TaffyTree, TraversePartialTree as _};

use crate::paint::PaintStyle;
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

impl Measure {
    /// What a node with nothing to measure reports.
    ///
    /// This is the `None` arm of the measure function: a container that ended
    /// up with no children is laid out as a leaf, and it has no measurement of
    /// its own, so it is zero whatever it is asked.
    const EMPTY: Self = Self {
        min_size: Vec2::ZERO,
        max_size: Vec2::ZERO,
        infinite: Vec2b::FALSE,
    };

    /// What a `leaf_fill` reports: no content size, so the layout alone sizes
    /// it, clamped to the root rect.
    const FILL: Self = Self {
        min_size: Vec2::ZERO,
        max_size: Vec2::INFINITY,
        infinite: Vec2b::TRUE,
    };
}

/// Turn what a leaf reported while it was drawn into the size the layout asks
/// for.
///
/// This is `egui_taffy`'s measure function, ported unchanged. Both layout paths
/// call it, so a leaf is the same size whichever one lays its row out.
/// `root_size` is the size of the rect the whole tree was given; a leaf that
/// says it is infinite in one direction is clamped to it.
fn measure_leaf(
    measure: Measure,
    available_space: Size<AvailableSpace>,
    root_size: Vec2,
) -> Size<f32> {
    let Measure {
        mut min_size,
        mut max_size,
        infinite,
    } = measure;

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
        AvailableSpace::Definite(num) => num.clamp(min_size.x, max_size.x.max(min_size.x)),
        AvailableSpace::MinContent => min_size.x,
        AvailableSpace::MaxContent => max_size.x,
    };
    let height = match available_space.height {
        AvailableSpace::Definite(num) => num.clamp(min_size.y, max_size.y.max(min_size.y)),
        AvailableSpace::MinContent => min_size.y,
        AvailableSpace::MaxContent => max_size.y,
    };

    Size { width, height }
}

/// What taffy is told about a node that has no children of its own.
enum NodeCtx {
    /// A leaf that drew itself into a `Ui` and reported what that `Ui` took.
    Leaf(Measure),
    /// A `<Text>`: laid out by the measure function, not by drawing it first.
    Text(TextCtx),
}

/// A `<Text>` node: the job to lay out, and the galley of the pass being drawn.
///
/// The galley is built inside the taffy measure function, so a new `<Text>`
/// needs no invisible sizing pass: taffy asks for its size and gets the real
/// one in the same pass it was added. The paint step then reuses the same
/// galley.
///
/// What survives between passes is the [`LayoutJob`], which depends on nothing
/// but the text and the style. The galley does not: it holds texture
/// coordinates into the glyph atlas of the `Fonts` that laid it out, and egui
/// throws that `Fonts` away and builds a new one with a new atlas after
/// `set_fonts`, after the text options changed (`Visuals::dark` and
/// `Visuals::light` rasterize glyphs differently) and when the atlas gets full,
/// with no way to ask whether it just did. So the galley is only ever reused
/// within one pass, and across passes it comes from epaint's own `GalleyCache`,
/// which lives inside `Fonts` and is rebuilt with the atlas: a rebuilt atlas
/// can never meet a stale galley. The cost is a `LayoutJob` clone, a hash and a
/// lookup per `<Text>` per frame — what `egui::Label` pays.
struct TextCtx {
    /// The layout job [`egui::Label`] would have built in a `Ui` of this node,
    /// with `wrap.max_width` left for each layout to fill in.
    job: Arc<LayoutJob>,
    /// A hash of `job`, so the per frame "did the text change?" test is one
    /// integer compare rather than a string compare.
    hash: u64,
    /// Wrap to the width the node is given, or run on (`Extend`)?
    wrap: bool,
    /// This pass's galley: the wrap width and the pixels per point it was laid
    /// out for (both as bits, so they compare exactly), the pass it was laid
    /// out in, and the galley. Reused within the pass — measure to paint, and
    /// on the placed path the widget rect to the paint — and never past it.
    galley: Option<(u32, u32, u64, Arc<Galley>)>,
}

impl TextCtx {
    /// The galley for `wrap_width`, laid out again unless this pass already did.
    fn galley(&mut self, fonts: Fonts<'_>, wrap_width: f32) -> Arc<Galley> {
        let key = (
            wrap_width.to_bits(),
            fonts.pixels_per_point.to_bits(),
            fonts.pass,
        );
        if let Some((width, ppp, pass, galley)) = &self.galley
            && (*width, *ppp, *pass) == key
        {
            return Arc::clone(galley);
        }
        let mut job = LayoutJob::clone(&self.job);
        job.wrap.max_width = wrap_width;
        // `fonts_mut`, as `WidgetText::into_galley_impl` does: a font used for
        // the first time has to be loaded before the text can be laid out.
        // epaint's own galley cache is behind this, so a miss here is a hash
        // and a lookup, not a re-layout, and it hands back the very same `Arc`
        // while the `Fonts` that made it lives.
        let galley = fonts.ctx.fonts_mut(|fonts| fonts.layout_job(job));
        self.galley = Some((key.0, key.1, key.2, Arc::clone(&galley)));
        galley
    }
}

/// What laying a galley out needs.
///
/// The pixels per point and the pass number are carried rather than read from
/// the context, because reading them takes egui's lock and this is on the per
/// node path.
#[derive(Clone, Copy)]
struct Fonts<'a> {
    ctx: &'a egui::Context,
    pixels_per_point: f32,
    /// egui's cumulative pass number, which is what a kept galley is keyed on:
    /// it is reused within the pass that made it and dropped after. Nothing
    /// here tries to tell one `Fonts` from the next — epaint's `GalleyCache`
    /// does that by dying with its atlas.
    pass: u64,
}

impl<'a> Fonts<'a> {
    /// Read from a `Ui`, which keeps the pixels per point on its painter.
    fn of(ui: &'a egui::Ui) -> Self {
        Self {
            ctx: ui.ctx(),
            pixels_per_point: ui.pixels_per_point(),
            pass: ui.ctx().cumulative_pass_nr(),
        }
    }
}

/// A `<Text>` whose shape is reserved in the painter and filled in once the
/// layout for this frame is final.
///
/// Every other leaf draws where the last frame put it and asks for a second
/// pass when that turns out to be wrong. A galley needs no `Ui` and no
/// drawing, so it does not have to: the shape's slot is claimed in draw order,
/// and the text goes into it after the layout is computed. That is what lets a
/// tree of `<View>` and `<Text>` alone be right on its very first frame.
struct PendingText {
    node: NodeId,
    idx: ShapeIdx,
    color: egui::Color32,
    wrap: bool,
    /// The `Ui`'s opacity where the slot was claimed.
    ///
    /// `Painter::set` applies the painter's opacity when the shape is set, and
    /// by then the tree's `Ui` is back at whatever it was outside this node. So
    /// the factor that was in force here is recorded and put back on a painter
    /// of our own in [`Tree::paint_texts`].
    opacity: f32,
}

/// A node's box — shadow, background, border — waiting for the final layout.
///
/// The look of a node needs its rect, and a rect is only known once taffy has
/// solved the frame. So the shapes are not painted where they are declared:
/// two slots are claimed in draw order, one before the node's children or
/// widget and one after them, and [`Tree::paint_boxes`] fills both from the
/// layout of *this* frame. Deferring always, rather than painting a node that
/// already has a layout right away, is one code path instead of two, and it is
/// what makes a container that grew around children which stayed put right in
/// the very pass it grew in.
struct PendingPaint {
    node: NodeId,
    /// The slot behind the children: the shadow and the background.
    bg_idx: ShapeIdx,
    /// The slot in front of them, claimed only when there is a border.
    border_idx: Option<ShapeIdx>,
    paint: PaintStyle,
    /// The opacity in force inside the node, as for [`PendingText`].
    opacity: f32,
}

/// One node of a tree, as looked up by its [`egui::Id`].
struct NodeSlot {
    node: NodeId,
    /// Was this node visited in the frame being drawn? Cleared by the sweep.
    keep: bool,
    /// Was this node added during the frame being drawn?
    ///
    /// Such a node has no layout it was drawn with — taffy leaves a new node
    /// at zero — so it takes no part in the "did anything move?" comparison.
    fresh: bool,
}

/// One taffy tree: everything a `<View>` root keeps between frames.
pub(crate) struct Tree {
    taffy: TaffyTree<NodeCtx>,
    nodes: HashMap<egui::Id, NodeSlot>,
    /// The outermost node, as of the last finished frame.
    ///
    /// [`Tree::layout_first_on_resize`] needs it before the frame builds the
    /// tree again.
    root: Option<NodeId>,
    /// The size of the rect the tree was given when it was last laid out.
    last_size: Vec2,
    /// Was a leaf added during the frame being drawn?
    ///
    /// A new leaf drew in an invisible sizing pass, so it always needs a
    /// second pass, whatever the layout says. Containers and `<Text>` nodes do
    /// not set this: a container draws nothing of its own and a `<Text>` is
    /// measured rather than drawn, so for those the layout comparison in
    /// [`Tree::finish`] is the whole story.
    created_this_frame: bool,
    /// The `<Text>` shapes claimed this frame, waiting for the final layout.
    texts: Vec<PendingText>,
    /// The box shapes claimed this frame, waiting for the final layout.
    paints: Vec<PendingPaint>,
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
            texts: Vec::new(),
            paints: Vec::new(),
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
                let node = taffy.new_leaf(style).unwrap();
                entry.insert(NodeSlot {
                    node,
                    keep: true,
                    fresh: true,
                });
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
        let same = matches!(self.taffy.get_node_context(node), Some(NodeCtx::Leaf(old)) if *old == measure);
        if !same {
            self.taffy
                .set_node_context(node, Some(NodeCtx::Leaf(measure)))
                .unwrap();
        }
    }

    /// Record the text of a `<Text>` node, keeping what it holds when neither
    /// the job nor the wrap mode changed. Only writes when it changed, because
    /// a write marks the node dirty.
    fn set_text(&mut self, node: NodeId, job: Arc<LayoutJob>, hash: u64, wrap: bool) {
        let same = matches!(
            self.taffy.get_node_context(node),
            Some(NodeCtx::Text(old)) if old.hash == hash && old.wrap == wrap
        );
        if !same {
            self.taffy
                .set_node_context(
                    node,
                    Some(NodeCtx::Text(TextCtx {
                        job,
                        hash,
                        wrap,
                        galley: None,
                    })),
                )
                .unwrap();
        }
    }

    /// The galley of a `<Text>` node at `wrap_width`, reused if this pass
    /// already laid it out. Only [`Tree::paint_texts`] calls this outside the
    /// measure function.
    fn text_galley(
        &mut self,
        node: NodeId,
        fonts: Fonts<'_>,
        wrap_width: f32,
    ) -> Option<Arc<Galley>> {
        match self.taffy.get_node_context_mut(node) {
            Some(NodeCtx::Text(text)) => Some(text.galley(fonts, wrap_width)),
            _ => None,
        }
    }

    /// Where a node's content rect sits on screen, walking up to the root.
    ///
    /// [`TreeCx`] carries the origin down while the frame is drawn, but the
    /// text shapes are filled in afterwards, from the layout as computed.
    fn content_rect_of(&self, node: NodeId, root_min: Pos2) -> Rect {
        content_rect(
            self.taffy.layout(node).unwrap(),
            self.origin_of(node, root_min),
        )
    }

    /// Where a node's whole box sits on screen, walking up to the root.
    ///
    /// This is what the paint uses, not [`Tree::content_rect_of`]: a background
    /// covers the node's padding and sits under its border, and the border sits
    /// in the band the layout reserved for it.
    fn border_box_of(&self, node: NodeId, root_min: Pos2) -> Rect {
        let layout = self.taffy.layout(node).unwrap();
        let origin = self.origin_of(node, root_min);
        Rect::from_min_size(
            border_box_min(layout, origin),
            egui::vec2(layout.size.width, layout.size.height),
        )
    }

    /// Where the parent of `node` has its border box, on screen.
    ///
    /// [`TreeCx`] carries this down while the frame is drawn, but the deferred
    /// shapes are filled in afterwards, from the layout as computed.
    fn origin_of(&self, node: NodeId, root_min: Pos2) -> Pos2 {
        let mut origin = Vec2::ZERO;
        let mut parent = self.taffy.parent(node);
        while let Some(node) = parent {
            let location = self.taffy.layout(node).unwrap().location;
            origin += egui::vec2(location.x, location.y);
            parent = self.taffy.parent(node);
        }
        root_min + origin
    }

    /// Paint the box of every node that claimed a slot this frame.
    ///
    /// Runs after the layout is final, so a node's look is never a frame behind
    /// its rect — not even on the frame it was created in.
    fn paint_boxes(&mut self, ui: &egui::Ui, root_min: Pos2) {
        if self.paints.is_empty() {
            return;
        }
        // A painter of our own, because `Painter::set` applies the painter's
        // opacity as the shape is set and the `Ui` is long back at the opacity
        // it had outside these nodes.
        let mut painter = ui.painter().clone();
        let visuals = ui.visuals();
        for pending in std::mem::take(&mut self.paints) {
            let rect = self.border_box_of(pending.node, root_min);
            painter.set_opacity(pending.opacity);

            // One slot holds both, in the order they are drawn.
            let behind: Vec<Shape> = pending
                .paint
                .shadow_shape(rect, visuals)
                .into_iter()
                .chain(pending.paint.bg_shape(rect))
                .collect();
            if !behind.is_empty() {
                painter.set(pending.bg_idx, Shape::Vec(behind));
            }

            if let Some(idx) = pending.border_idx
                && let Some(border) = pending.paint.border_shape(rect)
            {
                painter.set(idx, border);
            }
        }
    }

    /// Put every `<Text>` claimed this frame into the shape it reserved.
    ///
    /// Runs after the layout is final, so the galley is painted where the node
    /// ended up rather than where it was last frame.
    fn paint_texts(&mut self, ui: &egui::Ui, root_min: Pos2) {
        if self.texts.is_empty() {
            return;
        }
        let fonts = Fonts::of(ui);
        // A painter of our own, for the opacity, as in [`Tree::paint_boxes`].
        let mut painter = ui.painter().clone();
        for pending in std::mem::take(&mut self.texts) {
            let rect = self.content_rect_of(pending.node, root_min);
            let width = wrap_width(&rect, pending.wrap);
            let Some(galley) = self.text_galley(pending.node, fonts, width) else {
                continue;
            };
            painter.set_opacity(pending.opacity);
            let pos = galley_pos(&rect, &galley);
            painter.set(
                pending.idx,
                egui::epaint::TextShape::new(pos, galley, pending.color),
            );
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
    fn compute(
        &mut self,
        node: NodeId,
        available_space: Size<AvailableSpace>,
        root_size: Vec2,
        fonts: Fonts<'_>,
    ) {
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
                    // A `<Text>` is laid out here rather than measured after
                    // drawing: the galley is what taffy is asking about, and
                    // building it needs no `Ui`.
                    if let Some(NodeCtx::Text(text)) = context {
                        let wrap_width = if text.wrap {
                            match available_space.width {
                                AvailableSpace::Definite(width) => width,
                                AvailableSpace::MinContent => 0.0,
                                AvailableSpace::MaxContent => f32::INFINITY,
                            }
                        } else {
                            f32::INFINITY
                        };
                        // `ceil`, as the leaf path does, so that a node holding
                        // text is the size `egui::Label` in a leaf reported.
                        let size = text.galley(fonts, wrap_width).size().ceil();
                        return Size {
                            width: size.x,
                            height: size.y,
                        };
                    }

                    let context = match context {
                        Some(NodeCtx::Leaf(measure)) => *measure,
                        _ => Measure::EMPTY,
                    };

                    measure_leaf(context, available_space, root_size)
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
    fn layout_first_on_resize(
        &mut self,
        root_rect: Rect,
        available_space: Size<AvailableSpace>,
        fonts: Fonts<'_>,
    ) {
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
        self.compute(root, available_space, root_rect.size(), fonts);
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
        fonts: Fonts<'_>,
    ) {
        self.root = Some(root);

        // Whether the layout will be recomputed, decided before the sweep,
        // because the sweep itself dirties the tree. Nothing the sweep is about
        // to drop can make this wrong: a node it drops was already detached
        // from its parent by `trim_children` while the frame was drawn, and
        // that dirties the tree.
        let compute = self.taffy.dirty(root).unwrap() || self.last_size != root_rect.size();

        // The layout every node still in the tree was drawn with. "Drawn with"
        // stays true when `layout_first_on_resize` already computed the layout
        // at the top of the frame: every child read its rect after that
        // computation. Nodes added during this frame are left out: taffy leaves
        // a new node at zero, so comparing that with the computed layout would
        // report a move for every one of them. A new node that *drew* is
        // covered by `created` instead.
        let mut old: Vec<(NodeId, Layout)> = Vec::new();

        let Self { taffy, nodes, .. } = self;
        let mut removed = false;
        nodes.retain(|_, slot| {
            if slot.keep {
                slot.keep = false;
                let fresh = std::mem::replace(&mut slot.fresh, false);
                if compute && !fresh {
                    old.push((slot.node, *taffy.layout(slot.node).unwrap()));
                }
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

        if !compute {
            return;
        }

        self.last_size = root_rect.size();
        self.compute(root, available_space, root_rect.size(), fonts);

        let taffy = &self.taffy;
        let moved = old.iter().find(|(node, old)| {
            let new = taffy.layout(*node).unwrap();
            let overflow = taffy.style(*node).unwrap().overflow;
            let content_size_matters =
                overflow.x == taffy::Overflow::Scroll || overflow.y == taffy::Overflow::Scroll;
            match taffy.get_node_context(*node) {
                // A `<Text>` is painted from its galley's anchor, so only the
                // anchor and, when it wraps, the width it wraps at can put the
                // paint in the wrong place. Its size is what it measured, and
                // a label that changes every frame must not cost a pass.
                Some(NodeCtx::Text(text)) => text_moved(
                    &content_rect(old, Pos2::ZERO),
                    &content_rect(new, Pos2::ZERO),
                    text.job.halign,
                    text.wrap,
                ),
                // A widget drew a `Ui` in its whole content rect.
                Some(NodeCtx::Leaf(_)) => layout_moved(old, new, content_size_matters),
                // A container is judged by its location alone. Its children are
                // placed relative to it, so a shift of the container shifts
                // them, and that is what counts; a container that got wider or
                // narrower around children that stayed put changes nothing on
                // screen. Its own box does not change that: the shadow, the
                // background and the border are filled in from the layout
                // computed just above, in this very pass, so they are never a
                // frame behind the size they are painted at. A row
                // that shrinks to fit a label that changes every frame is the
                // usual case. The root included: the space the tree takes in
                // the surrounding `Ui` is read off its `content_size` after
                // this, in `show`, so it is never a frame behind.
                None => old.location != new.location,
            }
        });

        // A removed node counts as a change: it was part of the tree the
        // surviving nodes were drawn with, and it is already out of the map
        // above, so the comparison cannot tell what dropping it did.
        if created || removed || moved.is_some() {
            let reason = discard_reason(
                "layout",
                created,
                removed,
                moved.map(|(node, old)| {
                    (
                        describe_node(taffy.get_node_context(*node)),
                        content_rect(old, Pos2::ZERO),
                        content_rect(taffy.layout(*node).unwrap(), Pos2::ZERO),
                    )
                }),
            );
            log::debug!("egui-react: request_discard: {reason}");
            fonts.ctx.request_discard(reason);
        }
    }
}

/// Did the computation change anything a widget node is drawn from? (`<Text>`
/// nodes are judged by [`text_moved`] and containers by their location alone;
/// see `finish`.)
///
/// Every field but `content_size` places the node, so a difference there is a
/// difference on screen. `content_size` is read on a scrollable node only,
/// where it is the size of the scrolled content. Anywhere else it is just what
/// the node measured, and that changes without moving anything — a label that
/// got wider inside a node that grows to fill its row is the usual case — so it
/// must not cost a pass.
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

/// Did the computation move where a `<Text>` is painted?
///
/// A galley is painted from its anchor ([`galley_pos`]): the top of the node's
/// content rect and the edge its `halign` names. The node's own size is what
/// the text measured, so a text that got wider or narrower paints just as well
/// from the same anchor and costs no pass. A wrapped text is laid out at the
/// rect's width, so for one of those the width counts too.
fn text_moved(old: &Rect, new: &Rect, halign: egui::Align, wrap: bool) -> bool {
    anchor(old, halign) != anchor(new, halign) || (wrap && old.width() != new.width())
}

/// The reason handed to `request_discard`, naming what changed. Built only when
/// a discard is asked for, so a settled frame formats nothing. egui shows it in
/// its `PERF WARNING` overlay when discards run for three frames or more, and
/// the engine logs it at `debug` (`RUST_LOG=egui_react=debug`).
fn discard_reason(
    what: &str,
    created: bool,
    removed: bool,
    moved: Option<(String, Rect, Rect)>,
) -> String {
    use std::fmt::Write as _;
    let mut reason = format!("egui-react: {what} changed:");
    if created {
        reason.push_str(" a widget drew for the first time;");
    }
    if removed {
        reason.push_str(" a node was removed;");
    }
    if let Some((node, old, new)) = moved {
        write!(reason, " {node} moved {old:?} -> {new:?};").ok();
    }
    reason.pop();
    reason
}

/// What a taffy-path node is, for [`discard_reason`].
fn describe_node(ctx: Option<&NodeCtx>) -> String {
    match ctx {
        Some(NodeCtx::Text(text)) => describe_text(&text.job.text),
        Some(NodeCtx::Leaf(_)) => String::from("a widget"),
        None => String::from("a container"),
    }
}

/// A `<Text>` for [`discard_reason`]: its first characters, so the label that
/// caused a pass can be found in the source.
fn describe_text(text: &str) -> String {
    const MAX: usize = 24;
    let mut shown: String = text.chars().take(MAX).collect();
    if text.chars().count() > MAX {
        shown.push('…');
    }
    format!("the text {shown:?}")
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
    /// Is `origin` a place a layout computation actually put `parent`?
    ///
    /// False while any node between the tree root and here was created during
    /// this frame: taffy leaves a new node at zero, so everything below it is
    /// drawn at a position this frame's computation is about to change. A
    /// `<Text>` reads this to decide whether it may paint itself right away
    /// (see [`TreeCx::text`]).
    placed: bool,
    /// Is this position inside a `display="none"` subtree?
    ///
    /// Everything under such a node is laid out at zero by taffy, so its
    /// leaves draw invisible, out of the accessibility tree, and its texts
    /// register nothing at all.
    hidden: bool,
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
            placed: self.placed,
            hidden: self.hidden,
            child_index: self.child_index,
        }
    }

    /// Is this position inside a `display="none"` subtree?
    pub(crate) fn hidden(&self) -> bool {
        self.hidden
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

    /// Claim the slot a node's shadow and background go into, if it has any.
    ///
    /// The slot is taken here, in draw order, so that whatever the node draws
    /// next covers it; what goes in it is only known once the layout is final
    /// (see [`PendingPaint`]). `hidden` is the *node's* own flag, not this
    /// position's: a `display="none"` container hides itself as well as its
    /// children, and a hidden node claims nothing.
    fn claim_bg(&mut self, paint: PaintStyle, hidden: bool) -> Option<ShapeIdx> {
        (!hidden && !paint.is_none()).then(|| self.root_ui.painter().add(Shape::Noop))
    }

    /// Claim the border slot in front of the node and record both slots.
    ///
    /// `opacity` is the factor that was in force *inside* the node, which is
    /// what its own box is painted with.
    fn claim_border(&mut self, node: NodeId, paint: PaintStyle, bg_idx: ShapeIdx, opacity: f32) {
        let border_idx = paint
            .border
            .is_some()
            .then(|| self.root_ui.painter().add(Shape::Noop));
        self.tree.borrow_mut().paints.push(PendingPaint {
            node,
            bg_idx,
            border_idx,
            paint,
            opacity,
        });
    }

    /// Add a container node and build its children inside it.
    ///
    /// No `Ui` and no widget is created: a `<View>` is a rect. What `paint`
    /// asks for is painted around the children — the shadow and the background
    /// behind them, the border in front — from the layout of this frame.
    pub(crate) fn container<R>(
        &mut self,
        key: egui::Id,
        style: taffy::Style,
        paint: PaintStyle,
        f: impl FnOnce(&mut TreeCx<'_>) -> R,
    ) -> R {
        let index = self.next_index();
        // The node is still added and `f` still runs: keys and child indices
        // stay stable, so a show after a hide is not a "created" pass, and the
        // components inside keep their hooks.
        let hidden = self.hidden || style.display == taffy::Display::None;
        let (node, layout, first_frame) =
            self.tree
                .borrow_mut()
                .add_child_node(key, style, Some(self.parent), index);
        let origin = border_box_min(&layout, self.origin);

        let bg_idx = self.claim_bg(paint, hidden);
        // A container has no `Ui` of its own, so its opacity goes on the tree's
        // `Ui` around the children: every leaf `Ui` is a child of it and
        // inherits the factor, and so does a tree one of them opens.
        let outer_opacity = self.root_ui.opacity();
        if bg_idx.is_some()
            && let Some(opacity) = paint.opacity
        {
            self.root_ui.multiply_opacity(opacity);
        }
        let opacity = self.root_ui.opacity();

        let mut used = 0usize;
        let inner = {
            let mut child = TreeCx {
                tree: self.tree,
                root_ui: self.root_ui,
                parent: node,
                origin,
                placed: self.placed && !first_frame,
                hidden,
                child_index: &mut used,
            };
            f(&mut child)
        };
        self.tree.borrow_mut().trim_children(node, used);

        if let Some(bg_idx) = bg_idx {
            self.claim_border(node, paint, bg_idx, opacity);
            self.root_ui.set_opacity(outer_opacity);
        }
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
        paint: PaintStyle,
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

        // Behind the widget, which draws next into a `Ui` of its own.
        let hidden = self.hidden;
        let bg_idx = self.claim_bg(paint, hidden);

        let mut builder = UiBuilder::new()
            .max_rect(content_rect(&layout, self.origin))
            .id_salt(scope.with(index));
        if first_frame || self.hidden {
            // A node that has never been laid out has a zero rect, so its
            // first draw is a measurement: invisible, and in a sizing pass so
            // that widgets ask for as little space as they can. That is the
            // one reason a frame always needs a second pass, so it is recorded
            // here rather than wherever a node happens to be created. A hidden
            // leaf draws the same way, but nobody waits for its size, so it is
            // not a reason for a second pass.
            builder = builder.sizing_pass().invisible();
            if first_frame && !self.hidden {
                self.tree.borrow_mut().created_this_frame = true;
            }
        }
        let mut ui = self.root_ui.new_child(builder);
        // A leaf has a `Ui` of its own, so its opacity goes there: the widget
        // and any tree it opens inherit it, and the tree's `Ui` is left alone.
        if bg_idx.is_some()
            && let Some(opacity) = paint.opacity
        {
            ui.multiply_opacity(opacity);
        }
        let opacity = ui.opacity();
        if self.hidden {
            // Every widget `f` registers hangs from the `Ui`'s own id
            // (`Ui::interact` -> `register_accesskit_parent`), so one hidden
            // node here hides the whole subtree from assistive technology:
            // `accesskit_consumer::common_filter` excludes a hidden node with
            // everything under it. egui registers the widgets whether they are
            // visible or not (5.8), so this is the one way to keep them out.
            ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
                node.set_role(egui::accesskit::Role::GenericContainer);
                node.set_hidden();
            });
        }
        let inner = f(&mut ui);

        if self.hidden {
            // taffy lays a `Display::None` subtree out at zero whatever the
            // measure says, and the measure from the last visible draw is
            // what the node needs when it comes back.
            return inner;
        }

        // In front of the widget.
        if let Some(bg_idx) = bg_idx {
            self.claim_border(node, paint, bg_idx, opacity);
        }

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
            Measure::FILL
        };
        self.tree.borrow_mut().set_measure(node, measure);

        inner
    }

    /// Add a `<Text>` node: a galley, a widget rect and nothing else.
    ///
    /// This is what [`crate::Cx::text`] does instead of putting an
    /// `egui::Label` in a [`TreeCx::leaf`]. The calls `Label` makes that matter
    /// outside the picture — the widget rect, the `WidgetInfo` and the text
    /// selection state — are made here as well, so hover, `egui_kittest` label
    /// queries, screen readers and dragging to select all still work. What is
    /// skipped is the child `Ui` and laying the galley out twice.
    ///
    /// `selectable` follows `egui::Label::selectable`: `None` means the style's
    /// `interaction.selectable_labels`.
    ///
    /// There are two paint paths, and which one runs depends on whether the
    /// node's place on screen is already known:
    ///
    /// - **Placed** (this node and every node above it existed before this
    ///   frame): the galley is painted here, in draw order, through
    ///   `LabelSelectionState::label_text_selection`, which adds the shape
    ///   itself. If the layout computed at the end of the frame moves the node
    ///   after all, that counts as a move and the frame is discarded and drawn
    ///   again, so the paint is never left in the wrong place.
    /// - **Not placed** (created this frame, or under a container created this
    ///   frame): taffy leaves a new node at zero, so there is no position to
    ///   paint at yet. The shape's slot is claimed with a `Noop` and filled in
    ///   by [`Tree::paint_texts`] once the layout is final. That is what lets a
    ///   new `<View>` / `<Text>` tree be right on its first frame; the cost is
    ///   that the text is not selectable for that one frame.
    ///
    /// A non-selectable `<Text>` always takes the second path: it is cheaper,
    /// and it needs nothing from the selection state.
    // The arguments are `Cx::text`'s own plus the two ids and the two styles
    // every node here takes; bundling them would only move the list.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn text(
        &mut self,
        prefix: egui::Id,
        scope: egui::Id,
        style: taffy::Style,
        paint: PaintStyle,
        text: egui::WidgetText,
        wrap: bool,
        selectable: Option<bool>,
    ) -> egui::Response {
        let index = self.next_index();
        let (node, layout, first_frame) = self.tree.borrow_mut().add_child_node(
            prefix.with(index),
            style,
            Some(self.parent),
            index,
        );
        let content = content_rect(&layout, self.origin);

        let (job, hash) = text_job(self.root_ui, text);
        if self.hidden {
            // The galley cache and the node's text survive, so a show costs no
            // more than a visible frame. Nothing else is registered: no
            // `WidgetInfo` (so no accesskit node), no selection state, no shape
            // and no pending text. The rect nothing can hit is what a caller
            // reading the `Response` sees.
            self.tree
                .borrow_mut()
                .set_text(node, Arc::clone(&job), hash, wrap);
            return self
                .root_ui
                .interact(Rect::NOTHING, scope.with(index), egui::Sense::hover());
        }
        // Behind the galley, whichever paint path it takes below.
        let bg_idx = self.claim_bg(paint, self.hidden);
        let outer_opacity = self.root_ui.opacity();
        if bg_idx.is_some()
            && let Some(opacity) = paint.opacity
        {
            // A `<Text>` has no `Ui` either, so its opacity rides on the tree's
            // `Ui` while the galley is registered and painted.
            self.root_ui.multiply_opacity(opacity);
        }
        let opacity = self.root_ui.opacity();

        let fonts = Fonts::of(self.root_ui);
        let galley = {
            let mut tree = self.tree.borrow_mut();
            tree.set_text(node, Arc::clone(&job), hash, wrap);
            // For the widget rect below, and for the immediate paint path. On
            // the deferred path the galley that is painted comes from
            // `paint_texts`, after the layout is final; in the steady state
            // that is this same galley, kept on the node for the rest of the
            // pass.
            tree.text_galley(node, fonts, wrap_width(&content, wrap))
        };

        // The rect `Label` allocates: the galley's own size, not the node's.
        // A `<Text grow={1}>` fills its row, and the text inside it does not.
        let rect = galley
            .as_ref()
            .map_or(content, |galley| galley_rect(&content, galley));

        let selectable =
            selectable.unwrap_or_else(|| self.root_ui.style().interaction.selectable_labels);

        // The sense `Label` picks. A plain `<Text>` is inert; a selectable one
        // takes the clicks and drags that start and extend a selection, minus
        // `FOCUSABLE`, so the TAB key still walks past it.
        let sense = if selectable {
            // On a touch screen, dragging scrolls the enclosing `ScrollArea`
            // rather than selecting, exactly as `Label` decides it.
            let allow_drag_to_select = self.root_ui.input(|i| !i.has_touch_screen());
            let mut select_sense = if allow_drag_to_select {
                egui::Sense::click_and_drag()
            } else {
                egui::Sense::click()
            };
            select_sense -= egui::Sense::FOCUSABLE;
            egui::Sense::hover() | select_sense
        } else {
            egui::Sense::hover()
        };

        // The widget rect `Label` registers.
        let response = self.root_ui.interact(rect, scope.with(index), sense);
        let enabled = self.root_ui.is_enabled();
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, enabled, &job.text));

        // `Label`'s colour. Its `interactive` flag is true only when the caller
        // asked for a sense of its own, which `<Text>` never does, so a
        // selectable `<Text>` is coloured like an inert one and does not change
        // colour under the pointer. The cursor is what says it is selectable.
        let color = self.root_ui.style().visuals.text_color();

        match galley.filter(|_| selectable && self.placed && !first_frame) {
            Some(galley) => {
                let underline = if response.has_focus() || response.highlighted() {
                    egui::Stroke::new(1.0, color)
                } else {
                    egui::Stroke::NONE
                };
                egui::text_selection::LabelSelectionState::label_text_selection(
                    self.root_ui,
                    &response,
                    galley_pos(&content, &galley),
                    galley,
                    color,
                    underline,
                );
            }
            None => {
                let idx = self.root_ui.painter().add(Shape::Noop);
                self.tree.borrow_mut().texts.push(PendingText {
                    node,
                    idx,
                    color,
                    wrap,
                    opacity,
                });
            }
        }

        if let Some(bg_idx) = bg_idx {
            // In front of the galley, and the tree's `Ui` goes back to the
            // opacity it had outside this node.
            self.claim_border(node, paint, bg_idx, opacity);
            self.root_ui.set_opacity(outer_opacity);
        }

        response
    }
}

/// The width a `<Text>` galley is laid out for inside `rect`.
#[inline]
fn wrap_width(rect: &Rect, wrap: bool) -> f32 {
    if wrap { rect.width() } else { f32::INFINITY }
}

/// Where `egui::Label` puts the galley's top left corner inside the rect it
/// was given, and how big the rect it allocates for it is.
///
/// `halign` is the galley's own: it says which edge of the rect the text is
/// measured from, which is what `Label` reads it for too.
fn galley_pos(rect: &Rect, galley: &Galley) -> Pos2 {
    anchor(rect, galley.job.halign)
}

/// [`galley_pos`] for a text whose galley is not at hand: the point of `rect`
/// a galley with `halign` is painted from.
fn anchor(rect: &Rect, halign: egui::Align) -> Pos2 {
    match halign {
        egui::Align::LEFT => rect.left_top(),
        egui::Align::Center => rect.center_top(),
        egui::Align::RIGHT => rect.right_top(),
    }
}

/// The rect `egui::Label` would allocate for `galley` inside `rect`.
fn galley_rect(rect: &Rect, galley: &Galley) -> Rect {
    let size = galley.size();
    let pos = galley_pos(rect, galley);
    let min = match galley.job.halign {
        egui::Align::LEFT => pos,
        egui::Align::Center => pos - egui::vec2(size.x / 2.0, 0.0),
        egui::Align::RIGHT => pos - egui::vec2(size.x, 0.0),
    };
    Rect::from_min_size(min, size)
}

/// The layout job `egui::Label` would build for `text` in a `Ui` of `ui`.
///
/// A copy of the non-wrapping branch of `Label::layout_in_ui`, minus the
/// `wrap.max_width`, which each layout fills in from the width it is asked
/// about. `ui` is the tree's own `Ui`: a leaf `Ui` inherits its style and its
/// layout, so the two agree on the font, the alignment and the vertical
/// alignment of the text.
fn text_job(ui: &egui::Ui, text: egui::WidgetText) -> (Arc<LayoutJob>, u64) {
    let valign = ui.text_valign();
    let mut job = Arc::unwrap_or_clone(text.into_layout_job(
        ui.style(),
        egui::FontSelection::Default,
        valign,
    ));
    let layout = ui.layout();
    job.halign = layout.horizontal_placement();
    job.justify = layout.horizontal_justify();

    let hash = egui::epaint::util::hash(&job);
    (Arc::new(job), hash)
}

/// How much room a tree takes in the `Ui` it is drawn in, and what rect its
/// root node is laid out into.
#[derive(Clone, Copy)]
pub(crate) enum Reserve {
    /// The available width; the height comes from what the tree measured.
    ///
    /// Every `<View>` that is not the app root and not a list row.
    Content,
    /// All the space that is left, on both axes.
    ///
    /// The runner's root, so the outermost `<View>` fills the window.
    AllSpace,
    /// Exactly this size, wherever the cursor is.
    ///
    /// The caller knows the rect, so none of it is read from the `Ui`. That is
    /// what a `<VirtualList>` row uses. Inside `ScrollArea::show_rows` the
    /// space that is left runs from the row to the bottom of the band of
    /// visible rows, so a root rect taken from it moves with the scroll
    /// offset, and the room reserved afterwards would be the height the row
    /// happened to draw rather than the `row_h` the visible range was worked
    /// out from. Both go away here. See [`crate::Cx::with_root_size`].
    Fixed(Vec2),
}

/// Draw one tree, from the `Ui` it sits in.
///
/// The frame goes: reserve space, compute early if only the rect resized, let
/// `f` build the tree, sweep what nothing drew, recompute and decide about the
/// second pass, then tell the surrounding `Ui` how much room the tree took.
pub(crate) fn show<R>(
    store: &Store,
    ui: &mut egui::Ui,
    id: egui::Id,
    style: taffy::Style,
    paint: PaintStyle,
    reserve: Reserve,
    f: impl FnOnce(&mut TreeCx<'_>) -> R,
) -> R {
    // The same reservation `egui_taffy`'s `TuiInitializer` makes: a definite
    // width taken from the space that is left, and a minimum width on the `Ui`
    // so the surrounding layout knows about it. The height is `MinContent`
    // unless the caller asks for all the space.
    //
    // `Fixed` does none of that: the size is the caller's, so nothing is read
    // from the `Ui` but where its cursor is, and the space is reserved at the
    // end instead.
    let (root_rect, available_space) = match reserve {
        Reserve::Fixed(size) => (
            Rect::from_min_size(ui.available_rect_before_wrap().min, size),
            Size {
                width: AvailableSpace::Definite(size.x),
                height: AvailableSpace::Definite(size.y),
            },
        ),
        Reserve::Content | Reserve::AllSpace => {
            let width = ui.available_size().x;
            ui.set_min_width(width);
            let mut available_space = Size {
                width: AvailableSpace::Definite(width),
                height: AvailableSpace::MinContent,
            };
            if matches!(reserve, Reserve::AllSpace) {
                let height = ui.available_size().y;
                ui.set_min_height(height);
                available_space.height = AvailableSpace::Definite(height);
            }
            (ui.available_rect_before_wrap(), available_space)
        }
    };

    let tree = store.tree(id);
    // A child `Ui`, as `egui_taffy` does, rather than the caller's own: this is
    // what `cx.ui()` hands out, and a `<Panel>` carves its space out of it.
    let mut root_ui = ui.new_child(UiBuilder::new());

    tree.borrow_mut()
        .layout_first_on_resize(root_rect, available_space, Fonts::of(&root_ui));

    // The root node carries the container style the caller passed. It has no
    // parent, so it is never reordered, and its key is the tree's own id.
    let (root, root_layout, first_frame) = tree.borrow_mut().add_child_node(id, style, None, 0);

    // A tree opened inside a hidden leaf is hidden from its root.
    let hidden = store.in_hidden();

    let mut used = 0usize;
    let inner = {
        let mut tc = TreeCx {
            tree: &tree,
            root_ui: &mut root_ui,
            parent: root,
            origin: border_box_min(&root_layout, root_rect.min),
            placed: !first_frame,
            hidden,
            child_index: &mut used,
        };
        // The root node's own box works exactly like a container's: a slot
        // behind everything the tree draws, one in front of it, and the
        // opacity on the tree's `Ui` in between.
        let bg_idx = tc.claim_bg(paint, hidden);
        let outer_opacity = tc.root_ui.opacity();
        if bg_idx.is_some()
            && let Some(opacity) = paint.opacity
        {
            tc.root_ui.multiply_opacity(opacity);
        }
        let opacity = tc.root_ui.opacity();

        let inner = f(&mut tc);

        if let Some(bg_idx) = bg_idx {
            tc.claim_border(root, paint, bg_idx, opacity);
            tc.root_ui.set_opacity(outer_opacity);
        }
        inner
    };

    let taken = {
        let mut tree = tree.borrow_mut();
        // Before the sweep, unlike `egui_taffy`, which trims the root's tail
        // after it recomputes and so lays out one frame with the stale tail
        // still attached. Every other node is trimmed before, and doing the
        // same here is one rule instead of two.
        tree.trim_children(root, used);
        tree.finish(root, root_rect, available_space, Fonts::of(&root_ui));
        // After `finish`, so every box and every galley lands where the layout
        // for *this* frame puts it rather than where the last one did.
        tree.paint_boxes(&root_ui, root_rect.min);
        tree.paint_texts(&root_ui, root_rect.min);
        match reserve {
            // The size the caller asked for, not the one the content came to,
            // so the cursor advances by the same amount whatever the tree drew.
            // A tree that drew taller overlaps what comes next; that is the
            // caller's business (see `<VirtualList>`).
            Reserve::Fixed(size) => size,
            Reserve::Content | Reserve::AllSpace => {
                let content_size = tree.layout(root).content_size;
                egui::vec2(content_size.width, content_size.height)
            }
        }
    };
    ui.allocate_space(taken);

    inner
}

/// Build an empty tree. Only [`Store`] calls this.
pub(crate) fn new_tree() -> Rc<RefCell<Tree>> {
    Rc::new(RefCell::new(Tree::new()))
}
