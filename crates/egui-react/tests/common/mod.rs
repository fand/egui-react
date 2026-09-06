//! The components the spike tests drive, now written with `#[component]`,
//! `#[hook]` and `rsx!`, plus a tiny runner that plays the role of
//! `egui-react-app`.
#![allow(dead_code)]

use egui_react::prelude::*;

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

/// A counter with a decrement and an increment button.
///
/// The two `on_click` handlers are the point: `rsx!` fuses them into one
/// closure, and each borrows `count` mutably in turn.
#[component]
pub fn Counter(cx: &mut Cx, initial: i32) {
    let mut count = use_state(cx, || initial);
    let (store, scope) = (cx.store, cx.scope_id());
    cx.ui().horizontal(|ui| {
        let mut cx = Cx::new(store, ui, scope);
        if cx.ui().button("-").clicked() {
            *count -= 1;
        }
        cx.ui().label(format!("count: {}", *count));
        if cx.ui().button("+").clicked() {
            *count += 1;
        }
    });
}

/// Same as [`Counter`], but with unique widget labels so several can coexist.
#[component]
pub fn NamedCounter(cx: &mut Cx, name: &str, initial: i32) {
    let mut count = use_state(cx, || initial);
    let (store, scope) = (cx.store, cx.scope_id());
    cx.ui().horizontal(|ui| {
        let mut cx = Cx::new(store, ui, scope);
        if cx.ui().button(format!("{name} -")).clicked() {
            *count -= 1;
        }
        cx.ui().label(format!("{name}: {}", *count));
        if cx.ui().button(format!("{name} +")).clicked() {
            *count += 1;
        }
    });
}

/// A dialog with three callback props: two nullary and one carrying a payload.
///
/// `#[component]` turns the `#[event]` arguments into `DialogEvent` and gives
/// the body three `Emitter`s over one shared sink.
#[component]
pub fn Dialog(
    cx: &mut Cx,
    title: &str,
    #[event] on_ok: (),
    #[event] on_cancel: (),
    #[event] on_rename: String,
) {
    cx.ui().label(title);
    if cx.ui().button("OK").clicked() {
        on_ok.emit(());
    }
    if cx.ui().button("Cancel").clicked() {
        on_cancel.emit(());
    }
    if cx.ui().button("Rename").clicked() {
        on_rename.emit(String::from("Renamed?"));
    }
}

/// `<Counter/>` as a plain function, for tests that predate `rsx!`.
pub fn counter(cx: &mut Cx<'_, '_>, initial: i32) {
    rsx! { <Counter initial={initial}/> }.show(cx);
}

/// `<NamedCounter/>` as a plain function.
pub fn named_counter(cx: &mut Cx<'_, '_>, name: &str, initial: i32) {
    rsx! { <NamedCounter name={name} initial={initial}/> }.show(cx);
}

/// A custom hook: `#[hook]` keys the state by *where this is called*.
#[hook]
pub fn use_counter<'s>(cx: &mut Cx<'s, '_>) -> State<'s, i32> {
    use_state(cx, || 0)
}

/// The same custom hook *without* `#[hook]`: every call site shares one id.
#[allow(clippy::let_and_return)]
pub fn use_counter_unscoped<'s>(cx: &mut Cx<'s, '_>) -> State<'s, i32> {
    use_state(cx, || 0)
}
