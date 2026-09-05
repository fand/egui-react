//! [`Cx`]: the context threaded through every component and hook.

use std::fmt::Debug;
use std::hash::Hash;
use std::panic::Location;

use egui_taffy::{Tui, TuiBuilderLogic as _, taffy};

use crate::layout::ItemStyle;
use crate::store::Store;

/// Hash key derived from a `#[track_caller]` call site.
pub(crate) fn location_key(location: &'static Location<'static>) -> (&'static str, u32, u32) {
    (location.file(), location.line(), location.column())
}

/// Where a [`Cx`] is currently drawing.
///
/// Outside a `<View>` the surface is a plain egui `Ui`; inside one it is the
/// `egui_taffy` `Tui` that owns the flex/grid node, and every direct child has
/// to become a taffy node of its own.
enum Surface<'u> {
    Ui(&'u mut egui::Ui),
    Taffy(&'u mut Tui),
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
}

impl<'s, 'u> Cx<'s, 'u> {
    /// Build a `Cx` for `ui` under the scope id `scope`.
    ///
    /// This is what an egui container closure (`ui.vertical(|ui| ..)`) uses to
    /// re-enter the tree with the inner `Ui`.
    pub fn new(store: &'s Store, ui: &'u mut egui::Ui, scope: egui::Id) -> Self {
        Self {
            store,
            surface: Surface::Ui(ui),
            scope,
        }
    }

    /// Build a `Cx` for a taffy node under the scope id `scope`.
    pub fn new_taffy(store: &'s Store, tui: &'u mut Tui, scope: egui::Id) -> Self {
        Self {
            store,
            surface: Surface::Taffy(tui),
            scope,
        }
    }

    /// The egui `Ui` currently being drawn into.
    ///
    /// In taffy mode this is the `Ui` of the current taffy node, so anything
    /// drawn through it is *not* laid out by taffy. Elements use
    /// [`Cx::leaf`] and [`Cx::container`] instead; this is the escape hatch for
    /// user code that wants to call egui directly.
    pub fn ui(&mut self) -> &mut egui::Ui {
        match &mut self.surface {
            Surface::Ui(ui) => ui,
            Surface::Taffy(tui) => tui.egui_ui_mut(),
        }
    }

    /// Whether this `Cx` is inside a taffy container.
    pub fn in_taffy(&self) -> bool {
        matches!(self.surface, Surface::Taffy(_))
    }

    /// The id of the current component scope; the base of every hook id.
    pub fn scope_id(&self) -> egui::Id {
        self.scope
    }

    /// The egui context.
    pub fn ctx(&self) -> &egui::Context {
        match &self.surface {
            Surface::Ui(ui) => ui.ctx(),
            Surface::Taffy(tui) => tui.egui_ctx(),
        }
    }

    /// Enter a component scope: deepen the hook scope and the egui id.
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
        match &mut self.surface {
            Surface::Ui(ui) => {
                ui.push_id(source, |ui| {
                    let mut cx = Cx::new(store, ui, scope);
                    f(&mut cx)
                })
                .inner
            }
            // A taffy node's auto id is derived from the parent's auto id
            // prefix, so swapping the prefix separates the egui ids of two
            // instances of the same subtree the way `push_id` does.
            Surface::Taffy(tui) => tui.with_auto_id_prefix(scope, |tui| {
                let mut cx = Cx::new_taffy(store, tui, scope);
                f(&mut cx)
            }),
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
        let mut cx = Cx {
            store,
            surface: self.reborrow(),
            scope,
        };
        f(&mut cx)
    }

    /// Draw one egui widget, as a taffy leaf when inside a container.
    ///
    /// Outside a container `style` is ignored: a plain `Ui` has nothing to
    /// apply flex item properties to.
    pub fn leaf<R>(&mut self, style: &ItemStyle, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        match &mut self.surface {
            Surface::Ui(ui) => f(ui),
            Surface::Taffy(tui) => (&mut **tui).style(style.to_taffy()).ui(f),
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
        match &mut self.surface {
            Surface::Ui(ui) => f(ui),
            Surface::Taffy(tui) => {
                (&mut **tui)
                    .style(style.to_taffy())
                    .ui_manual(|ui, _container| egui_taffy::TuiContainerResponse {
                        inner: f(ui),
                        min_size: egui::Vec2::ZERO,
                        intrinsic_size: None,
                        max_size: egui::Vec2::INFINITY,
                        infinite: egui::Vec2b::TRUE,
                    })
            }
        }
    }

    /// Open a taffy container and draw `f` inside it.
    ///
    /// Outside taffy this starts a new `egui_taffy` tree in the current `Ui`;
    /// inside taffy it adds a child node. Either way `f` receives a `Cx` in
    /// taffy mode, so its direct children become taffy nodes.
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
        match &mut self.surface {
            Surface::Ui(ui) => {
                let tui = egui_taffy::tui(ui, id);
                let tui = if all_space {
                    tui.reserve_available_space()
                } else {
                    tui.reserve_available_width()
                };
                tui.style(style).show(|tui| {
                    let mut cx = Cx::new_taffy(store, tui, scope);
                    f(&mut cx)
                })
            }
            Surface::Taffy(tui) => (&mut **tui).id(id).style(style).add(|tui| {
                let mut cx = Cx::new_taffy(store, tui, scope);
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
            Surface::Taffy(tui) => Surface::Taffy(tui),
        }
    }
}
