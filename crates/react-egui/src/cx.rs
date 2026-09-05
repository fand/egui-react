//! [`Cx`]: the context threaded through every component and hook.

use std::fmt::Debug;
use std::hash::Hash;
use std::panic::Location;

use crate::store::Store;

/// Hash key derived from a `#[track_caller]` call site.
pub(crate) fn location_key(location: &'static Location<'static>) -> (&'static str, u32, u32) {
    (location.file(), location.line(), location.column())
}

/// The context passed to components and hooks.
///
/// `'s` is the lifetime of the [`Store`] and of every hook handle taken from
/// it; it is independent of the `&mut Cx` borrow, so a hook can return a guard
/// that outlives the borrow of the `Cx` it was taken from.
pub struct Cx<'s, 'u> {
    /// The hook store. `Copy`, so it can be re-borrowed into nested closures.
    pub store: &'s Store,
    /// The egui `Ui` currently being drawn into.
    pub ui: &'u mut egui::Ui,
    scope: egui::Id,
}

impl<'s, 'u> Cx<'s, 'u> {
    /// Build a `Cx` for `ui` under the scope id `scope`.
    ///
    /// This is what an egui container closure (`ui.vertical(|ui| ..)`) uses to
    /// re-enter the tree with the inner `Ui`.
    pub fn new(store: &'s Store, ui: &'u mut egui::Ui, scope: egui::Id) -> Self {
        Self { store, ui, scope }
    }

    /// The id of the current component scope; the base of every hook id.
    pub fn scope_id(&self) -> egui::Id {
        self.scope
    }

    /// The egui context.
    pub fn ctx(&self) -> &egui::Context {
        self.ui.ctx()
    }

    /// Enter a component scope: deepen the hook scope and `ui.push_id`.
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
        self.ui
            .push_id(source, |ui| {
                let mut cx = Cx::new(store, ui, scope);
                f(&mut cx)
            })
            .inner
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
        let store = self.store;
        let scope = self.scope.with(location_key(location));
        let mut cx = Cx::new(store, &mut *self.ui, scope);
        f(&mut cx)
    }
}
