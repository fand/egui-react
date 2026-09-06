//! Hooks of your own, and what `#[hook]` is for.
//!
//! The three hooks live in [`hooks`]; this file is five components that use
//! them. Each hook is called from two different components, and the pairs do
//! not share state: `#[hook]` keys the hook's slots by call site, so
//! `SearchBox` and `Mirror` each get their own debounce.
//!
//! That is the whole feature. A custom hook needs no registration, no trait and
//! no macro beyond `#[hook]`, and it composes with the built-in hooks because
//! it *is* the built-in hooks.

use example_meta::Meta;
use egui_react::prelude::*;
use egui_react_elements::prelude::*;

pub mod hooks;

use hooks::{use_debounce, use_previous, use_window_size};

pub const META: Meta = Meta {
    name: "custom-hook",
    summary: "Three hooks of your own, each called from two components that keep their own state.",
    hooks: &["use_state", "use_effect", "#[hook]"],
    elements: &["View", "Text", "TextEdit", "Button", "Frame", "Separator"],
    source: include_str!("lib.rs"),
    plain: None,
};

/// How long the debounce waits, in seconds.
const DELAY: f64 = 0.5;

/// Below this window width the responsive section stacks instead of spreading.
const NARROW: f32 = 520.0;

#[component]
pub fn App(cx: &mut Cx) {
    rsx! {
        <View direction="column" gap={8} p={12} grow={1.0}>
            <Text size={22.0} strong>"custom hooks"</Text>

            <SearchBox/>
            <Separator/>
            <Mirror/>
            <Separator/>
            <Counter/>
            <Separator/>
            <Responsive/>
        </View>
    }
}

/// `use_debounce` and `use_previous` together: the settled query, and the one
/// before it.
#[component]
fn SearchBox(cx: &mut Cx) {
    let mut query = use_state(cx, String::new);
    let live = query.clone();
    let settled = use_debounce(cx, &live, DELAY);
    let previous = use_previous(cx, settled.clone());

    rsx! {
        <View direction="column" gap={4}>
            <Text strong>"search box"</Text>
            <TextEdit w={220.0} bind={query.bind()} hint="type to search"/>
            <Text>{format!("live: {live}")}</Text>
            <Text>{format!("settled: {settled}")}</Text>
            <Text>{format!("before that: {}", if previous.as_deref().unwrap_or("").is_empty() {
                "(none)"
            } else {
                previous.as_deref().unwrap_or("(none)")
            })}</Text>
        </View>
    }
}

/// The second caller of `use_debounce`. Its state is its own: type in one box
/// and the other's readings do not move.
#[component]
fn Mirror(cx: &mut Cx) {
    let mut text = use_state(cx, || String::from("hello"));
    let live = text.clone();
    let settled = use_debounce(cx, &live, DELAY);

    rsx! {
        <View direction="column" gap={4}>
            <Text strong>"mirror"</Text>
            <TextEdit w={220.0} bind={text.bind()}/>
            <Text>{format!("mirror live: {live}")}</Text>
            <Text>{format!("mirror settled: {settled}")}</Text>
        </View>
    }
}

/// The second caller of `use_previous`.
#[component]
fn Counter(cx: &mut Cx) {
    let mut count = use_state(cx, || 0i32);
    let now = *count;
    let before = use_previous(cx, now);

    rsx! {
        <View direction="row" gap={8} align="center">
            <Button on_click={|| *count += 1}>"+"</Button>
            <Button on_click={|| *count -= 1}>"-"</Button>
            <Text>{format!(
                "count {now}, was {}",
                before.map_or_else(|| String::from("-"), |n| n.to_string()),
            )}</Text>
        </View>
    }
}

/// `use_window_size` twice: once as a readout, once feeding a layout
/// attribute. A hook can decide how something is laid out, not just what it
/// says.
#[component]
fn Responsive(cx: &mut Cx) {
    let size = use_window_size(cx);
    let narrow = size.x < NARROW;

    rsx! {
        <View direction="column" gap={4}>
            <Text strong>"responsive"</Text>
            <SizeReadout/>
            <Text>{format!("layout: {}", if narrow { "column" } else { "row" })}</Text>
            <View direction={if narrow { "column" } else { "row" }} gap={8}>
                for label in ["one", "two", "three"] {
                    <Frame key={label} inner_margin={8.0}>
                        <Text>{label}</Text>
                    </Frame>
                }
            </View>
        </View>
    }
}

/// The second caller of `use_window_size`.
#[component]
fn SizeReadout(cx: &mut Cx) {
    let size = use_window_size(cx);
    rsx! { <Text>{format!("window: {:.0} x {:.0}", size.x, size.y)}</Text> }
}
