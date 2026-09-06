// The ground every generated patch program stands on: the uniform block, the
// fullscreen triangle, and the two colour conversions the HSV node needs.
//
// `codegen.rs` appends the node functions and `fs_main` to this, in that
// order, so that nothing is ever referred to before it is declared.

struct Uniforms {
    time: f32,
    pad: f32,
    resolution: vec2<f32>,
    // One vec4 per node in the picture. Which node owns which slot is decided
    // when the code is generated and written straight into the source.
    p: array<vec4<f32>, 32>,
};

@group(0) @binding(0) var<uniform> U: Uniforms;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    // The clip position carried through as a varying, so the fragment stage
    // gets -1..1 across the canvas rect without being told where the rect is.
    @location(0) uv: vec2<f32>,
};

// One triangle big enough to cover clip space; there is no vertex buffer to
// bind. egui-wgpu has already set the viewport and scissor to the canvas rect.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    let x = f32(i32(index) / 2) * 4.0 - 1.0;
    let y = f32(i32(index) & 1) * 4.0 - 1.0;
    var out: VertexOut;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

fn rgb2hsv(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = mix(vec4<f32>(c.bg, k.wz), vec4<f32>(c.gb, k.xy), step(c.b, c.g));
    let q = mix(vec4<f32>(p.xyw, c.r), vec4<f32>(c.r, p.yzx), step(p.x, c.r));
    let d = q.x - min(q.w, q.y);
    let e = 1.0e-10;
    return vec3<f32>(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

fn hsv2rgb(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(vec3<f32>(c.x) + k.xyz) * 6.0 - vec3<f32>(k.w));
    return c.z * mix(vec3<f32>(k.x), clamp(p - vec3<f32>(k.x), vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}
