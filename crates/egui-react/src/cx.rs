//! [`Cx`]: the context threaded through every component and hook.

use std::fmt::Debug;
use std::hash::Hash;
use std::panic::Location;

use crate::engine::{self, TreeCx};
use crate::layout::ItemStyle;
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
}

impl<'s, 'u> Cx<'s, 'u> {
    /// Build a `Cx` for `ui` under the scope id `scope`.
    ///
    /// This is what an egui container closure (`ui.vertical(|ui| ..)`) uses to
    /// re-enter the tree with the inner `Ui`. The layout id starts at `scope`;
    /// an element that re-enters inside a reused list slot puts the slot's
    /// layout id back with [`Cx::with_layout_id`].
    pub fn new(store: &'s Store, ui: &'u mut egui::Ui, scope: egui::Id) -> Self {
        Self::at_ui(store, ui, scope, scope)
    }

    /// A `Cx` over an egui `Ui`, with both ids given.
    fn at_ui(store: &'s Store, ui: &'u mut egui::Ui, scope: egui::Id, layout: egui::Id) -> Self {
        Self {
            store,
            surface: Surface::Ui(ui),
            scope,
            layout,
        }
    }

    /// A `Cx` at a position in a layout tree, with both ids given.
    fn at_tree(store: &'s Store, tree: TreeCx<'u>, scope: egui::Id, layout: egui::Id) -> Self {
        Self {
            store,
            surface: Surface::Tree(tree),
            scope,
            layout,
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
        }
    }

    /// Whether this `Cx` is inside a taffy container.
    pub fn in_taffy(&self) -> bool {
        matches!(self.surface, Surface::Tree(_))
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
        match &mut self.surface {
            Surface::Ui(ui) => {
                let mut cx = Cx::at_ui(store, ui, scope, id);
                f(&mut cx)
            }
            // Inside a tree the layout id is what an unnamed node hashes into
            // its key, so swapping it moves the whole subtree's nodes with it.
            // Nothing else changes: no `Ui` is pushed.
            Surface::Tree(tree) => {
                let mut cx = Cx::at_tree(store, tree.reborrow(), scope, id);
                f(&mut cx)
            }
        }
    }

    /// The egui context.
    pub fn ctx(&self) -> &egui::Context {
        match &self.surface {
            Surface::Ui(ui) => ui.ctx(),
            Surface::Tree(tree) => tree.ctx(),
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
        match &mut self.surface {
            Surface::Ui(ui) => {
                ui.push_id(source, |ui| {
                    let mut cx = Cx::at_ui(store, ui, scope, layout);
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
        let mut cx = Cx {
            store,
            surface: self.reborrow(),
            scope,
            layout,
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
        }
    }

    /// Open a taffy container and draw `f` inside it.
    ///
    /// Outside taffy this starts a new layout tree in the current `Ui`; inside
    /// taffy it adds a child node. Either way `f` receives a `Cx` in taffy
    /// mode, so its direct children become taffy nodes.
    pub fn container<R>(
        &mut self,
        id: egui::Id,
        style: taffy::Style,
        f: impl FnOnce(&mut Cx<'s, '_>) -> R,
    ) -> R {
        self.container_reserving(id, style, false, f)
    }

    /// [`Cx::container`] for the root of an app: reserve *all* available space.
    ///
    /// The runner uses this so that the outermost `<View>` fills the window;
    /// nested containers only reserve the available width, so that a column of
    /// them stacks instead of each one claiming the whole height.
    pub fn root_container<R>(
        &mut self,
        id: egui::Id,
        style: taffy::Style,
        f: impl FnOnce(&mut Cx<'s, '_>) -> R,
    ) -> R {
        self.container_reserving(id, style, true, f)
    }

    fn container_reserving<R>(
        &mut self,
        id: egui::Id,
        style: taffy::Style,
        all_space: bool,
        f: impl FnOnce(&mut Cx<'s, '_>) -> R,
    ) -> R {
        let store = self.store;
        let scope = self.scope;
        let layout = self.layout;
        match &mut self.surface {
            Surface::Ui(ui) => engine::show(store, ui, id, style, all_space, |tree| {
                let mut cx = Cx::at_tree(store, tree.reborrow(), scope, layout);
                f(&mut cx)
            }),
            Surface::Tree(tree) => tree.container(id, style, |tree| {
                let mut cx = Cx::at_tree(store, tree.reborrow(), scope, layout);
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
        }
    }
}
