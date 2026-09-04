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

/// The event enum `#[component]` will generate from the `#[event]` arguments of
/// `Dialog`. `Ok` / `Cancel` carry no payload; `Rename` carries one.
pub enum DialogEvent {
    Ok(()),
    Cancel(()),
    Rename(String),
}

/// The props struct `#[component]` will generate for `Dialog`.
pub struct DialogProps<'e> {
    pub title: &'e str,
    pub events: &'e mut dyn FnMut(DialogEvent),
}

/// The hand-written expansion of `<Dialog title={..} on_ok={..} on_cancel={..}
/// on_rename={..} />`.
///
/// Three emitters over one sink are alive at the same time, which is the point:
/// each `#[event]` prop becomes an `Emitter` borrowing the same fused closure.
pub fn dialog(cx: &mut Cx<'_, '_>, props: DialogProps<'_>) {
    let sink: EventSink<'_, DialogEvent> = EventSink::new(props.events);
    let on_ok = Emitter::new(&sink);
    let on_cancel = Emitter::new(&sink);
    let on_rename = Emitter::new(&sink);

    cx.ui.label(props.title);
    if cx.ui.button("OK").clicked() {
        on_ok.emit(DialogEvent::Ok(()));
    }
    if cx.ui.button("Cancel").clicked() {
        on_cancel.emit(DialogEvent::Cancel(()));
    }
    if cx.ui.button("Rename").clicked() {
        on_rename.emit(DialogEvent::Rename(String::from("Renamed?")));
    }
}

/// The hand-written expansion of a `#[hook]`-annotated custom hook.
///
/// `#[hook]` adds `#[track_caller]` and wraps the body in `hook_scope`, so the
/// hook ids inside are keyed by *where the custom hook was called*, not by the
/// single line inside it.
#[track_caller]
pub fn use_counter<'s>(cx: &mut Cx<'s, '_>) -> State<'s, i32> {
    let location = std::panic::Location::caller();
    cx.hook_scope(location, |cx| use_state(cx, || 0))
}

/// The same custom hook *without* `#[hook]`: every call site shares one id.
#[allow(clippy::let_and_return)]
pub fn use_counter_unscoped<'s>(cx: &mut Cx<'s, '_>) -> State<'s, i32> {
    use_state(cx, || 0)
}
