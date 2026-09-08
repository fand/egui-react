//! The lite layout path: a `<VirtualList>` row laid out without a taffy tree.
//!
//! A row is a single-line flex box of two or three children with a fixed
//! height. The taffy path holds a retained tree for it: a node per `<View>` and
//! per `<Text>`, keyed by [`egui::Id`] in a `HashMap`, carrying a
//! [`taffy::Style`] of about three hundred bytes that is rebuilt and compared
//! every frame. None of that is the layout; all of it is the price of a general
//! retained tree.
//!
//! This module is the other way round. A row's nodes are a `Vec` rebuilt in
//! draw order every frame (the allocation is reused), a node's identity is its
//! index, its style is the [`ItemStyle`] / [`ContainerStyle`] the element
//! already had, and the boxes are solved by [`LiteTree::solve`] below, which is
//! CSS flexbox for the part of those two structs that a row uses.
//!
//! Everything else is the taffy path's: the leaf measure function
//! ([`super::measure_leaf`]), the galley cache ([`super::TextCtx`]), the way a
//! `<Text>` paints and registers itself, and the rule for when a frame has to
//! be drawn again. So a row lays out to the same rects either way, which
//! `crates/egui-react/tests/lite_parity.rs` checks node by node.
//!
//! **The subset.** `display` flex or none, `direction` row / column and their
//! reverses, `justify` and `align` other than `baseline`, `gap`, `w h min_w
//! min_h max_w max_h` in points or percent, `grow` / `shrink` / `basis`, and
//! `m*` / `p*` in points or percent. Anything else — `wrap`, `align_content`,
//! grid, block, `baseline`, `col_span` / `row_span`, an `auto` margin — is
//! outside it. The first such style seen switches that row's slot to the taffy
//! path for good (see [`LiteTree::fall_back`]). A percentage `basis` against a
//! container whose main size is not definite is *in* the subset: it resolves to
//! "measure the content", which is what taffy does with it, and the parity
//! corpus has a case for it.
//!
//! **The frame.** `render` runs first and every leaf draws into the rect the
//! last frame put it in ([`LiteCx::leaf`]); then the tree is solved and the
//! rects compared, and the frame is drawn again only if something moved
//! ([`LiteTree::finish`]). Two rules are carried over from the taffy path
//! because dropping either would cost a pass: a frame whose nodes are identical
//! to the last one's is not solved at all (taffy's dirty flag, asked as a
//! question about the input), and a frame that only resized is solved *before*
//! the leaves draw ([`LiteTree::begin`]).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use egui::{Pos2, Rect, UiBuilder, Vec2};
use taffy::{AvailableSpace, MaybeMath as _};

use super::{
    Fonts, Measure, TextCtx, describe_text, discard_reason, galley_pos, galley_rect, measure_leaf,
    text_job, text_moved, wrap_width,
};
use crate::layout::{Align, ContainerStyle, Direction, Display, ItemStyle, Justify, Length};

// ---------------------------------------------------------------------------
// Geometry helpers
//
// The flex algorithm is written in main/cross terms, but percentages and
// margins resolve against the *inline* size, so both framings are needed. As
// taffy does, values are kept in width/height and read through `main` / `cross`
// accessors that take the direction.
// ---------------------------------------------------------------------------

/// One value per axis, in width/height terms.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Sz<T> {
    w: T,
    h: T,
}

impl<T: Copy> Sz<T> {
    /// The value on the main axis of a row (`w`) or a column (`h`).
    fn main(self, row: bool) -> T {
        if row { self.w } else { self.h }
    }

    /// The value on the cross axis of a row (`h`) or a column (`w`).
    fn cross(self, row: bool) -> T {
        if row { self.h } else { self.w }
    }

    fn set_main(&mut self, row: bool, v: T) {
        if row { self.w = v } else { self.h = v }
    }

    fn set_cross(&mut self, row: bool, v: T) {
        if row { self.h = v } else { self.w = v }
    }

    fn with_main(mut self, row: bool, v: T) -> Self {
        self.set_main(row, v);
        self
    }

    fn with_cross(mut self, row: bool, v: T) -> Self {
        self.set_cross(row, v);
        self
    }

    /// Build from a main and a cross value.
    fn of(row: bool, main: T, cross: T) -> Self {
        if row {
            Sz { w: main, h: cross }
        } else {
            Sz { w: cross, h: main }
        }
    }
}

impl Sz<Option<f32>> {
    const NONE: Self = Sz { w: None, h: None };

    /// Per axis: keep this value if it has one, else take `other`'s.
    fn or(self, other: Self) -> Self {
        Sz {
            w: self.w.or(other.w),
            h: self.h.or(other.h),
        }
    }

    fn clamp(self, min: Self, max: Self) -> Self {
        Sz {
            w: self.w.maybe_clamp(min.w, max.w),
            h: self.h.maybe_clamp(min.h, max.h),
        }
    }

    /// Raise each axis that has a value to at least `rhs`; leave `None` alone.
    fn floor(self, rhs: Sz<f32>) -> Self {
        Sz {
            w: self.w.maybe_max(rhs.w),
            h: self.h.maybe_max(rhs.h),
        }
    }

    fn sub(self, rhs: Sz<f32>) -> Self {
        Sz {
            w: self.w.maybe_sub(rhs.w),
            h: self.h.maybe_sub(rhs.h),
        }
    }

    fn unwrap_or(self, other: Sz<f32>) -> Sz<f32> {
        Sz {
            w: self.w.unwrap_or(other.w),
            h: self.h.unwrap_or(other.h),
        }
    }
}

impl Sz<f32> {
    const ZERO: Self = Sz { w: 0.0, h: 0.0 };

    fn add(self, rhs: Self) -> Self {
        Sz {
            w: self.w + rhs.w,
            h: self.h + rhs.h,
        }
    }

    fn max(self, rhs: Self) -> Self {
        Sz {
            w: self.w.max(rhs.w),
            h: self.h.max(rhs.h),
        }
    }

    fn clamp_opt(self, min: Sz<Option<f32>>, max: Sz<Option<f32>>) -> Self {
        Sz {
            w: self.w.maybe_clamp(min.w, max.w),
            h: self.h.maybe_clamp(min.h, max.h),
        }
    }

    fn some(self) -> Sz<Option<f32>> {
        Sz {
            w: Some(self.w),
            h: Some(self.h),
        }
    }

    fn definite(self) -> Sz<AvailableSpace> {
        Sz {
            w: AvailableSpace::Definite(self.w),
            h: AvailableSpace::Definite(self.h),
        }
    }
}

/// The four sides of a margin or a padding, in points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Edges {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl Edges {
    const ZERO: Self = Self {
        left: 0.0,
        right: 0.0,
        top: 0.0,
        bottom: 0.0,
    };

    fn h_sum(&self) -> f32 {
        self.left + self.right
    }

    fn v_sum(&self) -> f32 {
        self.top + self.bottom
    }

    fn sum_axes(&self) -> Sz<f32> {
        Sz {
            w: self.h_sum(),
            h: self.v_sum(),
        }
    }

    fn main_sum(&self, row: bool) -> f32 {
        if row { self.h_sum() } else { self.v_sum() }
    }

    fn cross_sum(&self, row: bool) -> f32 {
        if row { self.v_sum() } else { self.h_sum() }
    }

    /// The side the main axis starts at. Not mirrored for a reverse direction:
    /// taffy lays a reversed line out from the same padding edge and only walks
    /// the items the other way, and so does this.
    fn main_start(&self, row: bool) -> f32 {
        if row { self.left } else { self.top }
    }

    fn cross_start(&self, row: bool) -> f32 {
        if row { self.top } else { self.left }
    }
}

/// A [`Length`] as a definite size, or `None` for `auto` and for a percentage
/// of a basis that is not known.
fn dim(v: Option<Length>, basis: Option<f32>) -> Option<f32> {
    match v {
        Some(Length::Px(v)) => Some(v),
        Some(Length::Percent(p)) => basis.map(|b| b * p),
        Some(Length::Auto) | None => None,
    }
}

/// A [`Length`] as a padding or a margin: `auto` and an unresolvable percentage
/// are zero, as taffy's `resolve_or_zero` has them.
fn length_or_zero(v: Option<Length>, basis: Option<f32>) -> f32 {
    match v {
        Some(Length::Px(v)) => v,
        Some(Length::Percent(p)) => basis.unwrap_or(0.0) * p,
        Some(Length::Auto) | None => 0.0,
    }
}

/// The shorthand order of `ItemStyle`'s edge props: the most specific wins.
fn pick(specific: Option<Length>, axis: Option<Length>, all: Option<Length>) -> Option<Length> {
    specific.or(axis).or(all)
}

/// `width` and `height`, resolved against the containing block.
fn item_size(item: &ItemStyle, basis: Sz<Option<f32>>) -> Sz<Option<f32>> {
    Sz {
        w: dim(item.w, basis.w),
        h: dim(item.h, basis.h),
    }
}

fn item_min(item: &ItemStyle, basis: Sz<Option<f32>>) -> Sz<Option<f32>> {
    Sz {
        w: dim(item.min_w, basis.w),
        h: dim(item.min_h, basis.h),
    }
}

fn item_max(item: &ItemStyle, basis: Sz<Option<f32>>) -> Sz<Option<f32>> {
    Sz {
        w: dim(item.max_w, basis.w),
        h: dim(item.max_h, basis.h),
    }
}

/// The padding, resolved against the containing block's *inline* size, which is
/// what CSS says for both axes.
fn item_padding(item: &ItemStyle, basis: Option<f32>) -> Edges {
    Edges {
        top: length_or_zero(pick(item.pt, item.py, item.p), basis),
        right: length_or_zero(pick(item.pr, item.px, item.p), basis),
        bottom: length_or_zero(pick(item.pb, item.py, item.p), basis),
        left: length_or_zero(pick(item.pl, item.px, item.p), basis),
    }
}

/// The margin, resolved like the padding. An `auto` margin is outside the
/// subset, so it never reaches here as anything but zero.
fn item_margin(item: &ItemStyle, basis: Option<f32>) -> Edges {
    Edges {
        top: length_or_zero(pick(item.mt, item.my, item.m), basis),
        right: length_or_zero(pick(item.mr, item.mx, item.m), basis),
        bottom: length_or_zero(pick(item.mb, item.my, item.m), basis),
        left: length_or_zero(pick(item.ml, item.mx, item.m), basis),
    }
}

/// Is this axis's size style `auto`? Read by the `align-items: stretch` rule,
/// which asks about the style rather than the resolved value.
fn is_auto(v: Option<Length>) -> bool {
    matches!(v, None | Some(Length::Auto))
}

/// Is the main axis horizontal?
fn is_row(direction: Direction) -> bool {
    matches!(direction, Direction::Row | Direction::RowReverse)
}

/// Are the items laid out from the end of the main axis?
fn is_reverse(direction: Direction) -> bool {
    matches!(direction, Direction::RowReverse | Direction::ColumnReverse)
}

/// `Normal` means "unset", as it does in [`ContainerStyle::merge`].
fn align_opt(a: Align) -> Option<Align> {
    (a != Align::Normal).then_some(a)
}

fn justify_opt(j: Justify) -> Option<Justify> {
    (j != Justify::Normal).then_some(j)
}

/// Gaps sit between items, so `n` items have `n - 1` of them.
fn sum_axis_gaps(gap: f32, count: usize) -> f32 {
    if count <= 1 {
        0.0
    } else {
        gap * (count - 1) as f32
    }
}

/// CSS fallback alignment, as taffy implements it: a distributed alignment with
/// nothing to distribute becomes an edge or a centre alignment, and a "safe"
/// alignment with negative free space becomes `start`.
fn alignment_fallback(free_space: f32, count: usize, mode: Justify) -> Justify {
    let (mut mode, is_safe) = if count <= 1 || free_space <= 0.0 {
        match mode {
            Justify::Stretch | Justify::SpaceBetween => (Justify::FlexStart, true),
            Justify::SpaceAround | Justify::SpaceEvenly => (Justify::Center, true),
            other => (other, false),
        }
    } else {
        (mode, false)
    };
    if free_space <= 0.0 && is_safe {
        mode = Justify::Start;
    }
    mode
}

/// How far along the main axis an item sits, beyond where the one before it
/// ended: the whole free space for the first item, the gap plus a share of the
/// free space for the rest.
fn alignment_offset(
    free_space: f32,
    count: usize,
    gap: f32,
    mode: Justify,
    reversed: bool,
    first: bool,
) -> f32 {
    if first {
        match mode {
            Justify::Start => 0.0,
            Justify::FlexStart | Justify::Normal => {
                if reversed {
                    free_space
                } else {
                    0.0
                }
            }
            Justify::End => free_space,
            Justify::FlexEnd => {
                if reversed {
                    0.0
                } else {
                    free_space
                }
            }
            Justify::Center => free_space / 2.0,
            Justify::Stretch | Justify::SpaceBetween => 0.0,
            Justify::SpaceAround => {
                if free_space >= 0.0 {
                    (free_space / count as f32) / 2.0
                } else {
                    free_space / 2.0
                }
            }
            Justify::SpaceEvenly => {
                if free_space >= 0.0 {
                    free_space / (count + 1) as f32
                } else {
                    free_space / 2.0
                }
            }
        }
    } else {
        let free_space = free_space.max(0.0);
        gap + match mode {
            Justify::SpaceBetween => free_space / (count - 1) as f32,
            Justify::SpaceAround => free_space / count as f32,
            Justify::SpaceEvenly => free_space / (count + 1) as f32,
            _ => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// The tree
// ---------------------------------------------------------------------------

/// What a node is, beyond its flex item properties.
enum Kind {
    /// A `<View>`: its own container properties, and children.
    Container(ContainerStyle),
    /// A `<Text>`: an index into [`LiteTree::texts`], where the galley cache
    /// is, plus what the frame comparison below needs to see change.
    Text { slot: usize, hash: u64, wrap: bool },
    /// A widget leaf. The measurement is what it reported while it drew, this
    /// same frame, so it is never a frame out of date.
    Leaf(Measure),
}

/// One node, rebuilt every frame in draw order.
struct Node {
    kind: Kind,
    item: ItemStyle,
    /// Children as a linked list, so that pushing a node stays O(1) and the
    /// frame allocates nothing beyond the node vector itself.
    first_child: Option<usize>,
    last_child: Option<usize>,
    next_sibling: Option<usize>,
    child_count: usize,
}

/// Where the solver put a node: everything the rect of the node and of its
/// children is read from. Unrounded; the rounding happens in one pass at the
/// end, as taffy does it.
#[derive(Clone, Copy, Debug, Default)]
struct Box2 {
    /// The border box's corner, relative to the parent's border box.
    location: Vec2,
    size: Sz<f32>,
    padding: Edges,
}

/// A `<Text>` whose shape is reserved in the painter and filled in once this
/// frame's layout is final. The taffy path's [`super::PendingText`], with a node
/// index in place of the taffy node.
struct PendingText {
    node: usize,
    idx: egui::layers::ShapeIdx,
    color: egui::Color32,
    wrap: bool,
}

/// One row's layout state: everything a slot keeps between frames.
pub(crate) struct LiteTree {
    /// The nodes of the frame being drawn, in draw order. Cleared and refilled
    /// every frame; the allocation is kept.
    nodes: Vec<Node>,
    /// Where the solver put each node, before rounding.
    layouts: Vec<Box2>,
    /// The content rect of each node as of the last finished frame, relative to
    /// the corner of the rect the row was given.
    ///
    /// Relative, not on screen: a scrolled row is the same layout at a
    /// different place, and comparing screen rects would report a move on every
    /// scrolled frame.
    rects: Vec<Rect>,
    /// This frame's rects, until they are swapped into `rects`.
    new_rects: Vec<Rect>,
    /// The galley cache of each `<Text>`, by the order the texts are drawn in.
    texts: Vec<TextCtx>,
    /// How many `<Text>`s the frame being drawn has claimed so far.
    text_count: usize,
    /// The `<Text>` shapes claimed this frame, waiting for the final layout.
    pending: Vec<PendingText>,
    /// The nodes of the frame before this one, kept so that a frame that
    /// changed nothing can be recognised without solving anything.
    prev_nodes: Vec<Node>,
    /// The size of the rect the row was laid out into last.
    last_size: Vec2,
    /// Did a leaf draw this frame without a rect to draw into?
    created: bool,
    /// The style that put this slot on the taffy path, if one did.
    fallback: Option<&'static str>,
    last_visited: u64,
}

impl LiteTree {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            prev_nodes: Vec::new(),
            last_size: Vec2::ZERO,
            layouts: Vec::new(),
            rects: Vec::new(),
            new_rects: Vec::new(),
            texts: Vec::new(),
            text_count: 0,
            pending: Vec::new(),
            created: false,
            fallback: None,
            last_visited: 0,
        }
    }

    /// Mark the slot as drawn in `pass`.
    pub(crate) fn visit(&mut self, pass: u64) {
        self.last_visited = pass;
    }

    /// The pass this slot was last drawn in.
    pub(crate) fn last_visited(&self) -> u64 {
        self.last_visited
    }

    /// Has this slot been moved to the taffy path?
    pub(crate) fn fallen_back(&self) -> bool {
        self.fallback.is_some()
    }

    /// Move this slot to the taffy path for good, naming the style that did it.
    ///
    /// Logged once per slot: a row author who wonders why one row is slower
    /// than its neighbours gets told which attribute to look at. The caller
    /// asks for a discard, because the frame being drawn was laid out by a
    /// solver that does not understand that attribute.
    fn fall_back(&mut self, field: &'static str) {
        if self.fallback.is_none() {
            self.fallback = Some(field);
            log::debug!(
                "egui-react: a row falls back to the taffy layout path: `{field}` is outside the lite subset"
            );
        }
    }

    /// The rect a node was drawn into last frame, if it had one.
    fn last_rect(&self, node: usize) -> Option<Rect> {
        self.rects.get(node).copied()
    }

    /// Start a frame: keep the last frame's nodes to compare against, and take
    /// their allocation for this frame's.
    ///
    /// When the only thing that changed is the size of the rect the row was
    /// given, the layout is recomputed *here*, before anything draws. That is
    /// [`super::Tree::layout_first_on_resize`], for the same reason: the tree
    /// is still the one the last frame built and every leaf's measurement is
    /// still on it, so the leaves can draw at their new places in this pass
    /// instead of drawing at the old ones and costing a second one.
    fn begin(&mut self, size: Vec2, solve: Solve<'_>) {
        if !self.nodes.is_empty() && self.last_size != size {
            self.lay_out(size, solve);
            self.last_size = size;
        }
        std::mem::swap(&mut self.nodes, &mut self.prev_nodes);
        self.nodes.clear();
        self.pending.clear();
        self.text_count = 0;
        self.created = false;
    }

    /// Add a node under `parent` and return its index.
    fn push(&mut self, parent: Option<usize>, kind: Kind, item: ItemStyle) -> usize {
        let index = self.nodes.len();
        self.nodes.push(Node {
            kind,
            item,
            first_child: None,
            last_child: None,
            next_sibling: None,
            child_count: 0,
        });
        if let Some(parent) = parent {
            match self.nodes[parent].last_child {
                Some(last) => self.nodes[last].next_sibling = Some(index),
                None => self.nodes[parent].first_child = Some(index),
            }
            self.nodes[parent].last_child = Some(index);
            self.nodes[parent].child_count += 1;
        }
        index
    }

    /// Claim the galley cache of the next `<Text>` in draw order, keeping what
    /// is in it when neither the job nor the wrap mode changed.
    fn push_text(&mut self, job: Arc<egui::text::LayoutJob>, hash: u64, wrap: bool) -> usize {
        let slot = self.text_count;
        self.text_count += 1;
        let fresh = TextCtx {
            job,
            hash,
            wrap,
            galley: None,
        };
        match self.texts.get_mut(slot) {
            Some(old) if old.hash == hash && old.wrap == wrap => {}
            Some(old) => *old = fresh,
            None => self.texts.push(fresh),
        }
        slot
    }
}

// ---------------------------------------------------------------------------
// The solver
//
// Single-line CSS flexbox, in the order the spec puts it (9.2 line length, 9.3
// main size, 9.4 cross size, 9.5 main alignment, 9.6 cross alignment, 9.7
// resolving flexible lengths), ported from taffy 0.9's `compute_flexbox_layout`
// with everything the subset excludes taken out: no wrapping, so there is
// always exactly one line; no baselines; no absolute positioning; no borders,
// scrollbar gutters or aspect ratios, none of which egui-react's styles can
// express; and `box-sizing: border-box`, which is taffy's default and the only
// one an `ItemStyle` produces.
// ---------------------------------------------------------------------------

/// The inputs of one computation: taffy's `LayoutInput`, minus what this path
/// does not use.
#[derive(Clone, Copy)]
struct Input {
    /// The size the parent has already decided on, per axis.
    known: Sz<Option<f32>>,
    /// The containing block percentages resolve against.
    parent_size: Sz<Option<f32>>,
    available: Sz<AvailableSpace>,
    /// taffy's `SizingMode::ContentSize`: ignore the node's own size styles,
    /// because the caller has applied them already.
    content_sizing: bool,
    /// taffy's `RunMode::PerformLayout`: place every node below, rather than
    /// only report how big this one is.
    perform: bool,
}

/// What the solver needs from the outside world.
#[derive(Clone, Copy)]
struct Solve<'a> {
    fonts: Fonts<'a>,
    /// The rect the row was given. A `leaf_fill` that reports "infinite" is
    /// clamped to it, exactly as on the taffy path.
    root_size: Vec2,
}

impl LiteTree {
    /// Lay the whole row out, from the root down.
    ///
    /// The root is taffy's `compute_root_layout`: no known size, the row's rect
    /// as both the containing block and the available space, and the container
    /// sized by its own styles inside that.
    fn solve(&mut self, root_size: Vec2, solve: Solve<'_>) {
        self.layouts.clear();
        self.layouts.resize(self.nodes.len(), Box2::default());

        let size = Sz {
            w: root_size.x,
            h: root_size.y,
        };
        let root = self.compute(
            0,
            Input {
                known: Sz::NONE,
                parent_size: size.some(),
                available: size.definite(),
                content_sizing: false,
                perform: true,
            },
            solve,
        );
        let padding = item_padding(&self.nodes[0].item, Some(root_size.x));
        self.layouts[0] = Box2 {
            location: Vec2::ZERO,
            size: root,
            padding,
        };
    }

    /// Lay `node` out and return the size of its border box.
    fn compute(&mut self, node: usize, input: Input, solve: Solve<'_>) -> Sz<f32> {
        let hidden = matches!(&self.nodes[node].kind, Kind::Container(style) if style.display == Display::None);
        if hidden {
            self.hide(node);
            return Sz::ZERO;
        }
        // A container with no children is laid out as a leaf, which is what
        // taffy does with it too: the flex algorithm needs items.
        let flex =
            matches!(self.nodes[node].kind, Kind::Container(_)) && self.nodes[node].child_count > 0;
        if flex {
            self.compute_flex(node, input, solve)
        } else {
            self.compute_leaf(node, input, solve)
        }
    }

    /// Zero a `display: none` node and everything under it.
    fn hide(&mut self, node: usize) {
        self.layouts[node] = Box2::default();
        let mut child = self.nodes[node].first_child;
        while let Some(index) = child {
            self.hide(index);
            child = self.nodes[index].next_sibling;
        }
    }

    /// taffy's `compute_leaf_layout`: apply the node's own size styles, work out
    /// what space is left for its content, and ask the measure function.
    fn compute_leaf(&mut self, node: usize, input: Input, solve: Solve<'_>) -> Sz<f32> {
        let item = self.nodes[node].item;
        let padding = item_padding(&item, input.parent_size.w);
        let margin = item_margin(&item, input.parent_size.w);
        let padding_sum = padding.sum_axes();

        // In content sizing mode the caller has already applied the size
        // styles, so applying them again would clamp twice.
        let (node_size, min_size, max_size) = if input.content_sizing {
            (input.known, Sz::NONE, Sz::NONE)
        } else {
            (
                input.known.or(item_size(&item, input.parent_size)),
                item_min(&item, input.parent_size),
                item_max(&item, input.parent_size),
            )
        };

        // Both axes are decided by the styles alone, and the caller only wants
        // the size: there is nothing to measure.
        if !input.perform
            && let (Some(w), Some(h)) = (node_size.w, node_size.h)
        {
            return Sz { w, h }.clamp_opt(min_size, max_size).max(padding_sum);
        }

        let available = Sz {
            w: input
                .known
                .w
                .map(AvailableSpace::Definite)
                .unwrap_or(input.available.w)
                .maybe_sub(margin.h_sum())
                .maybe_set(input.known.w)
                .maybe_set(node_size.w)
                .map_definite_value(|s| s.maybe_clamp(min_size.w, max_size.w) - padding.h_sum()),
            h: input
                .known
                .h
                .map(AvailableSpace::Definite)
                .unwrap_or(input.available.h)
                .maybe_sub(margin.v_sum())
                .maybe_set(input.known.h)
                .maybe_set(node_size.h)
                .map_definite_value(|s| s.maybe_clamp(min_size.h, max_size.h) - padding.v_sum()),
        };

        let measured = self.measure(node, available, solve);
        input
            .known
            .or(node_size)
            .unwrap_or(measured.add(padding_sum))
            .clamp_opt(min_size, max_size)
            .max(padding_sum)
    }

    /// What a node with no children of its own reports for `available`.
    ///
    /// The `<Text>` branch lays the galley out here rather than measuring a
    /// drawn widget, which is why a new `<Text>` needs no sizing pass. Both
    /// branches are the taffy path's measure function, called on the same
    /// inputs.
    fn measure(&mut self, node: usize, available: Sz<AvailableSpace>, solve: Solve<'_>) -> Sz<f32> {
        let taffy_available = taffy::Size {
            width: available.w,
            height: available.h,
        };
        let measure = match &self.nodes[node].kind {
            Kind::Text { slot, wrap, .. } => {
                let (slot, wrap) = (*slot, *wrap);
                let width = if wrap {
                    match available.w {
                        AvailableSpace::Definite(width) => width,
                        AvailableSpace::MinContent => 0.0,
                        AvailableSpace::MaxContent => f32::INFINITY,
                    }
                } else {
                    f32::INFINITY
                };
                let size = self.texts[slot].galley(solve.fonts, width).size().ceil();
                return Sz {
                    w: size.x,
                    h: size.y,
                };
            }
            Kind::Leaf(measure) => *measure,
            // A `<View>` that drew no children measures nothing, as an empty
            // taffy node with no context does.
            Kind::Container(_) => Measure::EMPTY,
        };
        let size = measure_leaf(measure, taffy_available, solve.root_size);
        Sz {
            w: size.width,
            h: size.height,
        }
    }

    /// taffy's `compute_flexbox_layout`: settle the container's own size first,
    /// then run the algorithm inside it.
    fn compute_flex(&mut self, node: usize, input: Input, solve: Solve<'_>) -> Sz<f32> {
        let item = self.nodes[node].item;
        let padding_sum = item_padding(&item, input.parent_size.w).sum_axes();
        let min_size = item_min(&item, input.parent_size);
        let max_size = item_max(&item, input.parent_size);
        let styled_size = if input.content_sizing {
            Sz::NONE
        } else {
            item_size(&item, input.parent_size).clamp(min_size, max_size)
        };
        // A max below a min decides the axis on its own.
        let pinned = Sz {
            w: match (min_size.w, max_size.w) {
                (Some(min), Some(max)) if max <= min => Some(min),
                _ => None,
            },
            h: match (min_size.h, max_size.h) {
                (Some(min), Some(max)) if max <= min => Some(min),
                _ => None,
            },
        };
        let known = input.known.or(pinned.or(styled_size).floor(padding_sum));

        if !input.perform
            && let (Some(w), Some(h)) = (known.w, known.h)
        {
            return Sz { w, h };
        }

        self.compute_preliminary(node, Input { known, ..input }, solve)
    }

    /// The flex algorithm itself.
    #[allow(clippy::too_many_lines)]
    fn compute_preliminary(&mut self, node: usize, input: Input, solve: Solve<'_>) -> Sz<f32> {
        let container = match &self.nodes[node].kind {
            Kind::Container(style) => style.clone(),
            _ => unreachable!("only a container is laid out as a flex box"),
        };
        let item = self.nodes[node].item;
        let row = is_row(container.direction);
        let reverse = is_reverse(container.direction);

        // 9.1. Initial setup: the constants the rest of the algorithm reads.
        let margin = item_margin(&item, input.parent_size.w);
        let inset = item_padding(&item, input.parent_size.w);
        let align_items = align_opt(container.align).unwrap_or(Align::Stretch);
        let justify = justify_opt(container.justify);
        let container_min = item_min(&item, input.parent_size);
        let container_max = item_max(&item, input.parent_size);
        let gap = Sz {
            w: container.gap.0,
            h: container.gap.1,
        };

        let mut node_inner_size = input.known.sub(inset.sum_axes());
        let mut container_size = Sz::ZERO;
        let mut inner_container_size = Sz::ZERO;

        // Generate the flex items. `display: none` children take no part and
        // are zeroed at the end.
        let mut items: Vec<FlexItem> = Vec::with_capacity(self.nodes[node].child_count);
        let mut child = self.nodes[node].first_child;
        while let Some(index) = child {
            child = self.nodes[index].next_sibling;
            if matches!(&self.nodes[index].kind, Kind::Container(style) if style.display == Display::None)
            {
                continue;
            }
            let style = self.nodes[index].item;
            items.push(FlexItem {
                node: index,
                size: item_size(&style, node_inner_size),
                min_size: item_min(&style, node_inner_size),
                max_size: item_max(&style, node_inner_size),
                auto_size: Sz {
                    w: is_auto(style.w),
                    h: is_auto(style.h),
                },
                align_self: style.align_self.and_then(align_opt).unwrap_or(align_items),
                grow: style.grow.unwrap_or(0.0),
                shrink: style.shrink.unwrap_or(1.0),
                basis: style.basis,
                margin: item_margin(&style, node_inner_size.w),
                padding: item_padding(&style, node_inner_size.w),
                ..FlexItem::new(index)
            });
        }

        // 9.2 (2). The space the items have to work with.
        let available = Sz {
            w: match input.known.w {
                Some(width) => AvailableSpace::Definite(width - inset.h_sum()),
                None => input
                    .available
                    .w
                    .maybe_sub(margin.h_sum())
                    .maybe_sub(inset.h_sum()),
            },
            h: match input.known.h {
                Some(height) => AvailableSpace::Definite(height - inset.v_sum()),
                None => input
                    .available
                    .h
                    .maybe_sub(margin.v_sum())
                    .maybe_sub(inset.v_sum()),
            },
        };

        // 9.2 (3). The flex base size and the hypothetical main size of each
        // item.
        for item in &mut items {
            let child = item.node;
            let padding_main = item.padding.main_sum(row);
            let padding_axes = item.padding.sum_axes();
            let style_min_main = item.min_size.main(row);
            let style_size_main = item.size.main(row);
            let style_max_main = item.max_size.main(row);
            let cross_available =
                cross_available_space(item, &available, node_inner_size, margin, row);
            let child_known = child_known_dimensions(item, cross_available, row);
            let child_parent = Sz::of(row, None, node_inner_size.cross(row));

            // A. A definite flex basis, or the main size style, is the base
            // size. E. Otherwise measure the content.
            let basis = dim(item.basis, node_inner_size.main(row)).or(style_size_main);
            let flex_basis = match basis {
                Some(basis) => basis,
                None => {
                    let main = if available.main(row) == AvailableSpace::MinContent {
                        AvailableSpace::MinContent
                    } else {
                        AvailableSpace::MaxContent
                    };
                    self.measure_child(
                        child,
                        child_known,
                        child_parent,
                        Sz::of(row, main, cross_available),
                        true,
                        solve,
                    )
                    .main(row)
                }
            };
            // Chrome and Firefox floor the basis by the padding even though the
            // spec says not to; taffy follows them, so this does too.
            let flex_basis = flex_basis.max(padding_main);

            // 4.5. The automatic minimum size of a flex item: its min-content
            // size, unless a `min_w` / `min_h` says otherwise.
            let minimum_main = match style_min_main {
                Some(min) => min,
                None => self
                    .measure_child(
                        child,
                        child_known,
                        child_parent,
                        Sz::of(row, AvailableSpace::MinContent, cross_available),
                        true,
                        solve,
                    )
                    .main(row)
                    .maybe_min(style_size_main)
                    .maybe_min(style_max_main)
                    .max(padding_axes.main(row)),
            };

            item.flex_basis = flex_basis;
            item.inner_flex_basis = flex_basis - padding_main;
            item.resolved_minimum_main_size = minimum_main;
            let min_main = minimum_main.max(padding_axes.main(row));
            let hypothetical = flex_basis.maybe_clamp(Some(min_main), style_max_main);
            item.hypothetical_inner.set_main(row, hypothetical);
            item.hypothetical_outer
                .set_main(row, hypothetical + item.margin.main_sum(row));
        }

        // 9.3 (4). The main size of the container. There is one line, so
        // "collect items into lines" (5) is nothing.
        match node_inner_size.main(row) {
            Some(inner_main) => {
                inner_container_size.set_main(row, inner_main);
                container_size.set_main(row, inner_main + inset.main_sum(row));
            }
            None => {
                let outer_main = self.container_main_size(
                    &mut items,
                    &available,
                    node_inner_size,
                    margin,
                    inset,
                    gap,
                    container_min,
                    container_max,
                    row,
                    solve,
                );
                let inner_main = (outer_main - inset.main_sum(row)).max(0.0);
                container_size.set_main(row, outer_main);
                inner_container_size.set_main(row, inner_main);
                node_inner_size.set_main(row, Some(inner_main));
            }
        }

        // 9.7 (6). Resolve the flexible lengths.
        resolve_flexible_lengths(&mut items, node_inner_size, gap, row);

        // 9.4 (7). The hypothetical cross size of each item.
        for item in &mut items {
            let padding_cross = item.padding.cross_sum(row);
            let child_cross = item
                .size
                .cross(row)
                .maybe_clamp(item.min_size.cross(row), item.max_size.cross(row))
                .maybe_max(padding_cross);
            let child_available_cross = available
                .cross(row)
                .maybe_clamp(item.min_size.cross(row), item.max_size.cross(row))
                .maybe_max(padding_cross);
            let known_main = container_size.main(row);
            let target_main = item.target.main(row);
            let (min_cross, max_cross) = (item.min_size.cross(row), item.max_size.cross(row));
            let child_node = item.node;

            let inner_cross = match child_cross {
                Some(cross) => cross,
                None => self
                    .measure_child(
                        child_node,
                        Sz::of(row, Some(target_main), child_cross),
                        node_inner_size,
                        Sz::of(
                            row,
                            AvailableSpace::Definite(known_main),
                            child_available_cross,
                        ),
                        true,
                        solve,
                    )
                    .cross(row)
                    .maybe_clamp(min_cross, max_cross)
                    .max(padding_cross),
            };
            item.hypothetical_inner.set_cross(row, inner_cross);
            item.hypothetical_outer
                .set_cross(row, inner_cross + item.margin.cross_sum(row));
        }

        // 9.4 (8). The cross size of the one line.
        let cross_inset = inset.cross_sum(row);
        let mut line_cross = match input.known.cross(row) {
            Some(cross) => Some(cross)
                .maybe_clamp(container_min.cross(row), container_max.cross(row))
                .maybe_sub(cross_inset)
                .maybe_max(0.0)
                .unwrap_or(0.0),
            None => items
                .iter()
                .map(|item| item.hypothetical_outer.cross(row))
                .fold(0.0f32, f32::max)
                .maybe_clamp(
                    container_min.cross(row).maybe_sub(cross_inset),
                    container_max.cross(row).maybe_sub(cross_inset),
                ),
        };

        // 9.4 (9). `align-content: stretch`, which is the default and the only
        // value in the subset: one line grows to the container's inner cross
        // size.
        let min_inner_cross = input
            .known
            .cross(row)
            .or(container_min.cross(row))
            .maybe_clamp(container_min.cross(row), container_max.cross(row))
            .maybe_sub(cross_inset)
            .maybe_max(0.0)
            .unwrap_or(0.0);
        if line_cross < min_inner_cross {
            line_cross = min_inner_cross;
        }

        // 9.4 (11). The used cross size of each item.
        for item in &mut items {
            let cross = if item.align_self == Align::Stretch && item.auto_size.cross(row) {
                (line_cross - item.margin.cross_sum(row))
                    .maybe_clamp(item.min_size.cross(row), item.max_size.cross(row))
            } else {
                item.hypothetical_inner.cross(row)
            };
            item.target.set_cross(row, cross);
            item.outer_target
                .set_cross(row, cross + item.margin.cross_sum(row));
        }

        // 9.5 (12). Distribute what main-axis space is left, per justify.
        let count = items.len();
        let main_gap = gap.main(row);
        let used: f32 = sum_axis_gaps(main_gap, count)
            + items
                .iter()
                .map(|item| item.outer_target.main(row))
                .sum::<f32>();
        let free_main = inner_container_size.main(row) - used;
        let mode = alignment_fallback(free_main, count, justify.unwrap_or(Justify::FlexStart));
        for (order, index) in order_of(count, reverse).enumerate() {
            items[index].offset_main =
                alignment_offset(free_main, count, main_gap, mode, reverse, order == 0);
        }

        // 9.6 (13, 14). Align the items along the cross axis. There are no auto
        // margins in the subset, so this is the align branch alone.
        for item in &mut items {
            let free = line_cross - item.outer_target.cross(row);
            item.offset_cross = match item.align_self {
                Align::Start | Align::FlexStart | Align::Stretch | Align::Normal => 0.0,
                Align::End | Align::FlexEnd => free,
                Align::Center => free / 2.0,
                // Outside the subset; the slot has already fallen back.
                Align::Baseline => 0.0,
            };
        }

        // 9.6 (15). The container's used cross size.
        let outer_cross = input
            .known
            .cross(row)
            .unwrap_or(line_cross + cross_inset)
            .maybe_clamp(container_min.cross(row), container_max.cross(row))
            .max(cross_inset);
        container_size.set_cross(row, outer_cross);
        inner_container_size.set_cross(row, (outer_cross - cross_inset).max(0.0));

        if !input.perform {
            return container_size;
        }

        // 16 would align the lines per `align-content`; with one line and the
        // stretch default its offset is always zero, so it is left out.

        // The final pass: lay each item out at the size it ended up with, and
        // record where it sits.
        let mut offset_main = inset.main_start(row);
        let offset_cross_base = inset.cross_start(row);
        for index in order_of(count, reverse) {
            let item = &items[index];
            let target = item.target;
            let size = self.compute(
                item.node,
                Input {
                    known: target.some(),
                    parent_size: node_inner_size,
                    available: container_size.definite(),
                    content_sizing: true,
                    perform: true,
                },
                solve,
            );
            let item = &items[index];
            let main = offset_main + item.offset_main + item.margin.main_start(row);
            let cross = offset_cross_base + item.offset_cross + item.margin.cross_start(row);
            let location = if row {
                egui::vec2(main, cross)
            } else {
                egui::vec2(cross, main)
            };
            self.layouts[item.node] = Box2 {
                location,
                size,
                padding: item.padding,
            };
            offset_main += item.offset_main + item.margin.main_sum(row) + size.main(row);
        }

        // 10. `display: none` children generate no box: zero, and zero
        // everything under them.
        let mut child = self.nodes[node].first_child;
        while let Some(index) = child {
            child = self.nodes[index].next_sibling;
            if matches!(&self.nodes[index].kind, Kind::Container(style) if style.display == Display::None)
            {
                self.hide(index);
            }
        }

        container_size
    }

    /// Measure a child without placing it.
    fn measure_child(
        &mut self,
        node: usize,
        known: Sz<Option<f32>>,
        parent_size: Sz<Option<f32>>,
        available: Sz<AvailableSpace>,
        content_sizing: bool,
        solve: Solve<'_>,
    ) -> Sz<f32> {
        self.compute(
            node,
            Input {
                known,
                parent_size,
                available,
                content_sizing,
                perform: false,
            },
            solve,
        )
    }

    /// The container's main size when its own styles did not decide it.
    ///
    /// taffy's `determine_container_main_size`, single line.
    #[allow(clippy::too_many_arguments)]
    fn container_main_size(
        &mut self,
        items: &mut [FlexItem],
        available: &Sz<AvailableSpace>,
        node_inner_size: Sz<Option<f32>>,
        margin: Edges,
        inset: Edges,
        gap: Sz<f32>,
        container_min: Sz<Option<f32>>,
        container_max: Sz<Option<f32>>,
        row: bool,
        solve: Solve<'_>,
    ) -> f32 {
        let main_inset = inset.main_sum(row);
        let gaps = sum_axis_gaps(gap.main(row), items.len());

        let outer_main = match available.main(row) {
            // Definite space to fill: the line is as long as its items want to
            // be, floored by their padding.
            AvailableSpace::Definite(_) => {
                let total: f32 = items
                    .iter()
                    .map(|item| {
                        let padding = item.padding.main_sum(row);
                        (item.flex_basis.maybe_max(item.min_size.main(row))
                            + item.margin.main_sum(row))
                        .max(padding)
                    })
                    .sum();
                total + gaps + main_inset
            }
            // Sized under a min- or max-content constraint: every item's
            // content contribution, scaled by its own flex fraction.
            AvailableSpace::MinContent | AvailableSpace::MaxContent => {
                for index in 0..items.len() {
                    let contribution = self.content_contribution(
                        items,
                        index,
                        available,
                        node_inner_size,
                        margin,
                        main_inset,
                        row,
                        solve,
                    );
                    let item = &mut items[index];
                    item.content_flex_fraction = {
                        let diff = contribution - item.flex_basis;
                        if diff > 0.0 {
                            diff / item.grow.max(1.0)
                        } else if diff < 0.0 {
                            diff / (item.shrink * item.inner_flex_basis).max(1.0)
                        } else {
                            0.0
                        }
                    };
                }
                let sum: f32 = items
                    .iter_mut()
                    .map(|item| {
                        let fraction = item.content_flex_fraction;
                        let contribution = if fraction > 0.0 {
                            item.grow.max(1.0) * fraction
                        } else if fraction < 0.0 {
                            item.shrink.max(1.0) * item.inner_flex_basis * fraction
                        } else {
                            0.0
                        };
                        let size = item.flex_basis + contribution;
                        item.target.set_main(row, size);
                        item.outer_target.set_main(row, size);
                        size
                    })
                    .sum();
                (sum + gaps).max(0.0) + main_inset
            }
        };

        outer_main
            .maybe_clamp(container_min.main(row), container_max.main(row))
            .max(main_inset)
    }

    /// One item's min- or max-content contribution to the container's main size.
    #[allow(clippy::too_many_arguments)]
    fn content_contribution(
        &mut self,
        items: &[FlexItem],
        index: usize,
        available: &Sz<AvailableSpace>,
        node_inner_size: Sz<Option<f32>>,
        margin: Edges,
        main_inset: f32,
        row: bool,
        solve: Solve<'_>,
    ) -> f32 {
        let item = &items[index];
        let style_min = item.min_size.main(row);
        let style_preferred = item.size.main(row);
        let style_max = item.max_size.main(row);

        // An item that cannot shrink is at least its basis, and one that cannot
        // grow is at most its basis.
        let clamping_basis = Some(item.flex_basis).maybe_max(style_preferred);
        let basis_min = clamping_basis.filter(|_| item.shrink == 0.0);
        let basis_max = clamping_basis.filter(|_| item.grow == 0.0);
        let min_main = style_min
            .maybe_max(basis_min)
            .or(basis_min)
            .unwrap_or(item.resolved_minimum_main_size)
            .max(item.resolved_minimum_main_size);
        let max_main = style_max
            .maybe_min(basis_max)
            .or(basis_max)
            .unwrap_or(f32::INFINITY);
        let margin_main = item.margin.main_sum(row);

        match (min_main, style_preferred, max_main) {
            // The clamps decide it, so the content need not be measured.
            (min, Some(preferred), max) if max <= min || max <= preferred => {
                preferred.min(max).max(min) + margin_main
            }
            (min, _, max) if max <= min => min + margin_main,
            _ => {
                let parent_cross = node_inner_size.cross(row);
                let margin_sum = margin.cross_sum(row);
                let cross_available = available
                    .cross(row)
                    .map_definite_value(|value| parent_cross.unwrap_or(value))
                    .maybe_clamp(
                        item.min_size.cross(row).maybe_add(margin_sum),
                        item.max_size.cross(row).maybe_add(margin_sum),
                    );
                let known = child_known_dimensions(item, cross_available, row);
                let node = item.node;
                let flex_basis = item.flex_basis;
                let content_main = self
                    .measure_child(
                        node,
                        known,
                        node_inner_size,
                        available.with_cross(row, cross_available),
                        false,
                        solve,
                    )
                    .main(row)
                    + margin_main;
                // Asymmetric on purpose: a row's automatic size is its
                // min-content size and a column's its max-content size, which
                // is the behaviour taffy matched Webkit and Firefox on.
                if row {
                    content_main
                        .maybe_clamp(style_min, style_max)
                        .max(main_inset)
                } else {
                    content_main
                        .max(flex_basis)
                        .maybe_clamp(style_min, style_max)
                        .max(main_inset)
                }
            }
        }
    }
}

/// Are these two nodes, at the same place in two frames' draw orders, the same
/// input to the solver?
///
/// Everything the solver reads is here: the item style, the container style,
/// what a leaf measured, the text's job and wrap mode, and the number of
/// children. A pre-order list of nodes plus each one's child count determines
/// the tree, so comparing the two lists element by element compares the whole
/// input.
fn same_node(a: &Node, b: &Node) -> bool {
    a.child_count == b.child_count
        && a.item == b.item
        && match (&a.kind, &b.kind) {
            (Kind::Container(a), Kind::Container(b)) => a == b,
            (
                Kind::Text {
                    slot: a_slot,
                    hash: a_hash,
                    wrap: a_wrap,
                },
                Kind::Text {
                    slot: b_slot,
                    hash: b_hash,
                    wrap: b_wrap,
                },
            ) => a_slot == b_slot && a_hash == b_hash && a_wrap == b_wrap,
            (Kind::Leaf(a), Kind::Leaf(b)) => a == b,
            _ => false,
        }
}

/// The available space an item's cross axis is sized under, clamped by its own
/// cross min and max.
fn cross_available_space(
    item: &FlexItem,
    available: &Sz<AvailableSpace>,
    node_inner_size: Sz<Option<f32>>,
    margin: Edges,
    row: bool,
) -> AvailableSpace {
    let parent_cross = node_inner_size.cross(row);
    let margin_sum = margin.cross_sum(row);
    let min = item.min_size.cross(row).maybe_add(margin_sum);
    let max = item.max_size.cross(row).maybe_add(margin_sum);
    match available.cross(row) {
        AvailableSpace::Definite(value) => {
            AvailableSpace::Definite(parent_cross.unwrap_or(value).maybe_clamp(min, max))
        }
        AvailableSpace::MinContent => min
            .map(AvailableSpace::Definite)
            .unwrap_or(AvailableSpace::MinContent),
        AvailableSpace::MaxContent => max
            .map(AvailableSpace::Definite)
            .unwrap_or(AvailableSpace::MaxContent),
    }
}

/// What an item's size is already known to be while its base size is measured:
/// never the main axis, and the cross axis only when it stretches.
fn child_known_dimensions(
    item: &FlexItem,
    cross_available: AvailableSpace,
    row: bool,
) -> Sz<Option<f32>> {
    let mut known = item.size.with_main(row, None);
    if item.align_self == Align::Stretch && known.cross(row).is_none() {
        known.set_cross(
            row,
            cross_available
                .into_option()
                .maybe_sub(item.margin.cross_sum(row)),
        );
    }
    known
}

/// The order items are placed in: reversed for `row-reverse` / `column-reverse`.
fn order_of(count: usize, reverse: bool) -> impl Iterator<Item = usize> {
    (0..count).map(move |i| if reverse { count - 1 - i } else { i })
}

/// 9.7. Resolving flexible lengths: grow or shrink the items to fill the line,
/// clamping and freezing as the spec's loop says.
fn resolve_flexible_lengths(
    items: &mut [FlexItem],
    node_inner_size: Sz<Option<f32>>,
    gap: Sz<f32>,
    row: bool,
) {
    let total_gap = sum_axis_gaps(gap.main(row), items.len());
    let inner_main = node_inner_size.main(row);

    // 1. Grow or shrink? Compare the hypothetical sizes with the line.
    let hypothetical_total: f32 = items
        .iter()
        .map(|item| item.hypothetical_outer.main(row))
        .sum();
    let used_flex_factor = total_gap + hypothetical_total;
    let growing = used_flex_factor < inner_main.unwrap_or(0.0);
    let shrinking = used_flex_factor > inner_main.unwrap_or(0.0);
    let exact = !growing && !shrinking;

    // 2. Freeze the items that cannot flex any further.
    for item in items.iter_mut() {
        let target = item.hypothetical_inner.main(row);
        item.target.set_main(row, target);
        item.frozen = false;
        if exact
            || (item.grow == 0.0 && item.shrink == 0.0)
            || (growing && item.flex_basis > target)
            || (shrinking && item.flex_basis < target)
        {
            item.frozen = true;
            item.outer_target
                .set_main(row, target + item.margin.main_sum(row));
        }
    }

    if exact {
        return;
    }

    // 3. The free space to start from.
    let used_space = |items: &[FlexItem]| -> f32 {
        total_gap
            + items
                .iter()
                .map(|item| {
                    if item.frozen {
                        item.outer_target.main(row)
                    } else {
                        item.flex_basis + item.margin.main_sum(row)
                    }
                })
                .sum::<f32>()
    };
    let initial_free_space = inner_main.maybe_sub(used_space(items)).unwrap_or(0.0);

    // 4. Loop until everything is frozen.
    loop {
        if items.iter().all(|item| item.frozen) {
            return;
        }

        let used = used_space(items);
        let (sum_grow, sum_shrink) = items
            .iter()
            .filter(|item| !item.frozen)
            .fold((0.0, 0.0), |(grow, shrink), item| {
                (grow + item.grow, shrink + item.shrink)
            });

        // b. A total flex factor below one only distributes that fraction.
        let free_space = if growing && sum_grow < 1.0 {
            (initial_free_space * sum_grow - total_gap).maybe_min(inner_main.maybe_sub(used))
        } else if shrinking && sum_shrink < 1.0 {
            (initial_free_space * sum_shrink - total_gap).maybe_max(inner_main.maybe_sub(used))
        } else {
            inner_main
                .maybe_sub(used)
                .unwrap_or(used_flex_factor - used)
        };

        // c. Hand it out in proportion to the flex factors.
        if free_space.is_normal() {
            if growing && sum_grow > 0.0 {
                for item in items.iter_mut().filter(|item| !item.frozen) {
                    let size = item.flex_basis + free_space * (item.grow / sum_grow);
                    item.target.set_main(row, size);
                }
            } else if shrinking && sum_shrink > 0.0 {
                let scaled: f32 = items
                    .iter()
                    .filter(|item| !item.frozen)
                    .map(|item| item.inner_flex_basis * item.shrink)
                    .sum();
                if scaled > 0.0 {
                    for item in items.iter_mut().filter(|item| !item.frozen) {
                        let factor = item.inner_flex_basis * item.shrink;
                        let size = item.flex_basis + free_space * (factor / scaled);
                        item.target.set_main(row, size);
                    }
                }
            }
        }

        // d. Clamp by the min and max sizes, and note by how much.
        let mut total_violation = 0.0;
        for item in items.iter_mut().filter(|item| !item.frozen) {
            let unclamped = item.target.main(row);
            let clamped = unclamped
                .maybe_clamp(
                    Some(item.resolved_minimum_main_size),
                    item.max_size.main(row),
                )
                .max(0.0);
            item.violation = clamped - unclamped;
            item.target.set_main(row, clamped);
            item.outer_target
                .set_main(row, clamped + item.margin.main_sum(row));
            total_violation += item.violation;
        }

        // e. Freeze whichever items were pushed the way the total went.
        for item in items.iter_mut().filter(|item| !item.frozen) {
            item.frozen = if total_violation > 0.0 {
                item.violation > 0.0
            } else if total_violation < 0.0 {
                item.violation < 0.0
            } else {
                true
            };
        }
    }
}

/// One item of the single flex line, while it is being solved.
struct FlexItem {
    node: usize,
    size: Sz<Option<f32>>,
    min_size: Sz<Option<f32>>,
    max_size: Sz<Option<f32>>,
    /// Whether the width / height style is `auto`, which is what the stretch
    /// rule asks about.
    auto_size: Sz<bool>,
    align_self: Align,
    grow: f32,
    shrink: f32,
    basis: Option<Length>,
    margin: Edges,
    padding: Edges,
    flex_basis: f32,
    inner_flex_basis: f32,
    resolved_minimum_main_size: f32,
    content_flex_fraction: f32,
    violation: f32,
    frozen: bool,
    hypothetical_inner: Sz<f32>,
    hypothetical_outer: Sz<f32>,
    target: Sz<f32>,
    outer_target: Sz<f32>,
    offset_main: f32,
    offset_cross: f32,
}

impl FlexItem {
    fn new(node: usize) -> Self {
        Self {
            node,
            size: Sz::NONE,
            min_size: Sz::NONE,
            max_size: Sz::NONE,
            auto_size: Sz { w: true, h: true },
            align_self: Align::Stretch,
            grow: 0.0,
            shrink: 1.0,
            basis: None,
            margin: Edges::ZERO,
            padding: Edges::ZERO,
            flex_basis: 0.0,
            inner_flex_basis: 0.0,
            resolved_minimum_main_size: 0.0,
            content_flex_fraction: 0.0,
            violation: 0.0,
            frozen: false,
            hypothetical_inner: Sz::ZERO,
            hypothetical_outer: Sz::ZERO,
            target: Sz::ZERO,
            outer_target: Sz::ZERO,
            offset_main: 0.0,
            offset_cross: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Rounding, rects and the frame
// ---------------------------------------------------------------------------

impl LiteTree {
    /// Round the solved layout to whole points and turn it into one content
    /// rect per node.
    ///
    /// This is taffy's `round_layout`, which `TaffyTree` runs by default and
    /// which the rects on the taffy path therefore already carry: positions are
    /// rounded on the *cumulative* coordinate so that no gap opens between
    /// neighbours, and a size is the difference of two rounded edges rather
    /// than a rounded size.
    fn round(&mut self, node: usize, cumulative: Vec2, origin: Vec2) {
        let layout = self.layouts[node];
        let cumulative = cumulative + layout.location;
        let (cx, cy) = (cumulative.x, cumulative.y);

        let x = layout.location.x.round();
        let y = layout.location.y.round();
        let width = (cx + layout.size.w).round() - cx.round();
        let height = (cy + layout.size.h).round() - cy.round();
        let left = (cx + layout.padding.left).round() - cx.round();
        let right =
            (cx + layout.size.w).round() - (cx + layout.size.w - layout.padding.right).round();
        let top = (cy + layout.padding.top).round() - cy.round();
        let bottom =
            (cy + layout.size.h).round() - (cy + layout.size.h - layout.padding.bottom).round();

        // The node's own border box corner, in the rounded steps the taffy path
        // walks up through when it places a leaf.
        let origin = origin + egui::vec2(x, y);
        self.new_rects[node] = Rect::from_min_size(
            Pos2::ZERO + origin + egui::vec2(left, top),
            egui::vec2(width - left - right, height - top - bottom),
        );

        let mut child = self.nodes[node].first_child;
        while let Some(index) = child {
            self.round(index, cumulative, origin);
            child = self.nodes[index].next_sibling;
        }
    }

    /// Solve the tree as it stands and turn it into one rect per node.
    /// Returns the first node that also existed before and has moved, with its
    /// old and new rect.
    fn lay_out(&mut self, root_size: Vec2, solve: Solve<'_>) -> Option<(usize, Rect, Rect)> {
        self.solve(root_size, solve);

        self.new_rects.clear();
        self.new_rects.resize(self.nodes.len(), Rect::ZERO);
        if !self.nodes.is_empty() {
            self.round(0, Vec2::ZERO, Vec2::ZERO);
        }

        // Only the nodes that were there before take part: a node added this
        // frame had no rect to be drawn at, so it cannot have moved. That is
        // the taffy path's rule, where a node created this frame is left out of
        // the comparison and a leaf that drew is covered by `created` instead.
        // The taffy path's rules: a `<Text>` is judged by where its galley is
        // painted from, its own size being what it measured; a container paints
        // nothing, so only its corner counts (its children are compared on
        // their own); a widget drew a `Ui` in its whole rect.
        let moved = std::iter::zip(&self.rects, &self.new_rects)
            .enumerate()
            .find_map(|(node, (old, new))| {
                let moved = match &self.nodes[node].kind {
                    Kind::Text { slot, wrap, .. } => {
                        text_moved(old, new, self.texts[*slot].job.halign, *wrap)
                    }
                    Kind::Container(_) => old.min != new.min,
                    Kind::Leaf(_) => old != new,
                };
                moved.then_some((node, *old, *new))
            });
        std::mem::swap(&mut self.rects, &mut self.new_rects);
        moved
    }

    /// What `node` is, for [`discard_reason`].
    fn describe(&self, node: usize) -> String {
        match &self.nodes[node].kind {
            Kind::Text { slot, .. } => describe_text(&self.texts[*slot].job.text),
            Kind::Leaf(_) => format!("a widget (node {node})"),
            Kind::Container(_) => format!("a container (node {node})"),
        }
    }

    /// Solve this frame's tree and decide whether what was drawn is now wrong.
    ///
    /// The rule is the taffy path's, read off the rects instead of off taffy's
    /// `Layout`: a leaf that drew before it had a rect, a node that went away,
    /// or any node that moved means the frame on screen does not match the
    /// layout, so it is drawn again. Returns the reason to hand to
    /// `request_discard`, if there is one.
    fn finish(&mut self, root_size: Vec2, solve: Solve<'_>) -> Option<String> {
        // Nothing about the row changed, so neither can its layout. This is
        // what `taffy.dirty(root)` decides on the other path; here the whole
        // input is the node vector, so comparing it with the last frame's is
        // the same question asked directly.
        let unchanged = self.last_size == root_size
            && self.rects.len() == self.nodes.len()
            && self.nodes.len() == self.prev_nodes.len()
            && std::iter::zip(&self.nodes, &self.prev_nodes).all(|(new, old)| same_node(new, old));
        self.texts.truncate(self.text_count);
        if unchanged {
            return self
                .created
                .then(|| discard_reason("row layout", true, false, None));
        }

        self.last_size = root_size;
        let removed = self.rects.len() > self.nodes.len();
        let moved = self.lay_out(root_size, solve);

        if self.created || removed || moved.is_some() {
            let moved = moved.map(|(node, old, new)| (self.describe(node), old, new));
            Some(discard_reason("row layout", self.created, removed, moved))
        } else {
            None
        }
    }

    /// Put every `<Text>` claimed this frame into the shape it reserved, now
    /// that the layout is final.
    fn paint_texts(&mut self, ui: &egui::Ui, root_min: Pos2) {
        if self.pending.is_empty() {
            return;
        }
        let fonts = Fonts::of(ui);
        let painter = ui.painter();
        for pending in std::mem::take(&mut self.pending) {
            let Some(rect) = self.rects.get(pending.node).copied() else {
                continue;
            };
            let rect = rect.translate(root_min.to_vec2());
            let width = wrap_width(&rect, pending.wrap);
            let Kind::Text { slot, .. } = &self.nodes[pending.node].kind else {
                continue;
            };
            let galley = self.texts[*slot].galley(fonts, width);
            let pos = galley_pos(&rect, &galley);
            painter.set(
                pending.idx,
                egui::epaint::TextShape::new(pos, galley, pending.color),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Building the tree: what `Cx` calls while a row is drawn
// ---------------------------------------------------------------------------

/// Where a [`crate::Cx`] is while it is building one lite row.
///
/// The lite half of `Surface`, shaped like [`super::TreeCx`] so that `Cx` reads
/// the same either way. There is no origin to carry: a node's place on screen
/// is `rects[index]`, and the index is the node's identity.
pub(crate) struct LiteCx<'u> {
    tree: &'u Rc<RefCell<LiteTree>>,
    /// The one `Ui` of this row. Every leaf is a child of it.
    root_ui: &'u mut egui::Ui,
    /// The corner of the rect the row was given; `rects` are relative to it.
    root_min: Pos2,
    /// The node children are added to.
    parent: usize,
    /// Is this position inside a `display="none"` subtree? Same meaning and
    /// same three consequences as on the taffy path.
    hidden: bool,
    /// How many children have been added under `parent` so far. Only the `Ui`
    /// ids are salted with it, so that a leaf keeps the id it has on the taffy
    /// path.
    child_index: &'u mut usize,
}

impl LiteCx<'_> {
    /// Re-borrow for a shorter lifetime, keeping the position in the tree.
    pub(crate) fn reborrow(&mut self) -> LiteCx<'_> {
        LiteCx {
            tree: self.tree,
            root_ui: self.root_ui,
            root_min: self.root_min,
            parent: self.parent,
            hidden: self.hidden,
            child_index: self.child_index,
        }
    }

    /// Is this position inside a `display="none"` subtree?
    pub(crate) fn hidden(&self) -> bool {
        self.hidden
    }

    /// The row's own `Ui`, which is what `cx.ui()` returns here.
    pub(crate) fn root_ui(&mut self) -> &mut egui::Ui {
        self.root_ui
    }

    /// The egui context.
    pub(crate) fn ctx(&self) -> &egui::Context {
        self.root_ui.ctx()
    }

    fn next_index(&mut self) -> usize {
        let index = *self.child_index;
        *self.child_index += 1;
        index
    }

    /// Note a style the solver does not understand and ask for the frame back.
    fn fall_back(&mut self, field: &'static str) {
        self.tree.borrow_mut().fall_back(field);
        self.root_ui
            .ctx()
            .request_discard("egui-react: row layout falls back to taffy");
    }

    /// Check a node's styles, moving the slot to the taffy path if one of them
    /// is outside the subset.
    fn check(&mut self, container: Option<&ContainerStyle>, item: &ItemStyle) {
        if let Some(field) = container.and_then(unsupported_container) {
            self.fall_back(field);
        }
        if let Some(field) = unsupported_item(item) {
            self.fall_back(field);
        }
    }

    /// Add a container node and build its children inside it.
    pub(crate) fn container<R>(
        &mut self,
        container: &ContainerStyle,
        item: &ItemStyle,
        f: impl FnOnce(&mut LiteCx<'_>) -> R,
    ) -> R {
        self.check(Some(container), item);
        self.next_index();
        // The node is pushed and `f` still runs, as on the taffy path: the
        // components inside keep their hooks and the solver zeroes the
        // subtree.
        let hidden = self.hidden || container.display == Display::None;
        let node = self.tree.borrow_mut().push(
            Some(self.parent),
            Kind::Container(container.clone()),
            *item,
        );

        let mut used = 0usize;
        let mut child = LiteCx {
            tree: self.tree,
            root_ui: self.root_ui,
            root_min: self.root_min,
            parent: node,
            hidden,
            child_index: &mut used,
        };
        f(&mut child)
    }

    /// A container whose style the solver cannot read (a raw [`taffy::Style`],
    /// which only `Cx::root_container` passes): move the row to the taffy path
    /// and hold this frame's children in a plain node so that nothing panics
    /// before the frame is drawn again.
    pub(crate) fn fall_back_container<R>(&mut self, f: impl FnOnce(&mut LiteCx<'_>) -> R) -> R {
        self.fall_back("style");
        self.container(&ContainerStyle::default(), &ItemStyle::default(), f)
    }

    /// Add a leaf node and draw one egui `Ui` in the rect the solver gave it.
    ///
    /// The rect is last frame's, as on the taffy path: the leaf has to draw
    /// before it can say how big it is, so the layout that reads its size comes
    /// after. A leaf with no rect yet draws invisible in a sizing pass and the
    /// frame is drawn again.
    pub(crate) fn leaf<R>(
        &mut self,
        scope: egui::Id,
        item: &ItemStyle,
        measured: bool,
        f: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        self.check(None, item);
        let index = self.next_index();
        let node = {
            let mut tree = self.tree.borrow_mut();
            let measure = if measured {
                Measure::default()
            } else {
                Measure::FILL
            };
            tree.push(Some(self.parent), Kind::Leaf(measure), *item)
        };

        let rect = self.tree.borrow().last_rect(node);
        let mut builder = UiBuilder::new()
            .max_rect(
                rect.map(|rect| rect.translate(self.root_min.to_vec2()))
                    .unwrap_or_else(|| Rect::from_min_size(self.root_min, Vec2::ZERO)),
            )
            .id_salt(scope.with(index));
        if rect.is_none() || self.hidden {
            builder = builder.sizing_pass().invisible();
            // A hidden leaf's draw is not a measurement anyone waits for, so
            // it is not a reason to draw the row again.
            if rect.is_none() && !self.hidden {
                self.tree.borrow_mut().created = true;
            }
        }
        let mut ui = self.root_ui.new_child(builder);
        if self.hidden {
            // One hidden accesskit node over the whole leaf; see
            // `super::TreeCx::leaf`.
            ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
                node.set_role(egui::accesskit::Role::GenericContainer);
                node.set_hidden();
            });
        }
        let inner = f(&mut ui);

        if self.hidden {
            // The measure from the last visible draw stays on the node: the
            // solver zeroes a hidden subtree whatever it says.
            return inner;
        }

        if measured {
            let min_size = ui.min_size().ceil();
            let measure = Measure {
                min_size,
                max_size: ui.min_size().max(min_size),
                infinite: egui::Vec2b::FALSE,
            };
            self.tree.borrow_mut().nodes[node].kind = Kind::Leaf(measure);
        }

        inner
    }

    /// Add a `<Text>` node: a galley, a widget rect and nothing else.
    ///
    /// The same two paint paths as [`super::TreeCx::text`], for the same
    /// reasons: a `<Text>` whose place is already known paints in draw order
    /// through `LabelSelectionState`, so it can be selected with the mouse; a
    /// new one reserves its shape and is filled in once the layout is final.
    pub(crate) fn text(
        &mut self,
        scope: egui::Id,
        item: &ItemStyle,
        text: egui::WidgetText,
        wrap: bool,
        selectable: Option<bool>,
    ) -> egui::Response {
        self.check(None, item);
        let index = self.next_index();

        let (job, hash) = text_job(self.root_ui, text);
        let fonts = Fonts::of(self.root_ui);
        let (node, placed, content, galley) = {
            let mut tree = self.tree.borrow_mut();
            let slot = tree.push_text(Arc::clone(&job), hash, wrap);
            let node = tree.push(Some(self.parent), Kind::Text { slot, hash, wrap }, *item);
            let last = tree.last_rect(node);
            // The rect the node was in last frame, as the taffy path reads its
            // last layout: what the widget rect and the immediate paint use.
            let content = last
                .map(|rect| rect.translate(self.root_min.to_vec2()))
                .unwrap_or_else(|| Rect::from_min_size(self.root_min, Vec2::ZERO));
            let galley = tree.texts[slot].galley(fonts, wrap_width(&content, wrap));
            (node, last.is_some(), content, galley)
        };

        if self.hidden {
            // The text and the node are pushed; nothing is registered. Same
            // reasons as `super::TreeCx::text`.
            return self
                .root_ui
                .interact(Rect::NOTHING, scope.with(index), egui::Sense::hover());
        }

        let rect = galley_rect(&content, &galley);
        let selectable =
            selectable.unwrap_or_else(|| self.root_ui.style().interaction.selectable_labels);

        let sense = if selectable {
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

        let response = self.root_ui.interact(rect, scope.with(index), sense);
        let enabled = self.root_ui.is_enabled();
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, enabled, &job.text));

        let color = self.root_ui.style().visuals.text_color();

        if selectable && placed {
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
        } else {
            let idx = self.root_ui.painter().add(egui::Shape::Noop);
            self.tree.borrow_mut().pending.push(PendingText {
                node,
                idx,
                color,
                wrap,
            });
        }

        response
    }
}

/// The container style that puts a row on the taffy path, if there is one.
fn unsupported_container(style: &ContainerStyle) -> Option<&'static str> {
    use crate::layout::Display;
    match style.display {
        Display::Grid | Display::Block => return Some("display"),
        Display::Flex | Display::None => {}
    }
    if style.wrap {
        return Some("wrap");
    }
    if style.align_content.is_some() {
        return Some("align_content");
    }
    if style.align == Align::Baseline {
        return Some("align");
    }
    None
}

/// The item style that puts a row on the taffy path, if there is one.
fn unsupported_item(item: &ItemStyle) -> Option<&'static str> {
    if item.align_self == Some(Align::Baseline) {
        return Some("align_self");
    }
    if item.col_span.is_some() {
        return Some("col_span");
    }
    if item.row_span.is_some() {
        return Some("row_span");
    }
    // An auto margin absorbs free space, which the solver does not do.
    for (name, value) in [
        ("m", item.m),
        ("mx", item.mx),
        ("my", item.my),
        ("mt", item.mt),
        ("mr", item.mr),
        ("mb", item.mb),
        ("ml", item.ml),
    ] {
        if value == Some(Length::Auto) {
            return Some(name);
        }
    }
    None
}

/// Can this row still take the lite path?
///
/// Asked before the row is drawn, so that a root whose own style is outside the
/// subset goes to taffy in this very pass instead of costing a discard.
pub(crate) fn supported(
    tree: &Rc<RefCell<LiteTree>>,
    container: &ContainerStyle,
    item: &ItemStyle,
) -> bool {
    let mut tree = tree.borrow_mut();
    if tree.fallen_back() {
        return false;
    }
    if let Some(field) = unsupported_container(container).or_else(|| unsupported_item(item)) {
        tree.fall_back(field);
        return false;
    }
    true
}

/// Draw one row, from the `Ui` it sits in.
///
/// The frame goes: push the root node, let `f` build the rest and draw every
/// leaf at last frame's rect, solve, decide about a second pass, paint the
/// texts where the solver put them, and reserve exactly the size the caller
/// asked for.
#[allow(clippy::too_many_arguments)]
pub(crate) fn show<R>(
    tree: &Rc<RefCell<LiteTree>>,
    ui: &mut egui::Ui,
    container: &ContainerStyle,
    item: &ItemStyle,
    size: Vec2,
    hidden: bool,
    f: impl FnOnce(&mut LiteCx<'_>) -> R,
) -> R {
    let root_min = ui.available_rect_before_wrap().min;
    let mut root_ui = ui.new_child(UiBuilder::new());

    {
        let mut tree = tree.borrow_mut();
        tree.begin(
            size,
            Solve {
                fonts: Fonts::of(&root_ui),
                root_size: size,
            },
        );
        tree.push(None, Kind::Container(container.clone()), *item);
    }

    let mut used = 0usize;
    let inner = {
        let mut cx = LiteCx {
            tree,
            root_ui: &mut root_ui,
            root_min,
            parent: 0,
            // A row drawn inside a hidden leaf is hidden from its root.
            hidden,
            child_index: &mut used,
        };
        f(&mut cx)
    };

    {
        let mut tree = tree.borrow_mut();
        let discard = tree.finish(
            size,
            Solve {
                fonts: Fonts::of(&root_ui),
                root_size: size,
            },
        );
        if let Some(reason) = discard {
            log::debug!("egui-react: request_discard: {reason}");
            root_ui.ctx().request_discard(reason);
        }
        tree.paint_texts(&root_ui, root_min);
    }

    // The size the caller asked for, not what the row drew, so the cursor moves
    // on by the pitch `show_rows` reserved. Same rule as `Reserve::Fixed`.
    ui.allocate_space(size);

    inner
}

/// Build an empty slot. Only [`crate::store::Store`] calls this.
pub(crate) fn new_tree() -> Rc<RefCell<LiteTree>> {
    Rc::new(RefCell::new(LiteTree::new()))
}
