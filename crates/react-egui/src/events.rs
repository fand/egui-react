//! Callback props: the fused event closure, [`Handler`] and [`Emitter`].
//!
//! `rsx!` turns `<Dialog on_ok={..} on_cancel={..} />` into a single `FnMut(E)`
//! closure that matches on a generated event enum. Each arm creates its handler
//! closure and calls it immediately through [`Handler::call`], so the macro does
//! not need to know whether the user wrote `|| ..` or `|payload| ..`.

use std::cell::RefCell;

/// Marker: the handler ignores the payload (`|| ..`).
pub struct Arity0;

/// Marker: the handler takes the payload (`|payload| ..`).
pub struct Arity1;

/// A callback prop that may or may not take the event payload.
///
/// `Marker` is `(Arity0 | Arity1, ReturnType)`. It exists only to keep the
/// zero-arg and one-arg impls from overlapping and to let the handler body
/// evaluate to something other than `()`; it is inferred from the closure at
/// every call site, so the macro can always emit a bare
/// `Handler::call(closure, payload)`.
pub trait Handler<A, Marker> {
    /// Call the handler with the payload, dropping it if the handler is nullary.
    fn call(self, payload: A);
}

impl<F: FnOnce() -> R, A, R> Handler<A, (Arity0, R)> for F {
    fn call(self, _payload: A) {
        self();
    }
}

impl<F: FnOnce(A) -> R, A, R> Handler<A, (Arity1, R)> for F {
    fn call(self, payload: A) {
        self(payload);
    }
}

/// The fused event closure a parent hands to a child, shared by every emitter.
pub type EventSink<'e, E> = RefCell<&'e mut (dyn FnMut(E) + 'e)>;

/// A child-side handle that fires one event kind.
///
/// Several emitters over the same sink can be alive at once, which is what
/// makes `#[event] on_ok` / `#[event] on_cancel` usable side by side.
pub struct Emitter<'a, 'e, E> {
    sink: &'a EventSink<'e, E>,
}

impl<E> Clone for Emitter<'_, '_, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<E> Copy for Emitter<'_, '_, E> {}

impl<'a, 'e, E> Emitter<'a, 'e, E> {
    /// Build an emitter over a shared event sink.
    pub fn new(sink: &'a EventSink<'e, E>) -> Self {
        Self { sink }
    }

    /// Fire one event into the parent's fused closure.
    ///
    /// # Panics
    /// Panics if called re-entrantly (an event fired from inside a handler for
    /// the same child). Direct expansion never does this.
    pub fn emit(&self, event: E) {
        let mut sink = self.sink.try_borrow_mut().expect(
            "react-egui: event emitted re-entrantly; a handler fired another \
             event on the same component while it was still running",
        );
        (*sink)(event);
    }
}
