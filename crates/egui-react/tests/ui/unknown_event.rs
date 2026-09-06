use egui_react::prelude::*;

#[component]
fn Dialog(cx: &mut Cx, #[event] on_ok: ()) {
    if cx.ui().button("OK").clicked() {
        on_ok.emit(());
    }
}

fn app(cx: &mut Cx<'_, '_>) {
    rsx! { <Dialog on_foo={|| {}}/> }.show(cx);
}

fn main() {
    let _ = app;
}
