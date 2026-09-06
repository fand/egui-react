//! The hook state store: a map from [`egui::Id`] to a slot holding one hook's value.

use std::any::{Any, TypeId};
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::{BTreeSet, HashMap};
use std::panic::Location;
use std::rc::Rc;

use elsa::{FrozenMap, FrozenVec};

use crate::engine::Tree;

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
    /// `use_memo` values, newest last.
    ///
    /// Old entries are kept until the end of the pass because a `&'s T` handed
    /// out earlier in the same pass may still point at one.
    memo: FrozenVec<Box<dyn Any>>,
    /// `use_persisted` bookkeeping: the storage key and how to serialise `T`.
    persist: Option<(String, ToJson)>,
    #[allow(dead_code)]
    location: &'static Location<'static>,
}

/// Serialise a slot value, given its concrete type at the `use_persisted` call.
pub(crate) type ToJson = fn(&dyn Any) -> Option<String>;

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

    /// The deps hash recorded by the last `use_effect` / `use_memo` run, if any.
    pub(crate) fn deps_hash(&self) -> Option<u64> {
        self.deps_hash.get()
    }

    /// Record the deps hash of the current `use_effect` / `use_memo` run.
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

    /// The newest memo value, if `use_memo` ever ran here.
    pub(crate) fn memo_last(&self) -> Option<&dyn Any> {
        let len = self.memo.len();
        (len > 0).then(|| &self.memo[len - 1])
    }

    /// Append a memo value and borrow it for as long as the slot lives.
    pub(crate) fn memo_push(&self, value: Box<dyn Any>) -> &dyn Any {
        self.memo.push_get(value)
    }

    /// The storage key and serialiser, if this slot came from `use_persisted`.
    fn persist(&self) -> Option<&(String, ToJson)> {
        self.persist.as_ref()
    }

    /// Serialise the current value for persistence, if this slot is persisted.
    fn to_json(&self) -> Option<(String, String)> {
        let (key, to_json) = self.persist()?;
        let value = self.value.try_borrow().ok()?;
        Some((key.clone(), to_json(&**value)?))
    }

    /// Drop every memo value but the newest.
    ///
    /// Safe only between passes: within a pass, references handed out earlier
    /// may still point at the older entries.
    fn prune_memo(&mut self) {
        let memo = self.memo.as_mut();
        if memo.len() > 1 {
            memo.drain(..memo.len() - 1);
        }
    }
}

/// One piece of work queued by `cx.defer` or `update_later`.
pub(crate) type Deferred = Box<dyn FnOnce(&Store)>;

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

/// The overlay text shown for one colliding call site.
fn collision_message(location: &Location<'static>) -> String {
    format!(
        "egui-react: hook id collision at {}:{}:{}. Wrap custom hooks in #[hook], or add key= inside loops.",
        location.file(),
        location.line(),
        location.column(),
    )
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
    /// The `<Suspense>` stack: how many `use_future`s are pending in each open
    /// boundary, innermost last.
    suspense: RefCell<Vec<usize>>,
    /// Work queued by `cx.defer` and `update_later`, applied in `end_pass`.
    deferred: RefCell<Vec<Deferred>>,
    /// `use_persisted` values as JSON, keyed by the user's string key.
    persisted: RefCell<HashMap<String, String>>,
    /// Every key `use_persisted` has been called with in this process.
    persisted_keys: RefCell<BTreeSet<String>>,
    /// One taffy tree per `<View>` root, keyed by the root's layout id.
    ///
    /// Here rather than in egui memory: one map lookup per tree per frame with
    /// no lock, and [`Store::end_pass`] drops a tree an unmounted subtree left
    /// behind instead of keeping it in egui's `IdTypeMap` forever. Each tree is
    /// behind its own `Rc<RefCell<..>>` so that a tree opened inside a leaf of
    /// another one does not run into the outer tree's borrow.
    trees: RefCell<HashMap<egui::Id, Rc<RefCell<Tree>>>>,
    warn_on_collision: bool,
}

/// The slot id of a `use_persisted` key.
///
/// Derived from the key alone, never from the call site: a persisted value has
/// to survive edits to the source (3.4).
pub(crate) fn persisted_id(key: &str) -> egui::Id {
    egui::Id::new(("egui_react_persisted", key))
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
            suspense: RefCell::new(Vec::new()),
            deferred: RefCell::new(Vec::new()),
            persisted: RefCell::new(HashMap::new()),
            persisted_keys: RefCell::new(BTreeSet::new()),
            trees: RefCell::new(HashMap::new()),
            warn_on_collision: cfg!(debug_assertions),
        }
    }

    /// Start a new pass: bump the pass counter and adopt `ctx` for repaints.
    pub fn begin_pass(&mut self, ctx: &egui::Context) {
        self.ctx = ctx.clone();
        self.pass.set(self.pass.get() + 1);
        self.collisions.borrow_mut().clear();
        self.contexts.borrow_mut().clear();
        self.suspense.borrow_mut().clear();
    }

    /// Finish the pass: apply the deferred queue, then sweep, then warn.
    ///
    /// Every [`crate::State`] guard must have been dropped before this is
    /// called. The deferred queue runs first so that a queued write lands on a
    /// slot that is still alive, and the collision overlay last so that it is
    /// painted over the frame it describes.
    pub fn end_pass(&mut self) {
        self.run_deferred();
        self.sweep();
        self.sweep_trees();
        self.show_collision_overlay();
    }

    /// Drop every layout tree that was not drawn this pass.
    ///
    /// A tree belongs to the `<View>` root that opened it, so a subtree that
    /// unmounted takes its trees with it.
    fn sweep_trees(&mut self) {
        let pass = self.pass.get();
        self.trees
            .borrow_mut()
            .retain(|_, tree| tree.borrow().last_visited() >= pass);
    }

    /// Apply everything `cx.defer` / `update_later` queued, until nothing is left.
    fn run_deferred(&mut self) {
        loop {
            let batch: Vec<Deferred> = std::mem::take(&mut *self.deferred.borrow_mut());
            if batch.is_empty() {
                return;
            }
            for f in batch {
                f(self);
            }
        }
    }

    /// Drop every slot that was not visited this pass and run its cleanup.
    fn sweep(&mut self) {
        let pass = self.pass.get();
        let mut cleanups = Vec::new();
        let mut saved: Vec<(String, String)> = Vec::new();
        let map: &mut HashMap<egui::Id, Box<Slot>> = self.slots.as_mut();
        map.retain(|_, slot| {
            let alive = slot.last_visited.get() >= pass;
            if alive {
                slot.prune_memo();
            } else {
                // Unmounted, but a persisted value must still survive to the
                // next launch, so it is serialised before the slot is dropped.
                saved.extend(slot.to_json());
                cleanups.extend(slot.take_cleanup());
            }
            alive
        });
        if !saved.is_empty() {
            let mut persisted = self.persisted.borrow_mut();
            persisted.extend(saved);
        }
        // Cleanups are `'static` and cannot reach back into the store, but they
        // are still run after the map borrow ends, to keep that obvious.
        for cleanup in cleanups {
            cleanup();
        }
    }

    /// Paint the debug overlay listing this pass's id collisions.
    fn show_collision_overlay(&self) {
        if !self.warn_on_collision {
            return;
        }
        let messages: BTreeSet<String> = self
            .collisions
            .borrow()
            .iter()
            .map(|c| collision_message(c.location))
            .collect();
        if messages.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("egui_react_collision_warning"))
            .order(egui::Order::Debug)
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(8.0, 8.0))
            .show(&self.ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    for message in &messages {
                        ui.colored_label(egui::Color32::RED, message);
                    }
                });
            });
    }

    /// Whether id collisions are drawn as an on-screen overlay.
    ///
    /// Defaults to `cfg!(debug_assertions)`.
    pub fn warn_on_collision(&self) -> bool {
        self.warn_on_collision
    }

    /// Turn the collision overlay on or off.
    pub fn set_warn_on_collision(&mut self, warn: bool) {
        self.warn_on_collision = warn;
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

    /// Number of live layout trees, one per `<View>` root drawn last pass.
    ///
    /// For tests: it is how a test asks "did scrolling a list build a tree per
    /// row?" without reaching into egui memory.
    pub fn tree_count(&self) -> usize {
        self.trees.borrow().len()
    }

    /// The layout tree keyed by `id`, created on first use and marked as drawn
    /// in this pass.
    pub(crate) fn tree(&self, id: egui::Id) -> Rc<RefCell<Tree>> {
        let pass = self.pass.get();
        let tree = {
            let mut trees = self.trees.borrow_mut();
            Rc::clone(trees.entry(id).or_insert_with(crate::engine::new_tree))
        };
        tree.borrow_mut().visit(pass);
        tree
    }

    /// Enter a `<Suspense>` boundary: push a counter of its own.
    ///
    /// The stack is cleared by [`Store::begin_pass`], so an early return out of
    /// a boundary cannot leak a counter into the next pass.
    pub fn begin_suspense(&self) {
        self.suspense.borrow_mut().push(0);
    }

    /// Leave the innermost `<Suspense>` boundary.
    ///
    /// Returns how many `use_future`s were pending inside it. Nested boundaries
    /// keep their own count, so this only reports what nothing inner caught.
    pub fn end_suspense(&self) -> usize {
        self.suspense.borrow_mut().pop().unwrap_or(0)
    }

    /// Count one pending `use_future` against the innermost boundary.
    ///
    /// Does nothing outside a boundary: a `use_future` with no `<Suspense>`
    /// above it just returns `Poll::Pending` to its component.
    pub fn note_pending(&self) {
        if let Some(count) = self.suspense.borrow_mut().last_mut() {
            *count += 1;
        }
    }

    /// Queue work to run at the end of the pass.
    ///
    /// The closure gets the store back, which is how `update_later` reaches
    /// its slot without borrowing it across the pass.
    pub(crate) fn defer_raw(&self, f: Deferred) {
        self.deferred.borrow_mut().push(f);
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
                log::warn!("egui-react: hook id collision at {location} (id {id:?})");
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
                memo: FrozenVec::new(),
                persist: None,
                location,
            }),
        )
    }

    /// The JSON `load_persisted` restored for `key`, if any.
    pub(crate) fn persisted_json(&self, key: &str) -> Option<String> {
        self.persisted.borrow().get(key).cloned()
    }

    /// Get the slot behind a `use_persisted` key, creating it with `init`.
    pub(crate) fn persisted_slot(
        &self,
        key: &str,
        location: &'static Location<'static>,
        to_json: ToJson,
        init: impl FnOnce() -> Box<dyn Any>,
    ) -> &Slot {
        let id = persisted_id(key);
        self.persisted_keys.borrow_mut().insert(key.to_owned());
        let pass = self.pass.get();
        if let Some(slot) = self.slots.get(&id) {
            if slot.last_visited.get() == pass {
                self.collisions
                    .borrow_mut()
                    .push(Collision { id, location });
                log::warn!("egui-react: use_persisted key collision at {location} (key {key:?})");
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
                memo: FrozenVec::new(),
                persist: Some((key.to_owned(), to_json)),
                location,
            }),
        )
    }

    /// Restore what [`Store::save_persisted`] produced.
    ///
    /// Call this before the first pass; keys that no `use_persisted` asks for
    /// are kept as they are, so an unused value survives a run that never
    /// mounted its component.
    pub fn load_persisted(&mut self, json: &str) {
        match serde_json::from_str::<HashMap<String, String>>(json) {
            Ok(map) => *self.persisted.borrow_mut() = map,
            Err(err) => log::warn!("egui-react: could not read persisted state: {err}"),
        }
    }

    /// Serialise every `use_persisted` value into one JSON string.
    ///
    /// Values whose component is currently mounted are read from their slot;
    /// unmounted ones come from what the sweep saved.
    pub fn save_persisted(&self) -> String {
        {
            let keys: Vec<String> = self.persisted_keys.borrow().iter().cloned().collect();
            let mut persisted = self.persisted.borrow_mut();
            for key in keys {
                if let Some(slot) = self.slots.get(&persisted_id(&key))
                    && let Some((key, json)) = slot.to_json()
                {
                    persisted.insert(key, json);
                }
            }
        }
        serde_json::to_string(&*self.persisted.borrow()).unwrap_or_else(|err| {
            log::warn!("egui-react: could not write persisted state: {err}");
            String::from("{}")
        })
    }
}
