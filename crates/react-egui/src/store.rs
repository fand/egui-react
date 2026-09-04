//! The hook state store: a map from [`egui::Id`] to a slot holding one hook's value.

use std::any::{Any, TypeId};
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::panic::Location;

use elsa::FrozenMap;

/// A single hook's storage.
///
/// Slots are boxed inside a [`FrozenMap`], so a `&Slot` handed out by
/// [`Store::slot`] stays valid while later hooks insert further slots.
pub(crate) struct Slot {
    id: egui::Id,
    value: RefCell<Box<dyn Any>>,
    last_visited: Cell<u64>,
    cleanup: RefCell<Option<Box<dyn FnOnce()>>>,
    deps_hash: Cell<Option<u64>>,
    #[allow(dead_code)]
    location: &'static Location<'static>,
}

impl Slot {
    /// The id this slot is stored under.
    pub(crate) fn id(&self) -> egui::Id {
        self.id
    }

    /// Borrow the slot value as `T`.
    pub(crate) fn borrow<T: 'static>(&self) -> Ref<'_, T> {
        Ref::map(self.value.borrow(), |v| {
            (**v).downcast_ref::<T>().expect("hook slot type mismatch")
        })
    }

    /// Mutably borrow the slot value as `T`.
    pub(crate) fn borrow_mut<T: 'static>(&self) -> RefMut<'_, T> {
        RefMut::map(self.value.borrow_mut(), |v| {
            (**v).downcast_mut::<T>().expect("hook slot type mismatch")
        })
    }

    /// Mutably borrow the slot value as `T`, or `None` if it is already borrowed.
    pub(crate) fn try_borrow_mut<T: 'static>(&self) -> Option<RefMut<'_, T>> {
        let value = self.value.try_borrow_mut().ok()?;
        Some(RefMut::map(value, |v| {
            (**v).downcast_mut::<T>().expect("hook slot type mismatch")
        }))
    }

    /// The deps hash recorded by the last `use_effect` run, if any.
    pub(crate) fn deps_hash(&self) -> Option<u64> {
        self.deps_hash.get()
    }

    /// Record the deps hash of the current `use_effect` run.
    pub(crate) fn set_deps_hash(&self, hash: u64) {
        self.deps_hash.set(Some(hash));
    }

    /// Take the stored cleanup, leaving none behind.
    pub(crate) fn take_cleanup(&self) -> Option<Box<dyn FnOnce()>> {
        self.cleanup.borrow_mut().take()
    }

    /// Store the cleanup to run on the next deps change or on unmount.
    pub(crate) fn set_cleanup(&self, cleanup: Option<Box<dyn FnOnce()>>) {
        *self.cleanup.borrow_mut() = cleanup;
    }
}

/// A hook id that was requested twice within one pass.
///
/// This means two hooks share an id, usually because a custom hook is missing
/// `#[hook]` / `hook_scope`, or because a `key` is missing inside a loop.
#[derive(Clone, Copy, Debug)]
pub struct Collision {
    /// The id that was requested twice.
    pub id: egui::Id,
    /// The call site that requested it the second time.
    pub location: &'static Location<'static>,
}

/// Owns every hook's state, keyed by [`egui::Id`].
///
/// The runner (or a test) calls [`Store::begin_pass`] before the tree is drawn
/// and [`Store::end_pass`] after every guard has been dropped.
pub struct Store {
    slots: FrozenMap<egui::Id, Box<Slot>>,
    pass: Cell<u64>,
    ctx: egui::Context,
    collisions: RefCell<Vec<Collision>>,
    /// The `provide_context` stack: the slot id each type is currently bound to.
    contexts: RefCell<Vec<(TypeId, egui::Id)>>,
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Store {
    /// Create an empty store.
    ///
    /// The [`egui::Context`] used for repaint requests is a placeholder until
    /// the first [`Store::begin_pass`].
    pub fn new() -> Self {
        Self {
            slots: FrozenMap::new(),
            pass: Cell::new(0),
            ctx: egui::Context::default(),
            collisions: RefCell::new(Vec::new()),
            contexts: RefCell::new(Vec::new()),
        }
    }

    /// Start a new pass: bump the pass counter and adopt `ctx` for repaints.
    pub fn begin_pass(&mut self, ctx: &egui::Context) {
        self.ctx = ctx.clone();
        self.pass.set(self.pass.get() + 1);
        self.collisions.borrow_mut().clear();
        self.contexts.borrow_mut().clear();
    }

    /// Finish the pass: drop every slot that was not visited and run its cleanup.
    ///
    /// Every [`crate::State`] guard must have been dropped before this is called.
    pub fn end_pass(&mut self) {
        let pass = self.pass.get();
        let mut cleanups = Vec::new();
        let map: &mut std::collections::HashMap<egui::Id, Box<Slot>> = self.slots.as_mut();
        map.retain(|_, slot| {
            let alive = slot.last_visited.get() >= pass;
            if !alive {
                cleanups.extend(slot.take_cleanup());
            }
            alive
        });
        // Cleanups are `'static` and cannot reach back into the store, but they
        // are still run after the map borrow ends, to keep that obvious.
        for cleanup in cleanups {
            cleanup();
        }
    }

    /// The context repaints are requested on.
    pub fn ctx(&self) -> &egui::Context {
        &self.ctx
    }

    /// The current pass number. Starts at 1 after the first `begin_pass`.
    pub fn pass(&self) -> u64 {
        self.pass.get()
    }

    /// Id collisions recorded during the current pass.
    pub fn collisions(&self) -> Vec<Collision> {
        self.collisions.borrow().clone()
    }

    /// Number of live slots.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether the store holds no slots.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Look up a slot without visiting it.
    ///
    /// Used by `use_context`, which reaches a slot some ancestor already
    /// visited this pass rather than declaring a hook of its own.
    pub(crate) fn slot_by_id(&self, id: egui::Id) -> Option<&Slot> {
        self.slots.get(&id)
    }

    /// Push a context binding for the duration of a subtree.
    pub(crate) fn push_context(&self, type_id: TypeId, slot: egui::Id) {
        self.contexts.borrow_mut().push((type_id, slot));
    }

    /// Pop the most recent context binding.
    pub(crate) fn pop_context(&self) {
        self.contexts.borrow_mut().pop();
    }

    /// The innermost binding for `type_id`, if any.
    pub(crate) fn lookup_context(&self, type_id: TypeId) -> Option<egui::Id> {
        self.contexts
            .borrow()
            .iter()
            .rev()
            .find(|(t, _)| *t == type_id)
            .map(|(_, id)| *id)
    }

    /// Get the slot for `id`, creating it with `init` on first visit.
    ///
    /// Visiting the same id twice in one pass is a collision and is recorded.
    pub(crate) fn slot(
        &self,
        id: egui::Id,
        location: &'static Location<'static>,
        init: impl FnOnce() -> Box<dyn Any>,
    ) -> &Slot {
        let pass = self.pass.get();
        if let Some(slot) = self.slots.get(&id) {
            if slot.last_visited.get() == pass {
                self.collisions
                    .borrow_mut()
                    .push(Collision { id, location });
                log::warn!("react-egui: hook id collision at {location} (id {id:?})");
            }
            slot.last_visited.set(pass);
            return slot;
        }
        self.slots.insert(
            id,
            Box::new(Slot {
                id,
                value: RefCell::new(init()),
                last_visited: Cell::new(pass),
                cleanup: RefCell::new(None),
                deps_hash: Cell::new(None),
                location,
            }),
        )
    }
}
