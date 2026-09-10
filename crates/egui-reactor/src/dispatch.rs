//! [`Dispatch`] and `use_reducer`: the way to change state from outside a pass.

use std::any::Any;
use std::panic::Location;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::cx::{Cx, location_key};
use crate::state::State;

/// A `Clone + Send` sender into one `use_reducer`'s message queue.
///
/// This is the only hook handle that outlives a pass, so it is what a
/// background thread (and, later, `use_future`) holds to report back.
pub struct Dispatch<M> {
    queue: Arc<Mutex<Vec<M>>>,
    ctx: egui::Context,
}

impl<M> Clone for Dispatch<M> {
    fn clone(&self) -> Self {
        Self {
            queue: Arc::clone(&self.queue),
            ctx: self.ctx.clone(),
        }
    }
}

impl<M> std::fmt::Debug for Dispatch<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dispatch")
            .field("pending", &self.queue.lock().map(|q| q.len()).unwrap_or(0))
            .finish()
    }
}

impl<M> Dispatch<M> {
    fn new(queue: Arc<Mutex<Vec<M>>>, ctx: egui::Context) -> Self {
        Self { queue, ctx }
    }

    /// Queue a message and ask for a repaint.
    ///
    /// The reducer runs the next time the hook is visited, which the repaint
    /// request guarantees will happen.
    pub fn send(&self, msg: M) {
        lock(&self.queue).push(msg);
        self.ctx.request_repaint();
    }
}

/// Lock the queue, ignoring poisoning: a panicking reducer must not wedge the app.
fn lock<M>(queue: &Mutex<Vec<M>>) -> MutexGuard<'_, Vec<M>> {
    queue.lock().unwrap_or_else(|e| e.into_inner())
}

/// Hold `S` in the store and fold queued messages into it with `reducer`.
///
/// Messages are applied *when the hook is visited*, not at the end of the
/// pass. That keeps `reducer` an ordinary closure (a pass-end reducer would
/// have to be stored, hence `'static`) and saves a frame of latency for
/// messages that arrived from another thread.
#[track_caller]
pub fn use_reducer<'s, S: 'static, M: Send + 'static>(
    cx: &mut Cx<'s, '_>,
    mut reducer: impl FnMut(&mut S, M),
    init: impl FnOnce() -> S,
) -> (State<'s, S>, Dispatch<M>) {
    let location = Location::caller();
    let store = cx.store;
    let id = cx.scope_id().with(location_key(location));

    // The queue lives in a slot of its own so that the state slot holds a bare
    // `S`, which is what `State` and `update_later` expect to downcast to.
    let queue_slot = store.slot(id.with("__egui_reactor_reducer_queue"), location, || {
        Box::new(Arc::new(Mutex::new(Vec::<M>::new()))) as Box<dyn Any>
    });
    let queue: Arc<Mutex<Vec<M>>> = Arc::clone(&queue_slot.borrow::<Arc<Mutex<Vec<M>>>>());

    let slot = store.slot(id, location, || Box::new(init()) as Box<dyn Any>);
    let mut state = State::new(store, slot, location);

    // Draining here is also what makes a second pass a no-op: the queue is
    // already empty when the hook is visited again in the same frame.
    let pending: Vec<M> = std::mem::take(&mut *lock(&queue));
    for msg in pending {
        reducer(&mut state, msg);
    }

    let dispatch = Dispatch::new(queue, store.ctx().clone());
    (state, dispatch)
}
