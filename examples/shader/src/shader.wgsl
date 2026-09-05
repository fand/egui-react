// A fullscreen triangle and a plasma, driven by the app's state.

struct Uniform {
    time: f32,
    speed: f32,
    resolution: vec2<f32>,
    mouse: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniform;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    // The clip position carried through as a varying. `@builtin(position)` in
    // the fragment stage is in whole-framebuffer pixels, so it would need the
    // rect's offset to be useful; interpolating the clip coordinate gives
    // -1..1 across the canvas rect and nothing else has to be passed in.
    @location(0) uv: vec2<f32>,
};

// One triangle big enough to cover the whole clip space, so there is no vertex
// buffer to bind. egui-wgpu has already set the viewport and the scissor to
// the canvas rect, so the triangle lands exactly on it.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    let x = f32(i32(index) / 2) * 4.0 - 1.0;
    let y = f32(i32(index) & 1) * 4.0 - 1.0;
    var out: VertexOut;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // Square the coordinates up, so the pattern does not stretch with the
    // window.
    let size = max(u.resolution, vec2<f32>(1.0, 1.0));
    var uv = vec2<f32>(in.uv.x * size.x / size.y, in.uv.y);
    // Dragging pushes the pattern around: the drag total arrives in pixels.
    uv = uv + vec2<f32>(u.mouse.x, -u.mouse.y) / size.y * 2.0;

    let t = u.time;
    let r = length(uv);
    // Four waves out of step with each other, so the colours drift apart
    // instead of moving as one grey pulse.
    var v = sin(uv.x * 4.0 + t);
    v = v + sin(uv.y * 4.0 - t * 1.3);
    v = v + sin((uv.x + uv.y) * 3.0 + t * 0.7);
    v = v + sin(r * 8.0 - t * 2.0);
    v = v * 0.25;

    // The slider shows up in the picture as well as in the pace: a faster
    // setting is a warmer one, so a paused canvas still reacts to it.
    let warmth = clamp(u.speed * 0.25, 0.0, 1.0);
    let color = vec3<f32>(
        0.5 + 0.5 * sin(v * 3.14159 + warmth * 2.0),
        0.5 + 0.5 * sin(v * 3.14159 + 2.094),
        0.5 + 0.5 * sin(v * 3.14159 + 4.188 - warmth),
    );
    return vec4<f32>(color, 1.0);
}
