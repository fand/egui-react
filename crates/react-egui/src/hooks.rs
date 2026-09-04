//! The hooks: `use_state`, `use_handle`, `use_effect`.

use std::any::Any;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::panic::Location;

use crate::cx::{Cx, location_key};
use crate::state::{Handle, State};

/// Hold `T` in the store, initialising it on the first visit.
///
/// Returns a guard that derefs to `T`. The guard borrows the store, not the
/// `Cx`, so sibling event handlers can each take it mutably in turn.
#[track_caller]
pub fn use_state<'s, T: 'static>(cx: &mut Cx<'s, '_>, init: impl FnOnce() -> T) -> State<'s, T> {
    let location = Location::caller();
    let store = cx.store;
    let id = cx.scope_id().with(location_key(location));
    let slot = store.slot(id, location, || Box::new(init()) as Box<dyn Any>);
    State::new(slot, store.ctx())
}

/// Like [`use_state`], but returns a `Copy` [`Handle`] instead of a guard.
#[track_caller]
pub fn use_handle<'s, T: 'static>(cx: &mut Cx<'s, '_>, init: impl FnOnce() -> T) -> Handle<'s, T> {
    let location = Location::caller();
    let store = cx.store;
    let id = cx.scope_id().with(location_key(location));
    let slot = store.slot(id, location, || Box::new(init()) as Box<dyn Any>);
    Handle::new(slot, store.ctx())
}

/// Marker for an effect body that returns no cleanup.
pub struct NoCleanup;

/// Marker for an effect body that returns a cleanup closure.
pub struct FnCleanup;

/// What an effect body may return.
///
/// The `Marker` type parameter exists only to keep the `()` and
/// `FnOnce() + 'static` impls from overlapping; it is always inferred.
pub trait IntoCleanup<Marker> {
    /// Convert into a stored cleanup, if any.
    fn into_cleanup(self) -> Option<Box<dyn FnOnce()>>;
}

impl IntoCleanup<NoCleanup> for () {
    fn into_cleanup(self) -> Option<Box<dyn FnOnce()>> {
        None
    }
}

impl<F: FnOnce() + 'static> IntoCleanup<FnCleanup> for F {
    fn into_cleanup(self) -> Option<Box<dyn FnOnce()>> {
        Some(Box::new(self))
    }
}

/// Run `f` on the first visit and whenever the hash of `deps` changes.
///
/// The body runs *at the call site*, not after the pass, so it can borrow
/// locals and `State` guards. The cleanup it returns is stored and run before
/// the next body and on unmount.
#[track_caller]
pub fn use_effect<D, C, M>(cx: &mut Cx<'_, '_>, deps: D, f: impl FnOnce() -> C)
where
    D: Hash,
    C: IntoCleanup<M>,
{
    let location = Location::caller();
    let store = cx.store;
    let id = cx.scope_id().with(location_key(location));
    let slot = store.slot(id, location, || Box::new(()) as Box<dyn Any>);

    let mut hasher = DefaultHasher::new();
    deps.hash(&mut hasher);
    let hash = hasher.finish();

    if slot.deps_hash() == Some(hash) {
        return;
    }
    if let Some(cleanup) = slot.take_cleanup() {
        cleanup();
    }
    let cleanup = f().into_cleanup();
    slot.set_cleanup(cleanup);
    slot.set_deps_hash(hash);
}
