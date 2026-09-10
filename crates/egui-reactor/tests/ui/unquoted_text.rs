use egui_reactor::prelude::*;

#[component]
fn Text(cx: &mut Cx, children: impl Into<String>) {
    cx.ui().label(children.into());
}

fn app(cx: &mut Cx<'_, '_>) {
    rsx! { <Text>hello</Text> }.show(cx);
}

fn main() {
    let _ = app;
}
