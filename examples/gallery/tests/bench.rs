//! What each gallery pane costs per frame, so the code pane does not hide
//! what the running example costs.
//!
//! ```sh
//! cargo test --release -p gallery --test bench -- --ignored --nocapture
//! ```
//!
//! Times `Harness::step` (layout + tessellation, no GPU), like list-10k's
//! bench. The list pane is private to the gallery, so it is rebuilt here the
//! way `lib.rs` draws it. The `code_view_ui` breakdown that found the cost is
//! in docs/tasks/code-pane/measurements.md; the code it timed is gone.

use egui_kittest::Harness;
use egui_react::prelude::*;
use egui_react_app::{root_id, root_style};
use egui_react_elements::prelude::*;
use example_meta::Meta;
use gallery::{App, Code, CodeEvent, EXAMPLES, all_tags, find};

thread_local! {
    static NO_A11Y: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

const SIZE: egui::Vec2 = egui::vec2(1280.0, 1000.0);
const WARMUP: usize = 5;
const FRAMES: usize = 30;

fn ms_per_frame<S>(harness: &mut Harness<'_, S>) -> f64 {
    for _ in 0..WARMUP {
        harness.step();
    }
    let start = std::time::Instant::now();
    for _ in 0..FRAMES {
        harness.step();
    }
    start.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64
}

/// Same as [`bench`], on a bare `egui::Context` with no accesskit: what a
/// native window costs. kittest turns accesskit on, as the web build does
/// through `WebA11y`.
fn bench_no_a11y(draw: impl Fn(&mut Cx<'_, '_>) + 'static) -> f64 {
    let ctx = egui::Context::default();
    let mut store = Store::new();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
        ..Default::default()
    };
    let mut frame = |ctx: &egui::Context| {
        let out = ctx.run_ui(input.clone(), |ui| {
            store.begin_pass(ui.ctx());
            {
                let store: &Store = &store;
                let mut cx = Cx::new(store, ui, root_id());
                cx.root_container(root_id(), root_style(), |cx| draw(cx));
            }
            store.end_pass();
        });
        let _ = ctx.tessellate(out.shapes, out.pixels_per_point);
    };
    for _ in 0..WARMUP {
        frame(&ctx);
    }
    let start = std::time::Instant::now();
    for _ in 0..FRAMES {
        frame(&ctx);
    }
    start.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64
}

fn bench(draw: impl Fn(&mut Cx<'_, '_>) + 'static) -> f64 {
    if NO_A11Y.with(|f| f.get()) {
        return bench_no_a11y(draw);
    }
    let mut harness = Harness::builder().with_size(SIZE).build_ui_state(
        move |ui, store: &mut Store| {
            store.begin_pass(ui.ctx());
            {
                let store: &Store = store;
                let mut cx = Cx::new(store, ui, root_id());
                cx.root_container(root_id(), root_style(), |cx| draw(cx));
            }
            store.end_pass();
        },
        Store::new(),
    );
    ms_per_frame(&mut harness)
}

fn full(name: &'static str) -> f64 {
    bench(move |cx| rsx! { <App start={name}/> }.show(cx))
}

/// The gallery's own code column, on its own.
fn code_pane(meta: &'static Meta) -> f64 {
    bench(move |cx| {
        let view = rsx! {
            <View direction="row" grow={1.0} gap={8}>
                <View w={200.0} shrink={0.0}/>
                <View grow={1.0} min_w={0.0}/>
                <Code
                    w="40%"
                    min_w={360.0}
                    shrink={0.0}
                    meta={*meta}
                    plain={false}
                    on_pick={|_: bool| {}}
                />
            </View>
        };
        view.show(cx);
    })
}

/// The running example in the centre column, with the other two as empty
/// boxes of the same size.
fn running_pane(name: &'static str) -> f64 {
    bench(move |cx| {
        let view = rsx! {
            <View direction="row" grow={1.0} gap={8}>
                <View w={200.0} shrink={0.0}/>
                <View direction="column" grow={1.0} min_w={0.0}>
                    match name {
                        "list-10k" => { <list_10k::App initial_count={1_000}/> }
                        "counter" => { <counter::App/> }
                        "board" => { <board::App/> }
                        "patch" => { <patch::App/> }
                        "spreadsheet" => { <spreadsheet::App/> }
                        _ => { <notes::App/> }
                    }
                </View>
                <View w="40%" min_w={360.0} shrink={0.0}/>
            </View>
        };
        view.show(cx);
    })
}

/// The list column: one button per example and the tag chips.
fn list_pane() -> f64 {
    bench(move |cx| {
        let view = rsx! {
            <View direction="row" grow={1.0} gap={8}>
                <View direction="column" w={200.0} shrink={0.0} gap={6}>
                    <Text strong size={18.0}>"examples"</Text>
                    <ScrollArea grow={1.0}>
                        <View direction="column" gap={4} w="100%">
                            for meta in EXAMPLES.iter() {
                                <View key={meta.name} direction="row" gap={4} align="center" w="100%">
                                    <Text w={12.0}>" "</Text>
                                    <Button grow={1.0} on_click={|| {}}>{meta.name}</Button>
                                </View>
                            }
                            <Separator/>
                            <View direction="row" wrap gap={(4.0, 4.0)} w="100%">
                                for tag in all_tags() {
                                    <Button key={tag} on_click={|| {}}>{tag}</Button>
                                }
                            </View>
                        </View>
                    </ScrollArea>
                </View>
                <View grow={1.0} min_w={0.0}/>
                <View w="40%" min_w={360.0} shrink={0.0}/>
            </View>
        };
        view.show(cx);
    })
}

#[test]
#[ignore = "a measurement, and only meaningful in release"]
fn frame_time_by_pane() {
    println!("== with accesskit (kittest, like the web build) ==");
    report();
    NO_A11Y.with(|f| f.set(true));
    println!();
    println!("== without accesskit (bare Context, like native) ==");
    report();
}

fn report() {
    println!(
        "{:>12} {:>6} {:>9} {:>9} {:>9} {:>9}",
        "example", "lines", "full", "code", "running", "list"
    );
    let list = list_pane();
    for name in [
        "counter",
        "list-10k",
        "notes",
        "board",
        "patch",
        "spreadsheet",
    ] {
        let meta = find(name).unwrap();
        println!(
            "{name:>12} {:>6} {:>6.2} ms {:>6.2} ms {:>6.2} ms {:>6.2} ms",
            meta.source.lines().count(),
            full(name),
            code_pane(meta),
            running_pane(name),
            list,
        );
    }
}
