//! Matched CPU benchmark: real React Row/VirtualList, shared controls and input.
//! Run: cargo test --release -p list-10k --test scenarios -- --ignored --nocapture
//! No GPU, window scheduling, or AccessKit. See docs/tasks/perf/measurements.md.
use egui_react::prelude::*;
use egui_react_app::{root_id, root_style};
use egui_react_elements::prelude::*;
use list_10k::{INDEX_W, ROW_GAP, ROW_H, Row, RowEvent};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

const FRAMES: usize = 120;
const WARMUP: usize = 30;

#[derive(Clone, Copy, Debug)]
enum Mode {
    All,
    Virtual,
    Plain,
}
#[derive(Clone, Copy, Debug)]
enum Scenario {
    Idle,
    Scroll,
    Filter,
    Resize,
}
#[derive(Default)]
struct Pass {
    rows: Vec<usize>,
    reasons: Vec<egui::RepaintCause>,
    refused: bool,
}
struct Sample {
    ms: f64,
    passes: Vec<Pass>,
}
struct Fixture {
    ctx: egui::Context,
    store: Store,
    cache: Vec<(usize, String)>,
    filter: String,
    cached_filter: Option<String>,
    frame: usize,
}
impl Fixture {
    fn new() -> Self {
        let ctx = egui::Context::default();
        ctx.options_mut(|o| o.max_passes = std::num::NonZeroUsize::new(3).unwrap());
        Self {
            ctx,
            store: Store::new(),
            cache: Vec::new(),
            filter: String::new(),
            cached_filter: None,
            frame: 0,
        }
    }
    fn step(&mut self, mode: Mode, scenario: Scenario, n: usize, active: bool) -> Sample {
        let size = if active && matches!(scenario, Scenario::Resize) {
            egui::vec2(600.0 + (n % 40) as f32 * 4.0, 800.0 + (n % 30) as f32 * 2.0)
        } else {
            egui::vec2(600.0, 800.0)
        };
        // Controlled filter edits, before drawing, at the same frame in both modes.
        // Includes cache rebuilding in the timed region; excludes keyboard/focus handling.
        if active && matches!(scenario, Scenario::Filter) {
            self.filter = ["a", "al", "alp", "alpha", ""][n % 5].to_owned();
        }
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(self.frame as f64 / 120.0),
            predicted_dt: 1.0 / 120.0,
            ..Default::default()
        };
        input
            .events
            .push(egui::Event::PointerMoved(egui::pos2(300.0, 400.0)));
        if active && matches!(scenario, Scenario::Scroll) {
            if n == 0 {
                input.events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    phase: egui::TouchPhase::Start,
                    delta: egui::Vec2::ZERO,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            input.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                delta: egui::vec2(0.0, -20.0),
                modifiers: egui::Modifiers::NONE,
            });
        }
        self.frame += 1;
        let mut passes = Vec::new();
        let start = Instant::now();
        let output = self.ctx.run_ui(input, |ui| {
            let rows = RefCell::new(Vec::new());
            if self.cached_filter.as_ref() != Some(&self.filter) {
                self.cache = list_10k::rows(10_000, &self.filter, &BTreeSet::new());
                self.cached_filter = Some(self.filter.clone());
            }
            egui::CentralPanel::default().show(ui, |ui| {
                // Identical toolbar and reserved height; no live FPS text affecting layout.
                ui.allocate_ui(egui::vec2(ui.available_width(), 100.0), |ui| {
                    ui.heading("list-10k");
                    ui.label(format!("showing {}", self.cache.len()));
                    ui.add(egui::TextEdit::singleline(&mut self.filter).desired_width(220.0));
                });
                // show_rows adds item_spacing.y. Zero here gives exactly 20pt in both modes.
                ui.spacing_mut().item_spacing.y = 0.0;
                let cache = &self.cache;
                match mode {
                    Mode::Plain => {
                        egui::ScrollArea::vertical().show_rows(ui, ROW_H + ROW_GAP, cache.len(), |ui, range| {
                            for row in range {
                                rows.borrow_mut().push(row);
                                let (index, name) = &cache[row];
                                ui.push_id(row, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.allocate_ui_with_layout(egui::vec2(INDEX_W, ROW_H), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                            ui.set_min_width(INDEX_W);
                                            ui.label(format!("#{index}"));
                                        });
                                        ui.label(name);
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { let _ = ui.button("x"); });
                                    });
                                    ui.add_space(ROW_GAP);
                                });
                            }
                        });
                    }
                    Mode::All | Mode::Virtual => {
                        self.store.begin_pass(ui.ctx());
                        let mut cx = Cx::new(&self.store, ui, root_id());
                        cx.root_container(root_id(), root_style(), |cx| {
                            match mode {
                                Mode::Virtual => {
                                    rsx! { <VirtualList grow={1.0} rows={cache.len()} row_h={ROW_H + ROW_GAP}
                                        render={|cx: &mut Cx<'_, '_>, row: usize| {
                                            rows.borrow_mut().push(row);
                                            let (index, name) = &cache[row];
                                            rsx! { <Row index={*index} name={name.as_str()} on_remove={|| {}}/> }.show(cx);
                                        }}/>
                                    }.show(cx);
                                }
                                Mode::All => {
                                    rsx! { <ScrollArea grow={1.0}><View direction="column" gap={ROW_GAP} w="100%">
                                        for (row, (index, name)) in cache.iter().enumerate() {
                                            { |cx: &mut Cx<'_, '_>| {
                                                rows.borrow_mut().push(row);
                                                rsx! { <Row key={index} index={*index} name={name.as_str()} on_remove={|| {}}/> }.show(cx);
                                            } }
                                        }
                                    </View></ScrollArea> }.show(cx);
                                }
                                Mode::Plain => unreachable!(),
                            }
                        });
                        self.store.end_pass();
                        // Same fallback as the application runner.
                        if ui.ctx().output(|o| o.requested_discard()) && !ui.ctx().will_discard() { ui.ctx().request_repaint(); }
                    }
                }
            });
            passes.push(Pass {
                rows: rows.into_inner(),
                reasons: ui.ctx().output(|o| o.request_discard_reasons.clone()),
                refused: ui.ctx().output(|o| o.requested_discard()) && !ui.ctx().will_discard(),
            });
        });
        std::hint::black_box(self.ctx.tessellate(output.shapes, output.pixels_per_point));
        Sample {
            ms: start.elapsed().as_secs_f64() * 1000.0,
            passes,
        }
    }
}

#[test]
#[ignore = "release CPU benchmark"]
fn matched_scenarios() {
    if cfg!(debug_assertions) {
        panic!("run with --release");
    }
    println!(
        "scenario,mode,mean_ms,p50_ms,p95_ms,passes_per_frame,discard_frames,refused_frames,final_rows_min,final_rows_max,rows_all_passes_per_frame"
    );
    let mut csv = String::from(
        "scenario,mode,frame,pass,frame_cpu_ms,submitted_rows,first_row,last_row,discard_requests,refused,reasons\n",
    );
    for scenario in [
        Scenario::Idle,
        Scenario::Scroll,
        Scenario::Filter,
        Scenario::Resize,
    ] {
        let mut fixtures = [Fixture::new(), Fixture::new(), Fixture::new()];
        let modes = [Mode::All, Mode::Virtual, Mode::Plain];
        for (fixture, mode) in fixtures.iter_mut().zip(modes) {
            for n in 0..WARMUP {
                fixture.step(mode, scenario, n, false);
            }
        }
        let mut samples: [Vec<Sample>; 3] = std::array::from_fn(|_| Vec::new());
        for n in 0..FRAMES {
            // Rotate order to reduce systematic first/last bias.
            for offset in 0..3 {
                let i = (n + offset) % 3;
                samples[i].push(fixtures[i].step(modes[i], scenario, n, true));
            }
            assert_eq!(
                samples[1][n].passes.last().unwrap().rows.len(),
                samples[2][n].passes.last().unwrap().rows.len(),
                "visible callback counts differ: {scenario:?}, frame {n}"
            );
        }
        let range_mismatches = (0..FRAMES)
            .filter(|&n| {
                samples[1][n].passes.last().unwrap().rows
                    != samples[2][n].passes.last().unwrap().rows
            })
            .count();
        println!("range_mismatch_frames {scenario:?}: {range_mismatches}/{FRAMES}");
        if matches!(scenario, Scenario::Scroll) {
            assert!(
                *samples[1]
                    .last()
                    .unwrap()
                    .passes
                    .last()
                    .unwrap()
                    .rows
                    .first()
                    .unwrap()
                    > 50
            );
        }
        for (mode, samples) in modes.into_iter().zip(samples) {
            let mut times: Vec<_> = samples.iter().map(|s| s.ms).collect();
            times.sort_by(f64::total_cmp);
            let final_rows: Vec<_> = samples
                .iter()
                .map(|s| s.passes.last().unwrap().rows.len())
                .collect();
            let mut reasons = BTreeMap::<String, usize>::new();
            for (frame, sample) in samples.iter().enumerate() {
                for (pass, data) in sample.passes.iter().enumerate() {
                    csv.push_str(&format!(
                        "{scenario:?},{mode:?},{frame},{pass},{:.6},{},{},{},{},{},{}\n",
                        sample.ms,
                        data.rows.len(),
                        data.rows.first().map_or(String::new(), usize::to_string),
                        data.rows.last().map_or(String::new(), usize::to_string),
                        data.reasons.len(),
                        data.refused,
                        data.reasons
                            .iter()
                            .map(|r| r.reason.as_ref())
                            .collect::<Vec<_>>()
                            .join(";")
                    ));
                }
            }
            for p in samples.iter().flat_map(|s| &s.passes) {
                for reason in &p.reasons {
                    *reasons.entry(reason.reason.to_string()).or_default() += 1;
                }
            }
            println!(
                "{scenario:?},{mode:?},{:.3},{:.3},{:.3},{:.2},{},{},{},{},{:.2}",
                times.iter().sum::<f64>() / FRAMES as f64,
                times[FRAMES / 2],
                times[FRAMES * 95 / 100],
                samples.iter().map(|s| s.passes.len()).sum::<usize>() as f64 / FRAMES as f64,
                samples
                    .iter()
                    .filter(|s| s.passes.iter().any(|p| !p.reasons.is_empty()))
                    .count(),
                samples
                    .iter()
                    .filter(|s| s.passes.iter().any(|p| p.refused))
                    .count(),
                final_rows.iter().min().unwrap(),
                final_rows.iter().max().unwrap(),
                samples
                    .iter()
                    .flat_map(|s| &s.passes)
                    .map(|p| p.rows.len())
                    .sum::<usize>() as f64
                    / FRAMES as f64
            );
            println!("reasons {scenario:?}/{mode:?}: {reasons:?}");
        }
    }
    if let Ok(path) = std::env::var("PERF_CSV") {
        std::fs::write(path, csv).unwrap();
    }
}
