//! The hooks: `use_state`, `use_handle`, `use_memo`, `use_effect`, `use_persisted`.

use std::any::Any;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::panic::Location;

use crate::cx::{Cx, location_key};
use serde::Serialize;
use serde::de::DeserializeOwned;

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
    State::new(store, slot, location)
}

/// Like [`use_state`], but returns a `Copy` [`Handle`] instead of a guard.
#[track_caller]
pub fn use_handle<'s, T: 'static>(cx: &mut Cx<'s, '_>, init: impl FnOnce() -> T) -> Handle<'s, T> {
    let location = Location::caller();
    let store = cx.store;
    let id = cx.scope_id().with(location_key(location));
    let slot = store.slot(id, location, || Box::new(init()) as Box<dyn Any>);
    Handle::new(store, slot)
}

/// Hash `deps` the way `use_effect` and `use_memo` compare them.
///
/// `Hash` rather than `PartialEq` so that borrowed deps (`(&str, &[T])`) are
/// allowed; hash collisions are as unlikely as egui's own id collisions.
pub(crate) fn deps_hash<D: Hash>(deps: &D) -> u64 {
    let mut hasher = DefaultHasher::new();
    deps.hash(&mut hasher);
    hasher.finish()
}

/// Compute `f` on the first visit and whenever the hash of `deps` changes.
///
/// Returns a reference that lives as long as the store borrow, not as long as
/// the `&mut Cx`, so a memo can be read next to a `State` guard. Superseded
/// values are only dropped between passes, because a reference handed out
/// earlier in the same pass may still point at one.
#[track_caller]
pub fn use_memo<'s, D: Hash, T: 'static>(
    cx: &mut Cx<'s, '_>,
    deps: D,
    f: impl FnOnce() -> T,
) -> &'s T {
    let location = Location::caller();
    let store = cx.store;
    let id = cx.scope_id().with(location_key(location));
    let slot = store.slot(id, location, || Box::new(()) as Box<dyn Any>);

    let hash = deps_hash(&deps);
    let cached = match slot.memo_last() {
        Some(value) if slot.deps_hash() == Some(hash) => Some(value),
        _ => None,
    };
    let value = match cached {
        Some(value) => value,
        None => {
            let value = slot.memo_push(Box::new(f()) as Box<dyn Any>);
            slot.set_deps_hash(hash);
            value
        }
    };
    value.downcast_ref::<T>().expect("memo slot type mismatch")
}

/// Hold `T` in the store *and* in the app's storage, keyed by `key`.
///
/// Unlike every other hook the id comes from `key` alone, not from the call
/// site: a call site moves whenever the source is edited, and saved data has to
/// survive that (3.4). Two calls with the same key therefore share one value,
/// and calling it twice in one pass is a collision like any other.
///
/// The value is restored by [`crate::Store::load_persisted`] before the first
/// pass and written back by [`crate::Store::save_persisted`]; the runner wires
/// both to eframe's storage.
#[track_caller]
pub fn use_persisted<'s, T: Serialize + DeserializeOwned + 'static>(
    cx: &mut Cx<'s, '_>,
    key: &str,
    init: impl FnOnce() -> T,
) -> State<'s, T> {
    fn to_json<T: Serialize + 'static>(value: &dyn Any) -> Option<String> {
        serde_json::to_string(value.downcast_ref::<T>()?).ok()
    }

    let location = Location::caller();
    let store = cx.store;
    let slot = store.persisted_slot(key, location, to_json::<T>, || {
        let restored =
            store
                .persisted_json(key)
                .and_then(|json| match serde_json::from_str::<T>(&json) {
                    Ok(value) => Some(value),
                    Err(err) => {
                        log::warn!("egui-react: persisted value {key:?} could not be read: {err}");
                        None
                    }
                });
        Box::new(restored.unwrap_or_else(init)) as Box<dyn Any>
    });
    State::new(store, slot, location)
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

    let hash = deps_hash(&deps);
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
