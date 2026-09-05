//! The smallest react-egui app: one piece of state and two handlers.

use react_egui::prelude::*;
use react_egui_app::{Options, run};
use react_egui_elements::prelude::*;

#[component]
fn App(cx: &mut Cx) {
    let mut count = use_state(cx, || 0i32);

    rsx! {
        <View direction="column" align="center" justify="center" gap={12} grow={1.0}>
            <Text size={32.0} strong>{format!("{}", *count)}</Text>
            <View direction="row" gap={8}>
                <Button on_click={|| *count -= 1}>"-"</Button>
                <Button on_click={|| *count = 0}>"reset"</Button>
                <Button on_click={|| *count += 1}>"+"</Button>
            </View>
        </View>
    }
}

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("react-egui: counter"),
            ..Default::default()
        },
        // The root closure gets a `Cx` it rarely needs: hooks belong in
        // components, so that a view can borrow their guards.
        |_cx| rsx! { <App/> },
    )
}
