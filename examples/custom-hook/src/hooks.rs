//! Three hooks of our own.
//!
//! A custom hook is an ordinary function that takes `&mut Cx` and calls other
//! hooks. `#[hook]` is what makes it reusable: it enters a scope keyed by the
//! *call site*, so two calls — in one component or in two — get their own
//! state. Without it every call would share one slot, and the second call in a
//! pass would be reported as a collision.
//!
//! Nothing here is special to this example. These are the hooks an app would
//! grow on its own, and they compose exactly like the built-in ones.

use std::time::Duration;

use egui_reactor::prelude::*;

/// `value` as it was once it stopped changing for `delay` seconds.
///
/// The usual reason: a search box that should not fire a request per keystroke.
/// While a change is still settling, nothing else would ask for the frame that
/// settles it, so the hook asks for it itself.
#[hook]
pub fn use_debounce(cx: &mut Cx, value: &str, delay: f64) -> String {
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

/// The value from the last time this hook saw a different one.
///
/// `None` until `value` has changed at least once. React's `usePrevious`, and
/// the same one-line body.
#[hook]
pub fn use_previous<T: Clone + PartialEq + 'static>(cx: &mut Cx, value: T) -> Option<T> {
    let mut seen = use_state(cx, || (value.clone(), None::<T>));
    if seen.0 != value {
        let last = std::mem::replace(&mut seen.0, value);
        seen.1 = Some(last);
    }
    seen.1.clone()
}

/// The size of the window, for layout that reacts to it.
///
/// No state: a hook is any function that reads from `Cx`, and this one only
/// needs the context. `#[hook]` costs nothing here and keeps it in the family.
#[hook]
pub fn use_window_size(cx: &mut Cx) -> egui::Vec2 {
    cx.ctx().viewport_rect().size()
}
