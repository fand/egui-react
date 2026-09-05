# react-egui

react-egui is a Rust library for writing [egui](https://github.com/emilk/egui) applications the way you write React: a JSX-like `rsx!` macro, function components with `#[component]`, and hooks such as `use_state` and `use_effect`. Because egui is immediate mode there is no retained tree and no reconciler, so event handlers run where they are written and can borrow local state with `&mut` — none of the `'static` closures, `Rc<RefCell<_>>` or `.clone()` ceremony that retained-mode Rust UI frameworks require. Flexbox and Grid layout are first-class through [egui_taffy](https://github.com/PPakalns/egui_taffy).

Design decisions are recorded in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Usage

```rust
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
        // Hooks belong in components, so that a view can borrow their guards;
        // the root closure's `Cx` is normally unused.
        |_cx| rsx! { <App/> },
    )
}
```

That is `examples/counter` verbatim. Run it natively with `cargo run -p counter`, or in a browser with [trunk](https://trunkrs.dev/): `trunk serve --config examples/counter/Trunk.toml`. The other examples are `todo` (`use_reducer`, `use_persisted`, `TextEdit` + `Checkbox`, `for` with `key`) and `layout` (a tour of the flex and grid attributes).

The hooks, the elements and the layout attributes are listed in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) sections 4 and 6.

## Testing

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace --target wasm32-unknown-unknown
```

Pixel snapshot tests live behind a cargo feature because they need a GPU, and the committed images were rendered on macOS, so they will not match another platform's renderer. Run them locally with:

```sh
cargo test -p react-egui-elements --features snapshot
# after an intentional visual change, on the platform the images came from:
UPDATE_SNAPSHOTS=1 cargo test -p react-egui-elements --features snapshot
```

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
