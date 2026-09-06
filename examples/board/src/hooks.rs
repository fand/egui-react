//! The hooks this example needed and the library does not have: undo, a
//! debounce, a drag session, and identity-keyed state.
//!
//! None of them is special. Each is an ordinary function that takes `&mut Cx`
//! and calls the built-in hooks, marked `#[hook]` so its slots are keyed by the
//! call site and two callers never share one. That is the claim being made
//! here: an undo stack is not a library feature waiting to be written, it is
//! twenty lines wrapped around `use_reducer`, and it composes with everything
//! else because it *is* everything else.
//!
//! `use_identity` is the exception, and the one to read first: it is about
//! breaking the call-site rule on purpose, so that a card's state can follow
//! the card instead of the place it is drawn.

use std::cell::RefCell;
use std::fmt::Debug;
use std::hash::Hash;
use std::rc::Rc;
use std::time::Duration;

use egui_react::prelude::*;

/// One `use_state` whose slot is keyed by `key` rather than by where the
/// component sits in the tree.
///
/// This is the hook the example turns on, so it is worth being precise about
/// what it fixes. An ordinary hook's slot id is the *scope chain* — every
/// element between the root and the component — plus the call site, and `key=`
/// only tells siblings under one parent apart. A card is drawn inside its
/// column, so `<Card key={id}/>` in "doing" and the same card in "review" are
/// two different slots: drag the card across and the draft it was carrying
/// would stay behind in the column it came from. Keying on the card's own id
/// instead makes the state belong to the card rather than to the place it is
/// drawn, which is what the eye expects when it watches a card move.
///
/// `Cx::new` takes the scope id to root at, so this is writable without
/// touching the library (see plan.md section 8, which argues it should not have
/// to be). The `Cx` built here is used for nothing but calling `use_state`; the
/// guard it hands back borrows the *store*, not the `Cx`, so it outlives the
/// borrow taken on this line and the caller carries on drawing as usual.
///
/// Two things are unchanged, and both matter. The pass-end sweep still drops
/// the slot when the card stops being drawn — that sweep is precisely the
/// `HashMap` housekeeping `plain.rs` has to write out by hand. And an id used
/// twice in one pass is still reported as a collision, which is why the key
/// says *which* piece of state as well as whose: `(card.id, "draft")`.
#[hook]
pub fn use_identity<'s, T: 'static>(
    cx: &mut Cx<'s, '_>,
    // `Hash + Debug` is what an egui id salt is, and what `key=` asks for too.
    key: impl Hash + Debug,
    init: impl FnOnce() -> T,
) -> State<'s, T> {
    let store = cx.store;
    let scope = egui::Id::new(("board/identity", key));
    let mut rooted = Cx::new(store, cx.ui(), scope);
    use_state(&mut rooted, init)
}

/// A message on its way through an undo history: do it, or walk the history.
pub enum Undoable<M> {
    Do(M),
    Undo,
    Redo,
}

/// A value and the way back to the values it used to be.
pub struct History<S> {
    pub present: S,
    past: Vec<S>,
    future: Vec<S>,
}

impl<S> History<S> {
    pub fn new(present: S) -> Self {
        Self {
            present,
            past: Vec::new(),
            future: Vec::new(),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
}

/// How many steps back the history keeps. Every step is a whole clone of the
/// state, so this is a memory budget, not a policy.
const DEPTH: usize = 64;

/// [`use_reducer`] with an undo history wrapped around it.
///
/// The reducer underneath knows nothing about any of this: it is still
/// `fn(&mut S, M)`, and it is the *board's* reducer, tested on its own. What
/// this hook adds is the two stacks and the rule that a message which changed
/// nothing does not become a step you have to press undo twice to get past.
///
/// Undo is deliberately not a library feature. `S: Clone` and one message per
/// user action are the whole design, and both are decisions an application
/// makes about its own state — which is the argument for keeping this here,
/// in twenty lines, rather than in the core.
#[hook]
pub fn use_undoable<'s, S: Clone + PartialEq + 'static, M: Send + 'static>(
    cx: &mut Cx<'s, '_>,
    reduce: impl Fn(&mut S, M),
    init: impl FnOnce() -> S,
) -> (State<'s, History<S>>, Dispatch<Undoable<M>>) {
    use_reducer(
        cx,
        move |history: &mut History<S>, msg: Undoable<M>| match msg {
            Undoable::Do(msg) => {
                let before = history.present.clone();
                reduce(&mut history.present, msg);
                if before == history.present {
                    return;
                }
                history.past.push(before);
                if history.past.len() > DEPTH {
                    history.past.remove(0);
                }
                // Doing something new is what makes the future unreachable.
                history.future.clear();
            }
            Undoable::Undo => {
                if let Some(previous) = history.past.pop() {
                    let present = std::mem::replace(&mut history.present, previous);
                    history.future.push(present);
                }
            }
            Undoable::Redo => {
                if let Some(next) = history.future.pop() {
                    let present = std::mem::replace(&mut history.present, next);
                    history.past.push(present);
                }
            }
        },
        || History::new(init()),
    )
}

/// `value` as it was once it stopped changing for `delay` seconds.
///
/// The search box types into a `String` on every keystroke; the filtering reads
/// this instead, so a memo is not rebuilt per letter. Time comes from
/// `ui.input(|i| i.time)` rather than `std::time::Instant`, which panics in a
/// browser, and the hook asks for the frame that will settle it — nothing else
/// would.
///
/// The `custom-hook` example builds this same hook to show what `#[hook]` is
/// for. Here it is doing a job.
#[hook]
pub fn use_debounced(cx: &mut Cx, value: &str, delay: f64) -> String {
    let mut latest = use_state(cx, || value.to_owned());
    let mut changed_at = use_state(cx, || f64::NEG_INFINITY);
    let mut settled = use_state(cx, || value.to_owned());

    let now = cx.ui().input(|i| i.time);
    if latest.as_str() != value {
        *latest = value.to_owned();
        *changed_at = now;
    }

    if settled.as_str() != latest.as_str() {
        let waited = now - *changed_at;
        if waited >= delay {
            *settled = latest.clone();
        } else {
            cx.ctx()
                .request_repaint_after(Duration::from_secs_f64(delay - waited));
        }
    }

    settled.clone()
}

/// What is being dragged, and the places it could be dropped.
///
/// Cheap to clone: everything is behind one `Rc`, so the copy a card reads out
/// of the context and the copy the board keeps are the same session.
///
/// The interior mutability is not decoration. Registering a drop slot happens
/// on every frame of a drag and must *not* mark any state dirty, because a
/// component that writes state on every pass asks for a repaint on every pass
/// and the app never goes idle (ARCHITECTURE 5.6). `Handle` has `set` and
/// `update`, both of which request a repaint, and no non-dirtying write; so the
/// per-frame scribbling happens inside the value, through `Handle::with`. See
/// plan.md section 8.
pub struct Dnd<P, T> {
    session: Rc<RefCell<Session<P, T>>>,
}

struct Session<P, T> {
    /// What the pointer picked up, until it is dropped or the drag is cancelled.
    carrying: Option<P>,
    /// The drop slots offered during the current frame.
    slots: Vec<(egui::Rect, T)>,
    /// The slot under the pointer, from the slots offered on the frame before.
    hovered: Option<T>,
    pointer: Option<egui::Pos2>,
    /// Set on the frame the pointer let go over a slot; taken by the board.
    dropped: Option<(P, T)>,
}

impl<P, T> Clone for Dnd<P, T> {
    fn clone(&self) -> Self {
        Self {
            session: Rc::clone(&self.session),
        }
    }
}

impl<P: Clone, T: Clone> Dnd<P, T> {
    /// A session with nothing in it. `use_dnd` makes one; a component drawn
    /// outside a provider falls back to one so that its handlers still compile.
    pub fn new() -> Self {
        Self {
            session: Rc::new(RefCell::new(Session {
                carrying: None,
                slots: Vec::new(),
                hovered: None,
                pointer: None,
                dropped: None,
            })),
        }
    }

    /// Take `payload` up. Called from whatever sensed the drag.
    pub fn pick_up(&self, payload: P) {
        self.session.borrow_mut().carrying = Some(payload);
    }

    /// What the pointer is carrying, if anything.
    pub fn carrying(&self) -> Option<P> {
        self.session.borrow().carrying.clone()
    }

    /// Where the pointer is, for drawing the thing it carries.
    pub fn pointer(&self) -> Option<egui::Pos2> {
        self.session.borrow().pointer
    }

    /// Offer `rect` as a place to drop what is being carried.
    ///
    /// Only worth doing during a drag; outside one there is nothing to place.
    pub fn slot(&self, rect: egui::Rect, target: T) {
        let mut session = self.session.borrow_mut();
        if session.carrying.is_some() {
            session.slots.push((rect, target));
        }
    }

    /// The slot the pointer is over, as of the slots offered on the previous
    /// frame — a card asks this to draw the line showing where it would land.
    ///
    /// One frame behind, because the answer has to be known before the cards
    /// draw and the cards are what offer the slots. Nothing has moved in
    /// between: a pointer that moved asked for this frame in the first place.
    pub fn hovered(&self) -> Option<T> {
        let session = self.session.borrow();
        session.carrying.as_ref()?;
        session.hovered.clone()
    }

    /// Take the drop that just happened, if one did. Called once per frame by
    /// whoever turns it into a message.
    pub fn take_drop(&self) -> Option<(P, T)> {
        self.session.borrow_mut().dropped.take()
    }

    /// Start of a frame: read the pointer, resolve a release against the slots
    /// the previous frame offered, and clear the slots for this one.
    fn begin_frame(&self, pointer: Option<egui::Pos2>, released: bool, cancelled: bool) {
        let mut session = self.session.borrow_mut();
        session.pointer = pointer;
        let hovered = pointer.and_then(|pointer| {
            session
                .slots
                .iter()
                .find(|(rect, _)| rect.contains(pointer))
                .map(|(_, target)| target.clone())
        });
        session.hovered = hovered;
        session.slots.clear();

        if cancelled {
            session.carrying = None;
        }
        if released {
            let landed = session.hovered.clone();
            // A release with nothing under it just ends the drag.
            session.dropped = session.carrying.take().zip(landed);
        }
    }
}

impl<P: Clone, T: Clone> Default for Dnd<P, T> {
    fn default() -> Self {
        Self::new()
    }
}

/// The drag session for the whole tree: one place that knows what is being
/// carried and where it could go.
///
/// Returns a `Handle` because that is what `provide_context` takes, and a drag
/// crosses the tree — the card that is picked up and the column it lands in are
/// in different branches. `handle.get()` hands out a clone of the session, not
/// a copy of the state.
#[hook]
pub fn use_dnd<'s, P: Clone + 'static, T: Clone + 'static>(
    cx: &mut Cx<'s, '_>,
) -> Handle<'s, Dnd<P, T>> {
    let session = use_handle(cx, Dnd::<P, T>::new);
    let (pointer, released, cancelled) = cx.ctx().input(|i| {
        (
            i.pointer.interact_pos(),
            i.pointer.any_released(),
            i.key_pressed(egui::Key::Escape),
        )
    });
    // `with`, not `update`: this runs on every frame and `update` would ask for
    // a repaint on every frame with it.
    session.with(|dnd| dnd.begin_frame(pointer, released, cancelled));
    session
}
