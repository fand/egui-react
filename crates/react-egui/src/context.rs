//! `provide_context` / `use_context`: values passed down a subtree by type.

use std::any::TypeId;

use crate::cx::Cx;
use crate::state::Handle;

/// Make `value` visible to `use_context::<T>` inside `children`.
///
/// The binding lasts exactly as long as `children` runs. What is stored is the
/// slot id, not the handle, so the store needs no lifetime parameter and no
/// `unsafe`.
pub fn provide_context<'s, T: 'static>(
    cx: &mut Cx<'s, '_>,
    value: Handle<'s, T>,
    children: impl FnOnce(&mut Cx<'s, '_>),
) {
    let store = cx.store;
    store.push_context(TypeId::of::<T>(), value.slot_id());
    children(cx);
    store.pop_context();
}

/// Read the innermost [`Handle`] provided for `T`, if any.
///
/// A [`Handle`] rather than a guard: the provider usually still holds its own
/// state, and two guards on one slot would double-borrow.
pub fn use_context<'s, T: 'static>(cx: &Cx<'s, '_>) -> Option<Handle<'s, T>> {
    let store = cx.store;
    let id = store.lookup_context(TypeId::of::<T>())?;
    let slot = store.slot_by_id(id)?;
    Some(Handle::new(slot, store.ctx()))
}
