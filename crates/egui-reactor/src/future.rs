//! `use_future` and [`spawn`]: work that spans frames.

use std::any::Any;
use std::cell::Cell;
use std::future::Future;
use std::hash::Hash;
use std::panic::Location;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::Poll;

use crate::cx::{Cx, location_key};
use crate::hooks::deps_hash;

/// A future [`use_future`] and [`spawn`] can run.
///
/// Native polls it on a thread of its own, so it has to be `Send`; wasm polls it
/// on the one browser thread, so it does not. That is the only difference
/// between the platforms, and it lives here so that user code names one trait.
#[cfg(not(target_arch = "wasm32"))]
pub trait SpawnFuture<T>: Future<Output = T> + Send + 'static {}

#[cfg(not(target_arch = "wasm32"))]
impl<T, F: Future<Output = T> + Send + 'static> SpawnFuture<T> for F {}

/// A future [`use_future`] and [`spawn`] can run.
///
/// See the native version: on wasm the future need not be `Send`.
#[cfg(target_arch = "wasm32")]
pub trait SpawnFuture<T>: Future<Output = T> + 'static {}

#[cfg(target_arch = "wasm32")]
impl<T, F: Future<Output = T> + 'static> SpawnFuture<T> for F {}

/// The one place that knows how a future is actually run.
mod task {
    /// Run `fut` on a thread of its own.
    ///
    /// One thread per future, no pool: this is meant for futures that wait
    /// (HTTP, file IO). A future that burns CPU should hand that off itself.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
        if let Err(err) = std::thread::Builder::new()
            .name(String::from("egui-reactor-future"))
            .spawn(move || pollster::block_on(fut))
        {
            // The future is dropped and the hook stays `Pending`. Losing a
            // thread is not worth taking the app down for.
            log::error!("egui-reactor: could not spawn a thread for a future: {err}");
        }
    }

    /// Run `fut` on the browser's event loop.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn spawn(fut: impl Future<Output = ()> + 'static) {
        wasm_bindgen_futures::spawn_local(fut);
    }
}

/// Run a future to completion and throw its result away.
///
/// This is the imperative counterpart of [`use_future`]: start something from an
/// event handler and report back with a
/// [`Dispatch`](crate::Dispatch), which is `Send` and requests a repaint.
///
/// ```no_run
/// # use egui_reactor::prelude::*;
/// # fn demo(dispatch: Dispatch<u32>) {
/// spawn(async move { dispatch.send(41) });
/// # }
/// ```
pub fn spawn(fut: impl SpawnFuture<()>) {
    task::spawn(fut);
}

/// Where a running future leaves its result for the next visit.
///
/// The cell is shared with the future, so it outlives the slot: a result that
/// arrives after the component unmounted is written to a cell nobody reads.
struct Inbox<T> {
    /// `(generation, value)`: which launch produced the result, and the result.
    cell: Arc<Mutex<Option<(u64, T)>>>,
    /// The generation of the most recent launch.
    generation: Cell<u64>,
}

/// Lock the inbox, ignoring poisoning: a panicking future must not wedge the app.
fn lock<T>(cell: &Mutex<Option<(u64, T)>>) -> MutexGuard<'_, Option<(u64, T)>> {
    cell.lock().unwrap_or_else(|e| e.into_inner())
}

/// Run the future `f` builds, and report where it has got to.
///
/// `f` is called at the call site whenever the hash of `deps` changes (and on
/// the first visit), so it can read locals and `State` guards while it builds
/// the future. The future itself is `'static`, so it has to own what it needs.
///
/// The result lands on the visit *after* the future finished; finishing asks for
/// a repaint, so that visit happens without the user touching anything. Changing
/// `deps` builds a new future and drops whatever the old one eventually returns;
/// it does not stop the old one, because a future cannot be stopped.
///
/// Like [`use_memo`](crate::use_memo) the reference borrows the store rather
/// than the `Cx`, so it can be read next to a `State` guard. A child component
/// waits with a `let`-`else`, which is where React would throw:
///
/// ```no_run
/// # use egui_reactor::prelude::*;
/// # fn load(url: String) -> impl Future<Output = String> + Send + 'static { async { url } }
/// #[component]
/// fn Body(cx: &mut Cx, url: &str) {
///     let text = use_future(cx, url, || load(url.to_owned()));
///     let Poll::Ready(text) = text else { return };
///     cx.ui().label(text.clone());
/// }
/// ```
///
/// While the hook is `Pending` it counts towards the nearest `<Suspense>`
/// boundary, which draws its fallback instead of its children.
#[track_caller]
pub fn use_future<'s, D, T, F>(cx: &mut Cx<'s, '_>, deps: D, f: impl FnOnce() -> F) -> &'s Poll<T>
where
    D: Hash,
    T: Send + 'static,
    F: SpawnFuture<T>,
{
    let location = Location::caller();
    let store = cx.store;
    let id = cx.scope_id().with(location_key(location));

    // Two slots, as in `use_reducer`: the state slot keeps the deps hash and the
    // `Poll` values, the inbox slot the cell the running future writes into.
    let inbox_slot = store.slot(id.with("__egui_reactor_future_inbox"), location, || {
        Box::new(Inbox::<T> {
            cell: Arc::new(Mutex::new(None)),
            generation: Cell::new(0),
        }) as Box<dyn Any>
    });
    let slot = store.slot(id, location, || Box::new(()) as Box<dyn Any>);

    let hash = deps_hash(&deps);
    if slot.deps_hash() != Some(hash) {
        let (cell, generation) = {
            let inbox = inbox_slot.borrow::<Inbox<T>>();
            inbox.generation.set(inbox.generation.get() + 1);
            (Arc::clone(&inbox.cell), inbox.generation.get())
        };
        let fut = f();
        let ctx = store.ctx().clone();
        // The launch is only ever `Pending` here: the result is picked up on the
        // next visit, which the repaint request below guarantees will come.
        slot.memo_push(Box::new(Poll::<T>::Pending) as Box<dyn Any>);
        slot.set_deps_hash(hash);
        task::spawn(async move {
            let value = fut.await;
            {
                let mut inbox = lock(&cell);
                // Two futures can finish in either order between two visits, so
                // an older one must not overwrite a newer one's result.
                let newer = inbox.as_ref().is_none_or(|(seen, _)| generation > *seen);
                if newer {
                    *inbox = Some((generation, value));
                }
            }
            ctx.request_repaint();
        });
    } else {
        let arrived = {
            let inbox = inbox_slot.borrow::<Inbox<T>>();
            // Taking unconditionally is what drops a stale result: its deps are
            // gone and the current future is still on its way.
            let taken = lock(&inbox.cell).take();
            taken.filter(|(generation, _)| *generation == inbox.generation.get())
        };
        if let Some((_, value)) = arrived {
            slot.memo_push(Box::new(Poll::Ready(value)) as Box<dyn Any>);
        }
    }

    let value = slot
        .memo_last()
        .expect("use_future pushes a Poll on every launch")
        .downcast_ref::<Poll<T>>()
        .expect("future slot type mismatch");
    if value.is_pending() {
        store.note_pending();
    }
    value
}
