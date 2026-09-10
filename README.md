# egui-reactor

egui-reactor is a Rust library for writing [egui](https://github.com/emilk/egui) applications the way you write React: a JSX-like `rsx!` macro, function components with `#[component]`, and hooks such as `use_state` and `use_effect`. Because egui is immediate mode there is no retained tree and no reconciler, so event handlers run where they are written and can borrow local state with `&mut` — none of the `'static` closures, `Rc<RefCell<_>>` or `.clone()` ceremony that retained-mode Rust UI frameworks require. Flexbox and Grid layout are first-class: `<View>` is a node in a small layout engine of our own, written over [taffy](https://github.com/DioxusLabs/taffy).

The documentation — a guide, a reference and every example running in the browser next to its source — is at [fand.github.io/egui-react](https://fand.github.io/egui-reactor/).

The current design is in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md); design decisions are in [docs/adr/](docs/adr/).

## Usage

```rust
use egui_reactor::prelude::*;
use egui_reactor_app::{Options, run};
use egui_reactor_elements::prelude::*;

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

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-reactor: counter"),
            ..Default::default()
        },
        // Hooks belong in components, so that a view can borrow their guards;
        // the root closure's `Cx` is normally unused.
        |_cx| rsx! { <App/> },
    )
}
```

That is the `counter` example: the component is [`examples/counter/src/lib.rs`](examples/counter/src/lib.rs) and the `run(..)` call is its [`src/main.rs`](examples/counter/src/main.rs). Each example is a library so that the gallery can embed it and tests can drive it, with a thin binary on top.

Work that spans frames is one `use_future(cx, deps, || async { .. })` returning a `&Poll<T>`; a child waits with `let Poll::Ready(x) = .. else { return };` and the nearest `<Suspense fallback={..}>` draws its fallback until everything below it is ready.

The hooks, the elements and the layout attributes are listed in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) sections 4 and 6.

## Examples

Every example runs in the browser on its own page in the [documentation site](https://fand.github.io/egui-reactor/), next to its source. Start with `notes`, a small notes app that uses most of the library at once; the rest take one idea each. Some of them also have a version written with plain egui, so you can switch between the two and compare. `cargo run -p gallery` is the native three-column tool — the example list, the running example and its code in one window — and the site is the web version of the same thing, one page per example. Either way the example runs on a canvas, so a screen reader reads it natively but not on the web; the widget names are there in both (`docs/tasks/a11y/`).

![The gallery: example list, the running example, and its source next to it](docs/gallery.png)

| name | what | live | source | plain egui |
|---|---|---|---|---|
| `notes` | A notes app: reducer, persistence, context, memo and an editor, together. | [notes](https://fand.github.io/egui-reactor/examples/notes) | [lib.rs](examples/notes/src/lib.rs) | – |
| `board` | Cards that keep the title being typed into them while they are dragged between columns. | [board](https://fand.github.io/egui-reactor/examples/board) | [lib.rs](examples/board/src/lib.rs) | [plain.rs](examples/board/src/plain.rs) |
| `patch` | A node editor that generates, validates and previews its own WGSL shader. | [patch](https://fand.github.io/egui-reactor/examples/patch) | [lib.rs](examples/patch/src/lib.rs) | – |
| `spreadsheet` | Formulas over 26 × 10,000 cells: two memo stages, and a draft that survives scrolling out of view. | [spreadsheet](https://fand.github.io/egui-reactor/examples/spreadsheet) | [lib.rs](examples/spreadsheet/src/lib.rs) | – |
| `counter` | One piece of state, three handlers that borrow it in turn. | [counter](https://fand.github.io/egui-reactor/examples/counter) | [lib.rs](examples/counter/src/lib.rs) | – |
| `todo` | A reducer drives the list; `use_persisted` keeps it across restarts. | [todo](https://fand.github.io/egui-reactor/examples/todo) | [lib.rs](examples/todo/src/lib.rs) | [plain.rs](examples/todo/src/plain.rs) |
| `form` | Every bound widget, a change log, and settings that survive a restart. | [form](https://fand.github.io/egui-reactor/examples/form) | [lib.rs](examples/form/src/lib.rs) | [plain.rs](examples/form/src/plain.rs) |
| `theme` | Two values provided at the top and read three levels down, with nothing in between. | [theme](https://fand.github.io/egui-reactor/examples/theme) | [lib.rs](examples/theme/src/lib.rs) | – |
| `clock` | A stopwatch that asks for its own repaints, and an effect that cleans up after itself. | [clock](https://fand.github.io/egui-reactor/examples/clock) | [lib.rs](examples/clock/src/lib.rs) | – |
| `custom-hook` | Three hooks of your own, each called from two components that keep their own state. | [custom-hook](https://fand.github.io/egui-reactor/examples/custom-hook) | [lib.rs](examples/custom-hook/src/lib.rs) | – |
| `escape-hatch` | Four ways down to plain egui: a closure, a leaf, a painter, and a nested Cx. | [escape-hatch](https://fand.github.io/egui-reactor/examples/escape-hatch) | [lib.rs](examples/escape-hatch/src/lib.rs) | – |
| `shader` | A wgpu fragment shader in a `<Canvas>`, with a slider wired to its uniform. | [shader](https://fand.github.io/egui-reactor/examples/shader) | [lib.rs](examples/shader/src/lib.rs) | – |
| `list-10k` | Ten thousand rows: what drawing all of them costs, and what `<VirtualList>` saves. | [list-10k](https://fand.github.io/egui-reactor/examples/list-10k) | [lib.rs](examples/list-10k/src/lib.rs) | [plain.rs](examples/list-10k/src/plain.rs) |
| `layout` | Every flex and grid attribute `<View>` understands, one section each. | [layout](https://fand.github.io/egui-reactor/examples/layout) | [lib.rs](examples/layout/src/lib.rs) | [plain.rs](examples/layout/src/plain.rs) |
| `styles` | Every `style` attribute in a table: the name, the code that uses it, and what it draws. | [styles](https://fand.github.io/egui-reactor/examples/styles) | [lib.rs](examples/styles/src/lib.rs) | – |
| `fetch` | `use_future` runs the request; the nearest `<Suspense>` draws the spinner. | [fetch](https://fand.github.io/egui-reactor/examples/fetch) | [lib.rs](examples/fetch/src/lib.rs) | – |
| `font` | CSS-style font chains: a bundled subset, a 4.5 MB web font fetched on demand, and the installed fonts, with what each entry resolved to. | [font](https://fand.github.io/egui-reactor/examples/font) | [lib.rs](examples/font/src/lib.rs) | – |

Run one natively, or in a browser with [trunk](https://trunkrs.dev/):

```sh
cargo run -p counter
cargo run -p todo --bin todo-plain          # the plain egui version
trunk serve --config examples/counter/Trunk.toml
```

The gallery runs the same way, and takes the name of the example to open first:

```sh
cargo run -p gallery todo
trunk serve --config examples/gallery/Trunk.toml
```

A font an app bundles with `include_bytes!` goes into the wasm (`examples/font` ships a 433 KB subset of Noto Sans JP for that reason and its `fonts/README.md` says how it was cut); the full 4.5 MB font the same example fetches at run time is not committed, a pre-build hook in its `Trunk.toml` (and the gallery's) downloads it the first time trunk builds. egui's own four fonts (Ubuntu-Light, Hack and two emoji fonts, 1.4 MB) are behind `egui-reactor-app`'s `default_fonts` feature, on by default; take the crate with `default-features = false` to leave them out, and then hand `Fonts` a bundled face before the first frame, because a chain with nothing loaded draws no glyphs at all.

## Testing

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace --target wasm32-unknown-unknown
```

Pixel snapshot tests live behind a cargo feature because they need a GPU, and the committed images were rendered on macOS, so they will not match another platform's renderer. They do not run in CI, so regenerate them by hand after any change that alters what they draw.

```sh
cargo test -p egui-reactor-elements --features snapshot
# Each example drawn twice, egui-reactor and plain egui, compared with one image:
# if both render the same picture, the only difference is the code.
cargo test -p gallery --features snapshot
```

After an intentional visual change, regenerate on the platform the images came from:

```sh
UPDATE_SNAPSHOTS=1 cargo test -p egui-reactor-elements --features snapshot
# The gallery's pairs share one file, so write it from the egui-reactor side
# first and then let the plain egui side check itself against it.
UPDATE_SNAPSHOTS=1 cargo test -p gallery --features snapshot egui_reactor
cargo test -p gallery --features snapshot
```

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
