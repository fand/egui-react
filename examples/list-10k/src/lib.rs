//! A hundred thousand rows through `<VirtualList>` (the name is history).
//!
//! `<VirtualList>` draws only the rows the viewport can see — around forty of
//! them — and reserves the height of the rest, so the frame time does not
//! depend on the count. That is the standard immediate-mode answer to a long
//! list; the plain egui version next to it (`plain.rs`) does the same thing
//! with `ScrollArea::show_rows`, which is what `<VirtualList>` wraps. What
//! drawing every row would cost instead is measured in `tests/scenarios.rs`
//! (about 22 ms a frame at ten thousand rows, against 0.14 ms here; conditions
//! in docs/tasks/list-perf/measurements.md).
//!
//! The rows in view are the element's business: `render` is called with the
//! index of each row that is on screen, as react-virtualized calls
//! `rowRenderer`. What the app owns is the data — `filtered` is the rows that
//! survive the filter and the removals, and it is memoised because building
//! ten thousand `String`s every frame would cost more than drawing them.
//!
//! Run on its own (`cargo run -p list-10k`), the window has a switch at the
//! top, "plain egui": it swaps the whole list for the `plain.rs` version, so
//! the two can be compared in the same window at the same size. The gallery
//! has its own plain/react switch, so `<App>` itself does not carry this one.

use std::collections::BTreeSet;

use egui_react::prelude::*;
use egui_react_elements::prelude::*;
use example_meta::Meta;

pub mod plain;

use plain::PlainState;

pub const META: Meta = Meta {
    name: "list-10k",
    summary: "Ten thousand rows, and what drawing all of them costs.",
    hooks: &["use_state", "use_memo"],
    elements: &[
        "View",
        "Text",
        "Slider",
        "TextEdit",
        "Button",
        "VirtualList",
    ],
    source: include_str!("lib.rs"),
    plain: Some(include_str!("plain.rs")),
};

/// A word per row, so the filter has something to match on.
pub const WORDS: [&str; 8] = [
    "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel",
];

/// How many rows the example opens with.
///
/// A hundred thousand: ten times the name, because the point is that the count
/// does not matter to the frame. It matters to the filter, which rebuilds the
/// row list on every keystroke (about 20 ms at this size). The gallery and the
/// snapshot pass a smaller number to match the plain column next to them.
pub const DEFAULT_COUNT: usize = 100_000;

/// The height of one row, and the gap under it. The plain version needs both as
/// numbers; here they are the row's natural height and a `gap` attribute.
pub const ROW_H: f32 = 18.0;
pub const ROW_GAP: f32 = 2.0;

/// The width of the index column, so the names line up.
pub const INDEX_W: f32 = 64.0;

/// Row `i`'s text. Deterministic, so both versions and every machine agree.
pub fn row_name(i: usize) -> String {
    format!("row {i} {}", WORDS[i % WORDS.len()])
}

/// The rows that survive `removed` and `filter`, as `(index, text)`.
pub fn rows(count: usize, filter: &str, removed: &BTreeSet<usize>) -> Vec<(usize, String)> {
    (0..count)
        .filter(|i| !removed.contains(i))
        .map(|i| (i, row_name(i)))
        .filter(|(_, name)| filter.is_empty() || name.contains(filter))
        .collect()
}

/// `initial_count` is the row count to open with — a hundred thousand by
/// default; the gallery and the tests pass something smaller.
#[component]
pub fn App(cx: &mut Cx, #[prop(default = DEFAULT_COUNT)] initial_count: usize) {
    let mut count = use_state(cx, move || initial_count);
    let mut filter = use_state(cx, String::new);
    let mut removed = use_state(cx, BTreeSet::<usize>::new);

    let frame_ms = cx.ui().input(|i| i.stable_dt) * 1000.0;

    // Building ten thousand strings on every frame would be a bigger cost than
    // drawing them. The deps are what the list depends on; removals only ever
    // grow, so their count is enough to notice one.
    let filtered = use_memo(cx, (*count, filter.as_str(), removed.len()), || {
        rows(*count, filter.as_str(), &removed)
    });
    let shown = filtered.len();

    rsx! {
        <View direction="column" gap={8} p={12} grow={1.0}>
            <Text size={22.0} strong>"list-10k"</Text>

            <Slider bind={count.bind()} range={100..=100_000} label="rows"/>
            <TextEdit w={220.0} bind={filter.bind()} hint="filter"/>
            <View direction="row" gap={8} align="center">
                <Text>{format!("showing {shown}")}</Text>
                // Not a benchmark: one frame, as egui measured it, including
                // whatever else the machine was doing.
                <Text>{format!("last frame {frame_ms:.1} ms ({:.0} fps)", 1000.0 / frame_ms.max(0.001))}</Text>
            </View>

            // Render by index: only the rows in view are ever built, and the
            // element decides which those are.
            <VirtualList
                grow={1.0}
                rows={shown}
                row_h={ROW_H + ROW_GAP}
                render={|cx: &mut Cx<'_, '_>, row: usize| {
                    let (i, name) = &filtered[row];
                    rsx! { <Row index={*i} name={name.as_str()} on_remove={|| {
                        removed.insert(*i);
                    }}/> }
                    .show(cx);
                }}
            />
        </View>
    }
}

/// The standalone binary's root: [`App`] or the plain egui list, switched at
/// the top of the window, so the two can be compared without a second window.
/// Each keeps its own state; both open at a hundred thousand rows.
#[component]
pub fn Compare(cx: &mut Cx) {
    let mut plain = use_state(cx, || false);
    let show_plain = *plain;

    rsx! {
        <View direction="column" grow={1.0}>
            <View direction="row" gap={8} align="center" pl={12} pt={12}>
                <Checkbox bind={plain.bind()} label="plain egui"/>
                <Text>{if show_plain { "plain.rs: egui by hand" } else { "lib.rs: egui-react" }}</Text>
            </View>
            if show_plain {
                <PlainApp/>
            } else {
                <App/>
            }
        </View>
    }
}

/// [`plain::ui`] as a component: one `use_state` for its whole state, drawn
/// into a leaf that fills the rest of the window. What the gallery does for
/// every plain example.
#[component]
pub fn PlainApp(cx: &mut Cx) {
    let mut state = use_state(cx, PlainState::default);
    cx.leaf_fill(&ItemStyle::default().grow(1.0), |ui| {
        plain::ui(ui, state.bind());
    });
}

/// One row, built by `<VirtualList>` for each row in view.
#[component]
pub fn Row(cx: &mut Cx, index: usize, name: &str, #[event] on_remove: ()) {
    rsx! {
        <View direction="row" gap={8} align="center" w="100%" h={ROW_H}>
            <Text w={INDEX_W}>{format!("#{index}")}</Text>
            <Text grow={1.0}>{name}</Text>
            <Button label="remove" on_click={|| on_remove.emit(())}>"x"</Button>
        </View>
    }
}
