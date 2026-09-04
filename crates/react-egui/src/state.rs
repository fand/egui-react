//! [`State`], the hook guard, and [`Handle`], its `Copy` accessor.

use std::cell::RefMut;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::panic::Location;

use crate::store::Slot;

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
    ctx: &'s egui::Context,
    dirty: bool,
}

impl<'s, T: 'static> State<'s, T> {
    pub(crate) fn new(
        slot: &'s Slot,
        ctx: &'s egui::Context,
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
            ctx,
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
            ctx: self.ctx,
            _t: PhantomData,
        }
        // `self` is dropped here, requesting a repaint if it was mutated.
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
            self.ctx.request_repaint();
        }
    }
}

/// A `Copy` accessor to one hook slot.
///
/// Unlike [`State`] it borrows the slot only for the duration of each call, so
/// it can be stored in a struct or handed to a child component.
pub struct Handle<'s, T: 'static> {
    slot: &'s Slot,
    ctx: &'s egui::Context,
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
        self.ctx.request_repaint();
    }

    /// Mutate the value in place and request a repaint.
    pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let r = f(&mut self.slot.borrow_mut::<T>());
        self.ctx.request_repaint();
        r
    }
}

impl<'s, T: 'static> Handle<'s, T> {
    pub(crate) fn new(slot: &'s Slot, ctx: &'s egui::Context) -> Self {
        Self {
            slot,
            ctx,
            _t: PhantomData,
        }
    }
}
