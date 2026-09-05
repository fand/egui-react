//! The same hand-written expansions the tests use, duplicated here on purpose.
//! `rsx!` / `#[component]` will replace both copies in phase 3.
#![allow(clippy::redundant_closure_call)]

use react_egui::prelude::*;

/// The hand-written expansion of `<Counter initial={..} />`.
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

/// The event enum `#[component]` generates from `Dialog`'s `#[event]` args.
pub enum DialogEvent {
    Ok(()),
    Cancel(()),
    Rename(String),
}

/// The props struct `#[component]` generates for `Dialog`.
pub struct DialogProps<'e> {
    pub title: &'e str,
    pub events: &'e mut dyn FnMut(DialogEvent),
}

/// The hand-written expansion of `<Dialog .. />`, with three emitters sharing
/// one fused event closure.
pub fn dialog(cx: &mut Cx<'_, '_>, props: DialogProps<'_>) {
    let sink: EventSink<'_, DialogEvent> = EventSink::new(props.events);
    let on_ok = Emitter::new(&sink);
    let on_cancel = Emitter::new(&sink);
    let on_rename = Emitter::new(&sink);

    cx.ui.label(props.title);
    let (store, scope) = (cx.store, cx.scope_id());
    cx.ui.horizontal(|ui| {
        let cx = Cx::new(store, ui, scope);
        if cx.ui.button("OK").clicked() {
            on_ok.emit(DialogEvent::Ok(()));
        }
        if cx.ui.button("Cancel").clicked() {
            on_cancel.emit(DialogEvent::Cancel(()));
        }
        if cx.ui.button("Rename").clicked() {
            on_rename.emit(DialogEvent::Rename(String::from("Renamed?")));
        }
    });
}

/// The root of the example.
pub fn app(cx: &mut Cx<'_, '_>) {
    cx.ui.heading("react-egui spike");
    cx.ui.separator();

    counter(cx, 0);
    cx.ui.separator();

    let mut open = use_state(cx, || true);
    let mut title = use_state(cx, || String::from("Quit?"));

    if !*open && cx.ui.button("Reopen").clicked() {
        *open = true;
    }

    if *open {
        // `props.title` borrows `title` while the fused closure needs it
        // mutably, so the value is copied out first.
        let title_text = (*title).clone();
        dialog(
            cx,
            DialogProps {
                title: &title_text,
                events: &mut |ev| match ev {
                    DialogEvent::Ok(a) => Handler::call(|| *open = false, a),
                    DialogEvent::Cancel(a) => Handler::call(|| *open = false, a),
                    DialogEvent::Rename(a) => Handler::call(|name: String| *title = name, a),
                },
            },
        );
    }
}
