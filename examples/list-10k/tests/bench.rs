//! What the two versions cost per frame, at three list sizes.
//!
//! Ignored by default: it is a measurement, not an assertion, and it is only
//! worth anything in release.
//!
//! ```sh
//! cargo test --release -p list-10k --test bench -- --ignored --nocapture
//! ```
//!
//! It times `Harness::step`, which runs layout and tessellation but no GPU, so
//! the numbers are the CPU side of a frame and not what a monitor would show.
//! That is the side this example is about.
//!
//! Three ways to draw the same list: every row through `<ScrollArea>` + `for`,
//! only the visible ones through `<VirtualList>`, and only the visible ones
//! through egui's own `ScrollArea::show_rows`.

use egui_kittest::Harness;
use list_10k::App;
use list_10k::plain::{self, PlainState};
use egui_react::prelude::*;
use egui_react_app::{root_id, root_style};

const SIZE: egui::Vec2 = egui::vec2(600.0, 800.0);
const WARMUP: usize = 5;
const FRAMES: usize = 20;

fn milliseconds_per_frame<S>(harness: &mut Harness<'_, S>) -> f64 {
    for _ in 0..WARMUP {
        harness.step();
    }
    let start = std::time::Instant::now();
    for _ in 0..FRAMES {
        harness.step();
    }
    start.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64
}

fn react(count: usize, virtualise: bool) -> f64 {
    let mut harness = Harness::builder().with_size(SIZE).build_ui_state(
        move |ui, store: &mut Store| {
            store.begin_pass(ui.ctx());
            {
                let store: &Store = store;
                let mut cx = Cx::new(store, ui, root_id());
                let view = rsx! { <App initial_count={count} virtualise={virtualise}/> };
                cx.root_container(root_id(), root_style(), |cx| view.show(cx));
            }
            store.end_pass();
        },
        Store::new(),
    );
    milliseconds_per_frame(&mut harness)
}

fn plain(count: usize) -> f64 {
    let mut harness = Harness::builder().with_size(SIZE).build_ui_state(
        |ui, state: &mut PlainState| {
            ui.set_min_size(ui.available_size());
            plain::ui(ui, state);
        },
        PlainState::with_count(count),
    );
    milliseconds_per_frame(&mut harness)
}

#[test]
#[ignore = "a measurement, and only meaningful in release"]
fn frame_time_by_row_count() {
    println!(
        "{:>8}  {:>14}  {:>14}  {:>14}",
        "rows", "ScrollArea+for", "VirtualList", "plain show_rows"
    );
    for count in [100usize, 1_000, 10_000] {
        println!(
            "{count:>8}  {:>11.2} ms  {:>11.2} ms  {:>11.2} ms",
            react(count, false),
            react(count, true),
            plain(count),
        );
    }
}
