# egui-reactor-elements

The elements you write inside `rsx!` for [egui-reactor](https://crates.io/crates/egui-reactor): `<View>`, `<Text>`, `<Button>`, `<TextEdit>`, `<VirtualList>`, `<Canvas>` and the rest.

```rust
use egui_reactor::prelude::*;
use egui_reactor_elements::prelude::*;

#[component]
pub fn Hello(cx: &mut Cx) {
    rsx! {
        <View direction="column" gap={8}>
            <Text strong>"Hello"</Text>
            <Button on_click={|| println!("clicked")}>"Click"</Button>
        </View>
    }
}
```

Docs: https://fand.github.io/egui-reactor/reference/elements
