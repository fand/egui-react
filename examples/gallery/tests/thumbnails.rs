//! The thumbnails on the site's Examples page: every example in `EXAMPLES`,
//! drawn once at the same size and written to `site/public/thumbs/<name>.png`.
//!
//! A generator, not a check. It is a test so it can use the same
//! `egui_kittest` harness as `snapshots.rs`, and it sits behind the `snapshot`
//! feature for the same reason: it needs a GPU. Regenerate the images with
//!
//! ```sh
//! cargo test -p gallery --features snapshot --test thumbnails
//! ```
//!
//! and commit them: the site build has no GPU, so it reads the files as they
//! are. `shell` has no thumbnail, because it does not run inside `Running`.

#![cfg(feature = "snapshot")]

use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use egui_kittest::kittest::Queryable as _;
use egui_kittest::Harness;
use egui_react::prelude::*;
use egui_react_app::{root_id, root_style};
use egui_react_elements::prelude::*;
use gallery::{EXAMPLES, Running};

/// The picture's size in points. 16:10, like the cards.
const SIZE: egui::Vec2 = egui::vec2(640.0, 400.0);

/// Two device pixels per point, so the picture is sharp on a retina card.
const SCALE: f32 = 2.0;

/// How long an example gets to settle before its picture is taken: enough for
/// `fetch` to get its response and `font` its web font over a normal link.
const SETTLE: Duration = Duration::from_secs(2);

fn thumbs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../site/public/thumbs")
}

fn thumbnail(name: &'static str) {
    let mut harness = Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(SCALE)
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::default())
        .build_ui_state(
            move |ui, store: &mut Store| {
                store.begin_pass(ui.ctx());
                {
                    let store: &Store = store;
                    let mut cx = Cx::new(store, ui, root_id());
                    cx.root_container(root_id(), root_style(), |cx| {
                        rsx! {
                            <View direction="column" grow={1.0}>
                                <Running name={name} plain={false}/>
                            </View>
                        }
                        .show(cx)
                    });
                }
                store.end_pass();
            },
            Store::new(),
        );
    harness.step();
    drive(name, &mut harness);
    // The clicks above leave a hovered button and a painted cursor behind.
    harness.remove_cursor();

    let started = std::time::Instant::now();
    // `step`, not `run`: `shader`, `clock` and a `Suspense` fallback ask for
    // a repaint every frame, which `run` reads as a loop that never settles.
    while started.elapsed() < SETTLE {
        harness.step();
        sleep(Duration::from_millis(50));
    }

    let image = harness.render().expect("render the example");
    let dir = thumbs_dir();
    std::fs::create_dir_all(&dir).expect("create site/public/thumbs");
    image
        .save(dir.join(format!("{name}.png")))
        .expect("write the thumbnail");
}

/// A few clicks so the picture has something in it: an empty list or an empty
/// notebook would say nothing about the example.
fn drive(name: &str, harness: &mut Harness<'_, Store>) {
    match name {
        "notes" => {
            for _ in 0..2 {
                harness.get_by_label("new").click_accesskit();
                harness.run();
                harness.run();
            }
        }
        "todo" => {
            for text in ["milk", "eggs", "bread"] {
                harness
                    .get_by_role(egui::accesskit::Role::TextInput)
                    .focus();
                harness.run();
                harness
                    .get_by_role(egui::accesskit::Role::TextInput)
                    .type_text(text);
                harness.run();
                harness.key_press(egui::Key::Enter);
                harness.run();
                harness.run();
            }
            harness
                .get_all_by_role(egui::accesskit::Role::CheckBox)
                .next()
                .expect("a checkbox for the first item")
                .click();
            harness.run();
            harness.run();
        }
        "form" => {
            harness
                .get_by_role(egui::accesskit::Role::TextInput)
                .focus();
            harness.run();
            harness
                .get_by_role(egui::accesskit::Role::TextInput)
                .type_text("ada");
            harness.run();
            harness
                .get_all_by_role(egui::accesskit::Role::CheckBox)
                .next()
                .expect("the notify checkbox")
                .click();
            harness.run();
            harness.run();
        }
        "escape-hatch" => {
            for _ in 0..4 {
                harness.get_by_label("add sample").click();
                harness.run();
                harness.run();
            }
        }
        "counter" => {
            for _ in 0..3 {
                harness.get_by_label("+").click();
                harness.run();
                harness.run();
            }
        }
        _ => {}
    }
}

#[test]
fn every_example() {
    for meta in EXAMPLES {
        thumbnail(meta.name);
    }
}
