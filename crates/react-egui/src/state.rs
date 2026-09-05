//! [`State`], the hook guard, and [`Handle`], its `Copy` accessor.

use std::cell::RefMut;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::panic::Location;

use crate::store::{Slot, Store};

/// Queue a write to the slot `id` for the end of the pass.
///
/// The closure is `'static`: it outlives the component body, so it cannot hold
/// the slot reference and has to look the slot up again when it runs.
fn queue_update<T: 'static>(store: &Store, id: egui::Id, f: impl FnOnce(&mut T) + 'static) {
    store.defer_raw(Box::new(move |store| {
        // The slot is gone if the component unmounted; drop the write.
        let Some(slot) = store.slot_by_id(id) else {
            return;
        };
        f(&mut slot.borrow_mut::<T>());
        store.ctx().request_repaint();
    }));
}

/// A guard over one `use_state` slot.
///
/// Derefs to `T`, so `*count += 1` works. The value lives in the store, so
/// nothing is written back on drop; the only thing drop does is request a
/// repaint if the value was mutably dereferenced.
pub struct State<'s, T: 'static> {
    // `Option` so that `into_handle` can release the `RefCell` borrow without
    // moving out of a type that implements `Drop` (which would need `unsafe`).
    inner: Option<RefMut<'s, T>>,
    slot: &'s Slot,
    store: &'s Store,
    dirty: bool,
}

impl<'s, T: 'static> State<'s, T> {
    pub(crate) fn new(
        store: &'s Store,
        slot: &'s Slot,
        location: &'static Location<'static>,
    ) -> Self {
        // A slot that is already borrowed means two hooks share one id and the
        // first guard is still alive. Report that instead of the bare
        // `RefCell` "already mutably borrowed" panic.
        let inner = slot.try_borrow_mut::<T>().unwrap_or_else(|| {
            panic!(
                "react-egui: hook id collision at {location}: the same hook \
                 slot is already borrowed in this pass. Wrap custom hooks in \
                 #[hook] (hook_scope) or add key= inside loops."
            )
        });
        Self {
            inner: Some(inner),
            slot,
            store,
            dirty: false,
        }
    }

    /// Consume the guard and return a `Copy` [`Handle`] to the same slot.
    ///
    /// This releases the guard's borrow. There is deliberately no
    /// `handle(&self)`: using a `Handle` while the guard is alive would
    /// double-borrow the same `RefCell` and panic.
    pub fn into_handle(mut self) -> Handle<'s, T> {
        self.inner = None;
        Handle {
            slot: self.slot,
            store: self.store,
            _t: PhantomData,
        }
        // `self` is dropped here, requesting a repaint if it was mutated.
    }

    /// Queue a write for the end of the pass and request a repaint.
    ///
    /// This is the way out of "the loop borrows the state, so the handler
    /// inside it cannot": `todos.update_later(move |t| t.remove(i))`. The
    /// closure is `'static`, so captures need `move`.
    ///
    /// Calling this while the guard is alive is fine: the write happens long
    /// after the guard is gone.
    pub fn update_later(&self, f: impl FnOnce(&mut T) + 'static) {
        queue_update(self.store, self.slot.id(), f);
    }
}

impl<T: 'static> Deref for State<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.inner.as_ref().expect("state guard already released")
    }
}

impl<T: 'static> DerefMut for State<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.dirty = true;
        self.inner.as_mut().expect("state guard already released")
    }
}

impl<T: 'static> Drop for State<'_, T> {
    fn drop(&mut self) {
        if self.dirty {
            self.store.ctx().request_repaint();
        }
    }
}

/// A `Copy` accessor to one hook slot.
///
/// Unlike [`State`] it borrows the slot only for the duration of each call, so
/// it can be stored in a struct or handed to a child component.
pub struct Handle<'s, T: 'static> {
    slot: &'s Slot,
    store: &'s Store,
    _t: PhantomData<fn() -> T>,
}

impl<T: 'static> Clone for Handle<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for Handle<'_, T> {}

impl<T: 'static> Handle<'_, T> {
    /// Read a clone of the value.
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.slot.borrow::<T>().clone()
    }

    /// Read the value through a closure.
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.slot.borrow::<T>())
    }

    /// Replace the value and request a repaint.
    pub fn set(&self, value: T) {
        *self.slot.borrow_mut::<T>() = value;
        self.store.ctx().request_repaint();
    }

    /// The store id of the slot this handle points at.
    pub(crate) fn slot_id(&self) -> egui::Id {
        self.slot.id()
    }

    /// Mutate the value in place and request a repaint.
    pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let r = f(&mut self.slot.borrow_mut::<T>());
        self.store.ctx().request_repaint();
        r
    }

    /// Queue a write for the end of the pass and request a repaint.
    ///
    /// See [`State::update_later`].
    pub fn update_later(&self, f: impl FnOnce(&mut T) + 'static) {
        queue_update(self.store, self.slot.id(), f);
    }
}

impl<'s, T: 'static> Handle<'s, T> {
    pub(crate) fn new(store: &'s Store, slot: &'s Slot) -> Self {
        Self {
            slot,
            store,
            _t: PhantomData,
        }
    }
}
