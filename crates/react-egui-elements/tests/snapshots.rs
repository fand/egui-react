//! Plan 3.4 / test 4-4: pixel snapshots of the layout primitives.
//!
//! Behind the `snapshot` feature because it needs a GPU (or a software Vulkan
//! driver) through `egui_kittest`'s wgpu renderer:
//!
//! ```sh
//! cargo test -p react-egui-elements --features snapshot
//! ```
//!
//! Regenerate the images with `UPDATE_SNAPSHOTS=1` and commit them.

#![cfg(feature = "snapshot")]

mod common;

use common::run_app;
use egui_kittest::Harness;
use react_egui::prelude::*;
use react_egui_elements::prelude::*;

/// Draw `app` at a fixed size and compare the rendering with `tests/snapshots`.
fn snapshot(name: &str, app: impl Fn(&mut Cx<'_, '_>) + 'static) {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(400.0, 200.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::default())
        .build_ui_state(
            move |ui, store: &mut Store| {
                run_app(ui, store, |cx| app(cx));
            },
            Store::new(),
        );
    harness.run();
    harness.snapshot(name);
}

#[test]
fn snapshot_row() {
    snapshot("row", |cx| {
        rsx! {
            <View direction="row" gap={8} p={12} align="center">
                <Text>"left"</Text>
                <Text grow={1.0}>"middle"</Text>
                <Button>"right"</Button>
            </View>
        }
        .show(cx);
    });
}

#[test]
fn snapshot_column_justify() {
    snapshot("column_justify", |cx| {
        rsx! {
            <View direction="column" justify="space-between" gap={4} p={12} h={160.0}>
                <Text>"top"</Text>
                <Text>"middle"</Text>
                <Text>"bottom"</Text>
            </View>
        }
        .show(cx);
    });
}

#[test]
fn snapshot_grid() {
    snapshot("grid", |cx| {
        rsx! {
            <View display="grid" cols={3} gap={6} p={12} w={360.0}>
                <Text col_span={3}>"header"</Text>
                <Text>"a"</Text>
                <Text>"b"</Text>
                <Text>"c"</Text>
                <Text>"d"</Text>
                <Text>"e"</Text>
                <Text>"f"</Text>
            </View>
        }
        .show(cx);
    });
}

#[test]
fn snapshot_text_wrap() {
    let long = "the quick brown fox jumps over the lazy dog";
    snapshot("text_wrap", move |cx| {
        rsx! {
            <View direction="column" gap={8} p={12} w={160.0}>
                <Text>{long}</Text>
                <Text wrap>{long}</Text>
            </View>
        }
        .show(cx);
    });
}

#[test]
fn snapshot_widgets() {
    snapshot("widgets", |cx| {
        let mut text = use_state(cx, || String::from("edit me"));
        let mut flag = use_state(cx, || true);
        let mut amount = use_state(cx, || 3i32);
        rsx! {
            <View direction="column" gap={6} p={12} w={360.0}>
                <Button>"a button"</Button>
                <TextEdit bind={text.bind()}/>
                <Checkbox bind={flag.bind()} label="a checkbox"/>
                <Slider bind={amount.bind()} range={0..=10} label="amount"/>
                <Separator/>
                <Label>"a label"</Label>
            </View>
        }
        .show(cx);
    });
}
