use egui_react::prelude::*;

#[component]
fn Field(cx: &mut Cx, label: &str) {
    cx.ui().label(label);
}

fn app(cx: &mut Cx<'_, '_>) {
    rsx! { <Field/> }.show(cx);
}

fn main() {
    let _ = app;
}
