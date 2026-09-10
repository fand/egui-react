use egui_reactor::prelude::*;

#[component]
fn Plain(cx: &mut Cx) {
    cx.ui().label("plain");
}

fn app(cx: &mut Cx<'_, '_>) {
    rsx! { <Plain on_click={|| {}}/> }.show(cx);
}

fn main() {
    let _ = app;
}
