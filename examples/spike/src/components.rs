//! The same components the core tests drive, written with the macros.

use react_egui::prelude::*;

/// A counter with a decrement and an increment button.
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

/// A dialog with three callback props: two nullary and one with a payload.
#[component]
pub fn Dialog(
    cx: &mut Cx,
    title: &str,
    #[event] on_ok: (),
    #[event] on_cancel: (),
    #[event] on_rename: String,
) {
    cx.ui().label(title);
    let (store, scope) = (cx.store, cx.scope_id());
    cx.ui().horizontal(|ui| {
        let mut cx = Cx::new(store, ui, scope);
        if cx.ui().button("OK").clicked() {
            on_ok.emit(());
        }
        if cx.ui().button("Cancel").clicked() {
            on_cancel.emit(());
        }
        if cx.ui().button("Rename").clicked() {
            on_rename.emit(String::from("Renamed?"));
        }
    });
}

/// The root of the example.
#[component]
pub fn App(cx: &mut Cx) {
    cx.ui().heading("react-egui spike");
    cx.ui().separator();

    let mut open = use_state(cx, || true);
    let mut title = use_state(cx, || String::from("Quit?"));

    if !*open && cx.ui().button("Reopen").clicked() {
        *open = true;
    }

    // The `title` prop borrows `title` while the fused closure needs it
    // mutably, so the value is copied out first (ARCHITECTURE.md 3.7).
    let title_text = (*title).clone();
    rsx! {
        <Counter initial={0}/>
        if *open {
            <Dialog
                title={&title_text}
                on_ok={|| *open = false}
                on_cancel={|| *open = false}
                on_rename={|name: String| *title = name}
            />
        }
    }
}
