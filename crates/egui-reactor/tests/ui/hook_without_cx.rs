use egui_reactor::prelude::*;

#[hook]
fn use_nothing() -> i32 {
    1
}

fn main() {
    let _ = use_nothing;
}
