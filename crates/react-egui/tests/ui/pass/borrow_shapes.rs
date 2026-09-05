//! The borrow shapes that must keep compiling.

use react_egui::prelude::*;

#[component]
fn Dialog(cx: &mut Cx, title: &str, #[event] on_ok: (), #[event] on_cancel: ()) {
    cx.ui().label(title);
    if cx.ui().button("OK").clicked() {
        on_ok.emit(());
    }
    if cx.ui().button("Cancel").clicked() {
        on_cancel.emit(());
    }
}

#[component]
fn Row(cx: &mut Cx, label: &str, #[event] on_remove: ()) {
    if cx.ui().button(label).clicked() {
        on_remove.emit(());
    }
}

#[component]
fn Frame(cx: &mut Cx, children: impl View) {
    cx.ui().label("frame");
    children.show(cx);
}

#[hook]
fn use_counter<'s>(cx: &mut Cx<'s, '_>) -> State<'s, i32> {
    use_state(cx, || 0)
}

fn app(cx: &mut Cx<'_, '_>) {
    let mut open = use_state(cx, || true);
    let mut count = use_counter(cx);
    let todos = use_state(cx, || vec![String::from("a")]);

    rsx! {
        // Two fused handlers on one element, both taking the same state.
        <Dialog title="Quit?" on_ok={|| *open = false} on_cancel={|| *open = false}/>
        // A loop that reads the state while the handler queues a write.
        for (i, todo) in todos.iter().enumerate() {
            <Row key={i} label={todo} on_remove={|| todos.update_later(move |t| { t.remove(i); })}/>
        }
        // A children closure borrowing a guard.
        <Frame>
            <Dialog title="inner" on_ok={|| *count += 1}/>
        </Frame>
        // The escape hatch, with `cx` inferred.
        {::react_egui::view(|cx| { cx.ui().label("escape"); })}
    }
    .show(cx);
}

fn main() {
    let _ = app;
}
