//! A wgpu fragment shader in a `<Canvas>`, driven by ordinary hooks.
//!
//! What it draws is a black hole: one fullscreen triangle, and for every pixel
//! of it a photon traced backwards from the camera through curved space until
//! it meets the accretion disk, the sky, or the horizon. `shader.wgsl` is where
//! that lives and it explains itself; the interesting part here is that none of
//! the code below knows any of it.
//!
//! Three pieces meet here, and each stays on its own side of the fence.
//!
//! The pipeline is built once, at startup, by `Options::setup` (see
//! `main.rs`). eframe hands that closure a `CreationContext` with a
//! `wgpu_render_state` in it, and `gpu::setup` puts a `ShaderResources` into
//! `renderer.write().callback_resources`. That is a `TypeMap`, so the entry is
//! found again by its type and cannot clash with another example's — which is
//! what lets the gallery run this one alongside the rest.
//!
//! The `<Canvas>` element knows nothing about any of it. It asks taffy for a
//! rect and calls `paint` with it; what goes in there is the caller's business.
//! Here it is one line: push an `egui_wgpu::Callback` onto the painter, and
//! egui will run it inside its own render pass with the viewport and scissor
//! already set to the rect.
//!
//! State is the point of the example. `speed`, `mass`, `bloom`, `tilt` and
//! `paused` are `use_state` like anywhere else, and the values are copied into
//! the callback struct, written to a uniform buffer in `prepare`, and read by
//! the WGSL.
//! Moving a slider changes a number in a hook and the picture changes; nothing
//! in between has to be told. `mass` is the one to try first: it is the
//! Schwarzschild radius the shader traces photons around, so it sets the size
//! of the shadow, how far the sky behind bends around it, and how much of the
//! disk the hole has eaten.
//!
//! Repainting is explicit, as always in egui: while the animation runs the app
//! asks for the next frame every frame, and `pause` simply stops asking, which
//! also freezes `time` at the value the last frame saw.

use egui_reactor::prelude::*;
use egui_reactor_elements::prelude::*;
use example_meta::Meta;

pub mod gpu;

pub const META: Meta = Meta {
    name: "shader",
    summary: "A ray-traced black hole in a <Canvas>, with sliders wired to its uniform.",
    hooks: &["use_state"],
    elements: &["View", "Canvas", "Slider", "Checkbox"],
    source: include_str!("lib.rs"),
    plain: None,
};

/// The shader canvas, four sliders and a pause box.
#[component]
pub fn App(cx: &mut Cx) {
    let mut speed = use_state(cx, || 1.0f32);
    let mut mass = use_state(cx, || 1.0f32);
    let mut bloom = use_state(cx, || 1.5f32);
    // Degrees, because that is what a slider labelled "tilt" should show; the
    // shader gets radians.
    let mut tilt = use_state(cx, || 22.0f32);
    let mut paused = use_state(cx, || false);
    let mut mouse = use_state(cx, || egui::Vec2::ZERO);

    // egui's own clock, in seconds since the app started. It stops moving when
    // no repaint is asked for, which is what makes `pause` work.
    let time = cx.ui().input(|i| i.time) as f32;
    // A shader animates, so it needs a frame a frame; asking here rather than
    // in `paint` keeps the request out of the drawing code.
    if !*paused {
        cx.ctx().request_repaint();
    }

    // Read the states once. The handlers below take them `&mut`, and `paint`
    // is a separate closure that would otherwise hold a borrow across them.
    let speed_value = *speed;
    let mass_value = *mass;
    let bloom_value = *bloom;
    let tilt_value = tilt.to_radians();
    let mouse_value = *mouse;
    let points_to_pixels = cx.ctx().pixels_per_point();

    rsx! {
        <View direction="column" gap={8} p={12} grow={1.0}>
            <Text size={22.0} strong>"shader"</Text>
            <Text>"Drag the canvas to orbit the camera."</Text>
            // `h={0}` with `grow={1}` is the flexbox idiom for "take what is
            // left and nothing more". A `<Canvas>` reports the whole window as
            // the size it could fill, and this column's height is decided by
            // its content, so `grow` on its own would size the column to the
            // canvas *plus* the controls and push the row below off the
            // bottom. Zero height and a share of the free space instead.
            <Canvas
                grow={1.0}
                h={0.0}
                sense={egui::Sense::drag()}
                on_drag={|delta: egui::Vec2| *mouse += delta}
                paint={move |ui: &mut egui::Ui, rect: egui::Rect| {
                    // The shader works in pixels, and a rect is in points.
                    let resolution = rect.size() * points_to_pixels;
                    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                        rect,
                        gpu::ShaderCallback {
                            time: time * speed_value,
                            mass: mass_value,
                            bloom: bloom_value,
                            tilt: tilt_value,
                            resolution,
                            mouse: mouse_value * points_to_pixels,
                        },
                    ));
                }}
            />
            <View direction="row" gap={12} align="center" wrap>
                <Slider bind={mass.bind()} range={0.1..=2.0} label="mass"/>
                <Slider bind={bloom.bind()} range={0.0..=4.0} label="bloom"/>
                <Slider bind={tilt.bind()} range={-60.0..=60.0} label="tilt"/>
                <Slider bind={speed.bind()} range={0.0..=4.0} label="speed"/>
                <Checkbox bind={paused.bind()} label="pause"/>
            </View>
        </View>
    }
}
