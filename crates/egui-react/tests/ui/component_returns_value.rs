use egui_react::prelude::*;

#[component]
fn Broken(cx: &mut Cx) -> i32 {
    cx.ui().label("broken");
    1
}

fn main() {
    let _ = Broken;
}
