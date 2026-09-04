//! Hand-written versions of what `rsx!` / `#[component]` will generate, plus a
//! tiny runner that plays the role of `react-egui-app`.
#![allow(dead_code)]
// `rsx!` will emit event handlers as closures that are created and called on
// the spot; that is the whole point of test 1, so the lint is off here. The
// macro will have to emit this `allow` in generated code as well.
#![allow(clippy::redundant_closure_call)]

use react_egui::prelude::*;

/// One pass: `begin_pass` -> draw -> (all guards dropped) -> `end_pass`.
pub fn run_app(ui: &mut egui::Ui, store: &mut Store, app: impl FnOnce(&mut Cx<'_, '_>)) {
    store.begin_pass(ui.ctx());
    {
        let store: &Store = store;
        let mut cx = Cx::new(store, ui, egui::Id::new("root"));
        app(&mut cx);
    }
    // Every guard died with the component bodies above, so the sweep is safe.
    store.end_pass();
}

/// The hand-written expansion of `<Counter initial={..} />`.
///
/// The two `(|| ..)()` calls are the shape `rsx!` will emit for `onclick`
/// handlers: closures created and consumed in place, each borrowing `count`
/// mutably in turn.
pub fn counter(cx: &mut Cx<'_, '_>, initial: i32) {
    let mut count = use_state(cx, || initial);
    let (store, scope) = (cx.store, cx.scope_id());
    cx.ui.horizontal(|ui| {
        let cx = Cx::new(store, ui, scope);
        if cx.ui.button("-").clicked() {
            (|| *count -= 1)();
        }
        cx.ui.label(format!("count: {}", *count));
        if cx.ui.button("+").clicked() {
            (|| *count += 1)();
        }
    });
}

/// Same as [`counter`], but with unique widget labels so several can coexist.
pub fn named_counter(cx: &mut Cx<'_, '_>, name: &str, initial: i32) {
    let mut count = use_state(cx, || initial);
    let (store, scope) = (cx.store, cx.scope_id());
    cx.ui.horizontal(|ui| {
        let cx = Cx::new(store, ui, scope);
        if cx.ui.button(format!("{name} -")).clicked() {
            (|| *count -= 1)();
        }
        cx.ui.label(format!("{name}: {}", *count));
        if cx.ui.button(format!("{name} +")).clicked() {
            (|| *count += 1)();
        }
    });
}
