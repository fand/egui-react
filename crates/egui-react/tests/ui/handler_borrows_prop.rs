use egui_react::prelude::*;

#[component]
fn Dialog(cx: &mut Cx, title: &str, #[event] on_rename: String) {
    cx.ui().label(title);
    if cx.ui().button("Rename").clicked() {
        on_rename.emit(String::from("new"));
    }
}

fn app(cx: &mut Cx<'_, '_>) {
    let mut title = use_state(cx, || String::from("old"));
    // The prop borrows `title` immutably while the fused closure needs it
    // mutably: E0502. Clone the value or use `update_later`.
    rsx! {
        <Dialog title={&*title} on_rename={|name: String| *title = name}/>
    }
    .show(cx);
}

fn main() {
    let _ = app;
}
