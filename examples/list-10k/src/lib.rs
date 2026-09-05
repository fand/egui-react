//! What a long list costs, told honestly.
//!
//! react-egui draws every row. `for` in `rsx!` is a real loop, each row is a
//! `<View>` with three children, and taffy lays out all of them whether they
//! are on screen or not. At ten thousand rows that is around forty thousand
//! taffy nodes per frame, and the frame time says so.
//!
//! The plain egui version next to it uses `ScrollArea::show_rows`, which draws
//! only the rows the viewport can see — around forty of them — and leaves a gap
//! of the right height above and below. That is the standard immediate-mode
//! answer to a long list, and react-egui has no equivalent today: `<ScrollArea>`
//! takes its children as an opaque `impl View` and cannot slice a `for` loop it
//! never sees. See tasks/examples/plan.md section 8.
//!
//! So this example is the one where plain egui wins, and the numbers are in the
//! readout at the top. Turn the count down to a few hundred and the difference
//! disappears; that is the honest shape of it.

use std::collections::BTreeSet;

use example_meta::Meta;
use react_egui::prelude::*;
use react_egui_elements::prelude::*;

pub mod plain;

pub const META: Meta = Meta {
    name: "list-10k",
    summary: "Ten thousand rows, and what drawing all of them costs.",
    hooks: &["use_state", "use_memo"],
    elements: &["View", "Text", "Slider", "TextEdit", "Button", "ScrollArea"],
    source: include_str!("lib.rs"),
    plain: Some(include_str!("plain.rs")),
};

/// A word per row, so the filter has something to match on.
pub const WORDS: [&str; 8] = [
    "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel",
];

/// How many rows the example opens with.
///
/// Ten thousand is the name of the example, and the point of it. The gallery
/// and the snapshot pass a smaller number, because a gallery that freezes for a
/// second when you click it is not showing anything useful.
pub const DEFAULT_COUNT: usize = 10_000;

/// The height of one row, and the gap under it. The plain version needs both as
/// numbers; here they are the row's natural height and a `gap` attribute.
pub const ROW_H: f32 = 18.0;
pub const ROW_GAP: f32 = 2.0;

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

/// `initial_count` is the row count to open with. The default is the ten
/// thousand in the name; the gallery and the tests pass something smaller.
#[component]
pub fn App(cx: &mut Cx, #[prop(default = DEFAULT_COUNT)] initial_count: usize) {
    let mut count = use_state(cx, move || initial_count);
    let mut filter = use_state(cx, String::new);
    let mut removed = use_state(cx, BTreeSet::<usize>::new);

    let frame_ms = cx.ui().input(|i| i.stable_dt) * 1000.0;

    // Building ten thousand strings on every frame would be a bigger cost than
    // drawing them. The deps are what the list depends on; removals only ever
    // grow, so their count is enough to notice one.
    let visible = use_memo(cx, (*count, filter.as_str(), removed.len()), || {
        rows(*count, filter.as_str(), &removed)
    });
    let shown = visible.len();

    rsx! {
        <View direction="column" gap={8} p={12} grow={1.0}>
            <Text size={22.0} strong>"list-10k"</Text>

            <Slider bind={count.bind()} range={100..=10_000} label="rows"/>
            <TextEdit w={220.0} bind={filter.bind()} hint="filter"/>
            <View direction="row" gap={8} align="center">
                <Text>{format!("showing {shown}")}</Text>
                // Not a benchmark: one frame, as egui measured it, including
                // whatever else the machine was doing.
                <Text>{format!("last frame {frame_ms:.1} ms ({:.0} fps)", 1000.0 / frame_ms.max(0.001))}</Text>
            </View>

            <ScrollArea grow={1.0}>
                <View direction="column" gap={ROW_GAP} w="100%">
                    // Every one of these is laid out, on screen or not.
                    for (i, name) in visible.iter() {
                        <View key={i} direction="row" gap={8} align="center" w="100%">
                            <Text w={64.0}>{format!("#{i}")}</Text>
                            <Text grow={1.0}>{name.as_str()}</Text>
                            <Button on_click={|| {
                                removed.insert(*i);
                            }}>"x"</Button>
                        </View>
                    }
                </View>
            </ScrollArea>
        </View>
    }
}
