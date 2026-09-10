use egui_reactor::prelude::*;

#[component]
fn Row(cx: &mut Cx, label: &str, #[event] on_remove: ()) {
    if cx.ui().button(label).clicked() {
        on_remove.emit(());
    }
}

fn app(cx: &mut Cx<'_, '_>) {
    let mut todos = use_state(cx, || vec![String::from("a"), String::from("b")]);
    // The loop borrows `todos`, so the handler cannot remove from it: E0502.
    // Use `todos.update_later(move |t| { t.remove(i); })` instead.
    rsx! {
        for (i, todo) in todos.iter().enumerate() {
            <Row key={i} label={todo} on_remove={|| { todos.remove(i); }}/>
        }
    }
    .show(cx);
}

fn main() {
    let _ = app;
}
