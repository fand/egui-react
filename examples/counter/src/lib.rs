//! The smallest react-egui app: one piece of state and two handlers.

use example_meta::Meta;
use react_egui::prelude::*;
use react_egui_elements::prelude::*;

pub const META: Meta = Meta {
    name: "counter",
    summary: "One piece of state, three handlers that borrow it in turn.",
    hooks: &["use_state"],
    elements: &["View", "Text", "Button"],
    source: include_str!("lib.rs"),
    plain: None,
};

#[component]
pub fn App(cx: &mut Cx) {
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
