//! [`Cx`]: the context threaded through every component and hook.

use std::fmt::Debug;
use std::hash::Hash;
use std::panic::Location;

use crate::engine::lite::{self, LiteCx};
use crate::engine::{self, Reserve, TreeCx};
use crate::layout::{ContainerStyle, ItemStyle};
use crate::store::Store;

/// Hash key derived from a `#[track_caller]` call site.
pub(crate) fn location_key(location: &'static Location<'static>) -> (&'static str, u32, u32) {
    (location.file(), location.line(), location.column())
}

/// Where a [`Cx`] is currently drawing.
///
/// Outside a `<View>` the surface is a plain egui `Ui`; inside one it is a
/// position in the layout tree (see [`crate::engine`]), and every direct child
/// has to become a taffy node of its own.
enum Surface<'u> {
    Ui(&'u mut egui::Ui),
    Tree(TreeCx<'u>),
    /// A position in a `<VirtualList>` row laid out without taffy. Everything
    /// `Cx` does here it also does in [`Surface::Tree`]; only who solves the
    /// boxes differs. See [`crate::engine::lite`].
    Lite(LiteCx<'u>),
}

/// The context passed to components and hooks.
///
/// `'s` is the lifetime of the [`Store`] and of every hook handle taken from
/// it; it is independent of the `&mut Cx` borrow, so a hook can return a guard
/// that outlives the borrow of the `Cx` it was taken from.
pub struct Cx<'s, 'u> {
    /// The hook store. `Copy`, so it can be re-borrowed into nested closures.
    pub store: &'s Store,
    surface: Surface<'u>,
    scope: egui::Id,
    /// The id layout trees and nodes are keyed by.
    ///
    /// Normally this walks in step with `scope`, so it is the same id. It parts
    /// from `scope` where a list draws the same shape in a reused slot: hook
    /// state has to follow the row, while the taffy tree has to stay with the
    /// slot so that scrolling reuses nodes instead of building new ones. See
    /// [`Cx::with_layout_id`].
    layout: egui::Id,
    /// The size the next tree opened over a plain `Ui` is laid out into, if the
    /// caller set one. See [`Cx::with_root_size`].
    root_size: Option<egui::Vec2>,
}

impl<'s, 'u> Cx<'s, 'u> {
    /// Build a `Cx` for `ui` under the scope id `scope`.
    ///
    /// This is what an egui container closure (`ui.vertical(|ui| ..)`) uses to
    /// re-enter the tree with the inner `Ui`. The layout id starts at `scope`;
    /// an element that re-enters inside a reused list slot puts the slot's
    /// layout id back with [`Cx::with_layout_id`].
    pub fn new(store: &'s Store, ui: &'u mut egui::Ui, scope: egui::Id) -> Self {
        // Once per pass, before any `<Text>` reaches for its cached galley: a
        // `Ui` is the proof of a running pass, which the fonts lookup needs.
        store.note_fonts(ui.ctx());
        Self::at_ui(store, ui, scope, scope, None)
    }

    /// A `Cx` over an egui `Ui`, with both ids and the root size hint given.
    fn at_ui(
        store: &'s Store,
        ui: &'u mut egui::Ui,
        scope: egui::Id,
        layout: egui::Id,
        root_size: Option<egui::Vec2>,
    ) -> Self {
        Self {
            store,
            surface: Surface::Ui(ui),
            scope,
            layout,
            root_size,
        }
    }

    /// A `Cx` at a position in a lite row, with both ids given.
    ///
    /// The root size hint is dropped for the same reason as in
    /// [`Cx::at_tree`]: inside a row a `<View>` is a node, and its rect comes
    /// from the layout.
    fn at_lite(store: &'s Store, lite: LiteCx<'u>, scope: egui::Id, layout: egui::Id) -> Self {
        Self {
            store,
            surface: Surface::Lite(lite),
            scope,
            layout,
            root_size: None,
        }
    }

    /// A `Cx` at a position in a layout tree, with both ids given.
    ///
    /// The root size hint is dropped here: it is about the rect a tree is laid
    /// out into, and inside a tree a `<View>` is a node whose rect the layout
    /// decides. A tree opened by a leaf of this one starts from a plain `Ui`
    /// again and is unaffected as well.
    fn at_tree(store: &'s Store, tree: TreeCx<'u>, scope: egui::Id, layout: egui::Id) -> Self {
        Self {
            store,
            surface: Surface::Tree(tree),
            scope,
            layout,
            root_size: None,
        }
    }

    /// The egui `Ui` currently being drawn into.
    ///
    /// In taffy mode this is the tree's own `Ui`, so anything drawn through it
    /// is *not* laid out by taffy. Elements use [`Cx::leaf`] and
    /// [`Cx::container`] instead; this is the escape hatch for user code that
    /// wants to call egui directly, and it is what a docked `<Panel>` carves
    /// its space out of.
    ///
    /// "The tree's own `Ui`" is exact: a container is a rect, not a `Ui`, so
    /// however many `<View>`s sit between the tree root and the caller, this
    /// is the `Ui` the tree was started in. That is what makes four sibling
    /// `<Panel>`s dock along the window's four edges instead of each carving
    /// up its own little node.
    pub fn ui(&mut self) -> &mut egui::Ui {
        match &mut self.surface {
            Surface::Ui(ui) => ui,
            Surface::Tree(tree) => tree.root_ui(),
            Surface::Lite(lite) => lite.root_ui(),
        }
    }

    /// Whether this `Cx` is inside a `<View>`.
    ///
    /// True on both layout paths: what an element reads it for is whether its
    /// size is the layout's decision, and that is the same either way.
    pub fn in_taffy(&self) -> bool {
        matches!(self.surface, Surface::Tree(_) | Surface::Lite(_))
    }

    /// The id of the current component scope; the base of every hook id.
    pub fn scope_id(&self) -> egui::Id {
        self.scope
    }

    /// The id the next taffy tree or node is keyed by.
    ///
    /// The same as [`Cx::scope_id`] unless a list element replaced it (see
    /// [`Cx::with_layout_id`]). `<View>` passes this to [`Cx::container`].
    pub fn layout_id(&self) -> egui::Id {
        self.layout
    }

    /// Run `f` with `id` as the layout id, leaving hooks and the egui id stack
    /// alone.
    ///
    /// This is an internal for list elements, not something a component body
    /// needs. `<VirtualList>` uses it to key each row's taffy tree by the slot
    /// the row occupies rather than by the row's index: a slot is drawn on
    /// every frame, so its nodes are reused instead of created, measured in an
    /// invisible pass and thrown away when the row scrolls out.
    ///
    /// The ids handed to this must be unique among the nodes drawn in one
    /// frame; two of them under one taffy tree would collide.
    pub fn with_layout_id<R>(&mut self, id: egui::Id, f: impl FnOnce(&mut Cx<'s, '_>) -> R) -> R {
        let store = self.store;
        let scope = self.scope;
        let root_size = self.root_size;
        match &mut self.surface {
            Surface::Ui(ui) => {
                let mut cx = Cx::at_ui(store, ui, scope, id, root_size);
                f(&mut cx)
            }
            // Inside a tree the layout id is what an unnamed node hashes into
            // its key, so swapping it moves the whole subtree's nodes with it.
            // Nothing else changes: no `Ui` is pushed.
            Surface::Tree(tree) => {
                let mut cx = Cx::at_tree(store, tree.reborrow(), scope, id);
                f(&mut cx)
            }
            Surface::Lite(lite) => {
                let mut cx = Cx::at_lite(store, lite.reborrow(), scope, id);
                f(&mut cx)
            }
        }
    }

    /// Run `f` with the size the trees it opens are laid out into.
    ///
    /// This is an internal for list elements, like [`Cx::with_layout_id`], not
    /// something a component body needs.
    ///
    /// A `<View>` over a plain `Ui` normally takes the width that is left,
    /// measures its own height and reserves that much room. Neither is what a
    /// `<VirtualList>` row wants. Inside `egui::ScrollArea::show_rows` the
    /// space that is left runs from the row to the bottom of the band of
    /// visible rows, so the root rect moves with the scroll offset; and the
    /// room reserved is whatever the row drew, which is not the `row_h` the
    /// visible range was worked out from, so the rows drift out of step with
    /// it.
    ///
    /// With a size given, the tree's root rect is that size at the cursor, both
    /// axes are definite, and exactly that much room is reserved afterwards. So
    /// the rows sit at one pitch, and the root size is the same on every frame:
    /// a scrolled frame lays a row out again only when something in the row
    /// really changed.
    ///
    /// The size wins over what the tree measured. A tree that draws taller than
    /// this overlaps whatever comes after it; `<VirtualList>` says the same
    /// thing about its rows, and it is the same rule.
    ///
    /// **The hint is not consumed by the first tree**: every tree `f` opens
    /// over a plain `Ui` gets it, at any depth of components, until a tree is
    /// entered — inside a tree the hint is dropped, so a tree opened by a leaf
    /// of this one is unaffected. Consuming it would mean shared mutable state,
    /// because a `Cx` is copied into each child scope rather than borrowed, and
    /// it would buy nothing: the caller's contract is that everything drawn
    /// here is one row of a fixed height, so a second root tree in the same row
    /// is already a mistake either way.
    pub fn with_root_size<R>(
        &mut self,
        size: egui::Vec2,
        f: impl FnOnce(&mut Cx<'s, '_>) -> R,
    ) -> R {
        let (store, scope, layout) = (self.store, self.scope, self.layout);
        match &mut self.surface {
            Surface::Ui(ui) => {
                let mut cx = Cx::at_ui(store, ui, scope, layout, Some(size));
                f(&mut cx)
            }
            // Nothing to do in tree mode: a `<View>` here is a node, and its
            // rect comes from the layout.
            Surface::Tree(tree) => {
                let mut cx = Cx::at_tree(store, tree.reborrow(), scope, layout);
                f(&mut cx)
            }
            Surface::Lite(lite) => {
                let mut cx = Cx::at_lite(store, lite.reborrow(), scope, layout);
                f(&mut cx)
            }
        }
    }

    /// The egui context.
    pub fn ctx(&self) -> &egui::Context {
        match &self.surface {
            Surface::Ui(ui) => ui.ctx(),
            Surface::Tree(tree) => tree.ctx(),
            Surface::Lite(lite) => lite.ctx(),
        }
    }

    /// Enter a component scope: deepen the hook scope, the layout id and the
    /// egui id.
    ///
    /// `source` needs `Debug` as well as `Hash` because egui 0.36's
    /// `Ui::push_id` takes `impl AsIdSalt`, which is `Hash + Debug`.
    pub fn scope<R>(
        &mut self,
        source: impl Hash + Debug,
        f: impl FnOnce(&mut Cx<'s, '_>) -> R,
    ) -> R {
        let store = self.store;
        let scope = self.scope.with(&source);
        let layout = self.layout.with(&source);
        let root_size = self.root_size;
        match &mut self.surface {
            Surface::Ui(ui) => {
                ui.push_id(source, |ui| {
                    let mut cx = Cx::at_ui(store, ui, scope, layout, root_size);
                    f(&mut cx)
                })
                .inner
            }
            // No `Ui` is pushed in tree mode: a node is a rect, and the only
            // `Ui` in the tree is the root's. Deepening the two ids is what
            // separates two instances of the same subtree — the layout id
            // keys their nodes, and the hook scope salts their leaves' `Ui`s.
            Surface::Tree(tree) => {
                let mut cx = Cx::at_tree(store, tree.reborrow(), scope, layout);
                f(&mut cx)
            }
            Surface::Lite(lite) => {
                let mut cx = Cx::at_lite(store, lite.reborrow(), scope, layout);
                f(&mut cx)
            }
        }
    }

    /// Enter a custom hook scope: deepen the hook scope only.
    ///
    /// Unlike [`Cx::scope`] this does not touch the egui id stack, because a
    /// custom hook is not a widget boundary.
    pub fn hook_scope<R>(
        &mut self,
        location: &'static Location<'static>,
        f: impl FnOnce(&mut Cx<'s, '_>) -> R,
    ) -> R {
        self.scope_sharing_ui(location_key(location), f)
    }

    /// Enter a component scope that draws into *this* `Ui`.
    ///
    /// Same hook scoping as [`Cx::scope`], but without `Ui::push_id`, so the
    /// component shares the surface with its parent. `#[component(shares_ui)]`
    /// elements go through here: a docked panel has to carve space out of the
    /// parent's `Ui`, and `Ui::end_row` only reaches the grid it was called on.
    pub fn scope_sharing_ui<R>(
        &mut self,
        source: impl Hash + Debug,
        f: impl FnOnce(&mut Cx<'s, '_>) -> R,
    ) -> R {
        let store = self.store;
        let scope = self.scope.with(&source);
        // The layout id deepens here too, and it has to: two sibling
        // `shares_ui` components draw into one `Ui`, so if both kept the
        // parent's layout id their `<View>`s would ask for the same taffy tree
        // and collide. `source` is a call site or a hook's location, never a
        // row index, so nothing positional leaks in.
        let layout = self.layout.with(&source);
        let root_size = self.root_size;
        let mut cx = Cx {
            store,
            surface: self.reborrow(),
            scope,
            layout,
            root_size,
        };
        f(&mut cx)
    }

    /// Draw one egui widget, as a taffy leaf when inside a container.
    ///
    /// Outside a container `style` is ignored: a plain `Ui` has nothing to
    /// apply flex item properties to.
    pub fn leaf<R>(&mut self, style: &ItemStyle, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let (prefix, scope) = (self.layout, self.scope);
        match &mut self.surface {
            Surface::Ui(ui) => f(ui),
            Surface::Tree(tree) => tree.leaf(prefix, scope, style.to_taffy(), true, f),
            Surface::Lite(lite) => lite.leaf(scope, style, true, f),
        }
    }

    /// Draw static text, as a taffy node when inside a container.
    ///
    /// Outside a container this is `ui.add(egui::Label::new(text))` and
    /// nothing else. Inside one it skips the `Label` and the `Ui` it would need:
    /// the galley is laid out by the layout engine, which is where its size is
    /// wanted anyway, and painted straight onto the tree's own `Ui`. What the
    /// engine keeps of `Label` is the widget rect, the `WidgetInfo` and the
    /// text selection, so the text is still hoverable, still in the
    /// accessibility tree, still found by `egui_kittest`'s label queries and
    /// still selectable with the mouse.
    ///
    /// `wrap` picks between `TextWrapMode::Wrap` and `TextWrapMode::Extend`,
    /// as `<Text>`'s own prop does. `selectable` is `Label::selectable`:
    /// `None` follows the style's `interaction.selectable_labels`. A `<Text>`
    /// created this frame is not selectable for that one frame, because its
    /// place on screen is only known once the layout is computed.
    pub fn text(
        &mut self,
        style: &ItemStyle,
        text: egui::WidgetText,
        wrap: bool,
        selectable: Option<bool>,
    ) -> egui::Response {
        let (prefix, scope) = (self.layout, self.scope);
        match &mut self.surface {
            Surface::Ui(ui) => {
                let wrap_mode = if wrap {
                    egui::TextWrapMode::Wrap
                } else {
                    egui::TextWrapMode::Extend
                };
                let mut label = egui::Label::new(text).wrap_mode(wrap_mode);
                if let Some(selectable) = selectable {
                    label = label.selectable(selectable);
                }
                ui.add(label)
            }
            Surface::Tree(tree) => {
                tree.text(prefix, scope, style.to_taffy(), text, wrap, selectable)
            }
            Surface::Lite(lite) => lite.text(scope, style, text, wrap, selectable),
        }
    }

    /// Like [`Cx::leaf`], but the leaf takes whatever space taffy gives it.
    ///
    /// A plain leaf is measured by its content: taffy sizes it to what it drew
    /// and never larger. That is wrong for egui widgets that themselves fill
    /// the space they are given (`ScrollArea`): the widget fills its node, then
    /// reports that as its content size, and the node is pinned at whatever
    /// size the first frame happened to have. This variant reports no content
    /// size at all, so the node is sized purely by taffy: `w` / `h`, `grow`,
    /// or the remaining space in the container.
    pub fn leaf_fill<R>(&mut self, style: &ItemStyle, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let (prefix, scope) = (self.layout, self.scope);
        match &mut self.surface {
            Surface::Ui(ui) => f(ui),
            Surface::Tree(tree) => tree.leaf(prefix, scope, style.to_taffy(), false, f),
            Surface::Lite(lite) => lite.leaf(scope, style, false, f),
        }
    }

    /// Open a container and draw `f` inside it.
    ///
    /// Outside a container this starts a new layout tree in the current `Ui`;
    /// inside one it adds a child node. Either way `f` receives a `Cx` that is
    /// inside a container, so its direct children become layout nodes.
    ///
    /// The two styles are passed as they are rather than merged into a
    /// [`taffy::Style`], because a `<VirtualList>` row is laid out by the lite
    /// path (`crate::engine::lite`), which reads them directly and never builds
    /// a taffy style at all. The taffy path merges them itself.
    pub fn container<R>(
        &mut self,
        id: egui::Id,
        container: &ContainerStyle,
        item: &ItemStyle,
        f: impl FnOnce(&mut Cx<'s, '_>) -> R,
    ) -> R {
        let store = self.store;
        let scope = self.scope;
        let layout = self.layout;

        // A row of a `<VirtualList>`: a fixed rect over a plain `Ui`. That is
        // the one place the lite path applies, and only while every style in
        // the row is inside its subset.
        if let Surface::Ui(ui) = &mut self.surface
            && let Some(size) = self.root_size
            && !store.taffy_rows_forced()
        {
            let tree = store.lite_tree(id);
            if lite::supported(&tree, container, item) {
                return lite::show(&tree, ui, container, item, size, |lite| {
                    let mut cx = Cx::at_lite(store, lite.reborrow(), scope, layout);
                    f(&mut cx)
                });
            }
        }

        if let Surface::Lite(lite) = &mut self.surface {
            return lite.container(container, item, |lite| {
                let mut cx = Cx::at_lite(store, lite.reborrow(), scope, layout);
                f(&mut cx)
            });
        }

        self.container_taffy(id, container.merge(item), false, f)
    }

    /// [`Cx::container`] for the root of an app: reserve *all* available space.
    ///
    /// The runner uses this so that the outermost `<View>` fills the window;
    /// nested containers only reserve the available width, so that a column of
    /// them stacks instead of each one claiming the whole height.
    ///
    /// Takes a [`taffy::Style`], unlike [`Cx::container`]: an app root is never
    /// a `<VirtualList>` row, so it never takes the lite path, and the runner's
    /// `root_style()` is a taffy style users can reach for.
    pub fn root_container<R>(
        &mut self,
        id: egui::Id,
        style: taffy::Style,
        f: impl FnOnce(&mut Cx<'s, '_>) -> R,
    ) -> R {
        self.container_taffy(id, style, true, f)
    }

    fn container_taffy<R>(
        &mut self,
        id: egui::Id,
        style: taffy::Style,
        all_space: bool,
        f: impl FnOnce(&mut Cx<'s, '_>) -> R,
    ) -> R {
        let store = self.store;
        let scope = self.scope;
        let layout = self.layout;
        // A size set by `with_root_size` wins over both: the caller knows the
        // rect, so nothing is taken from the `Ui`.
        let reserve = match (self.root_size, all_space) {
            (Some(size), _) => Reserve::Fixed(size),
            (None, true) => Reserve::AllSpace,
            (None, false) => Reserve::Content,
        };
        match &mut self.surface {
            Surface::Ui(ui) => engine::show(store, ui, id, style, reserve, |tree| {
                let mut cx = Cx::at_tree(store, tree.reborrow(), scope, layout);
                f(&mut cx)
            }),
            Surface::Tree(tree) => tree.container(id, style, |tree| {
                let mut cx = Cx::at_tree(store, tree.reborrow(), scope, layout);
                f(&mut cx)
            }),
            // A raw taffy style inside a lite row, which only
            // [`Cx::root_container`] can be: the solver cannot read it, so the
            // row moves to the taffy path and this frame is drawn again.
            Surface::Lite(lite) => lite.fall_back_container(|lite| {
                let mut cx = Cx::at_lite(store, lite.reborrow(), scope, layout);
                f(&mut cx)
            }),
        }
    }

    /// Queue `f` to run at the end of the pass, after every guard is gone.
    ///
    /// The closure is `'static` because it outlives the component body, so
    /// captured values need `move`. It cannot touch the store, so it does not
    /// request a repaint on its own; use
    /// [`State::update_later`](crate::State::update_later) for that.
    pub fn defer(&self, f: impl FnOnce() + 'static) {
        self.store.defer_raw(Box::new(move |_store| f()));
    }

    /// Re-borrow the surface for a shorter lifetime.
    fn reborrow(&mut self) -> Surface<'_> {
        match &mut self.surface {
            Surface::Ui(ui) => Surface::Ui(ui),
            Surface::Tree(tree) => Surface::Tree(tree.reborrow()),
            Surface::Lite(lite) => Surface::Lite(lite.reborrow()),
        }
    }
}
