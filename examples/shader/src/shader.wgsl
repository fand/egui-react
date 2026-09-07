// A gravitationally lensed black hole, driven by the app's state.
//
// One fullscreen triangle, and for every pixel of it a photon traced backwards
// from the camera through curved space. What the photon runs into on the way —
// the accretion disk, the sky, or the horizon — is the colour of the pixel.

struct Uniform {
    time: f32,
    // The Schwarzschild radius in scene units. The slider writes it, and it
    // sets both the size of the shadow and how hard the light bends.
    mass: f32,
    resolution: vec2<f32>,
    mouse: vec2<f32>,
    // 1.0 when egui is drawing into an sRGB surface format, 0.0 when it is
    // not. `gpu::setup` reads it off the render state; see the end of
    // `fs_main` for what it is for.
    srgb_target: f32,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> u: Uniform;

// How many integration steps a photon gets before it is given up on. A ray
// that runs out is one caught in the photon sphere, spiralling; black is the
// right answer for it anyway.
const STEPS: i32 = 190;
// The disk's outer edge, in scene units. Its inner edge follows the mass.
const DISK_OUTER: f32 = 9.0;
const CAM_DISTANCE: f32 = 17.0;

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

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(0.1031, 0.1030));
    q = q + dot(q, q.yx + 33.33);
    return fract((q.x + q.y) * q.x);
}

fn hash31(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q = q + dot(q, q.zyx + 31.32);
    return fract((q.x + q.y) * q.z);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    // Smoothstep on the cell coordinate, so neighbouring cells meet without a
    // crease.
    let w = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, w.x), mix(c, d, w.x), w.y);
}

fn fbm2(p0: vec2<f32>) -> f32 {
    var p = p0;
    var sum = 0.0;
    var amp = 0.5;
    for (var i = 0; i < 4; i++) {
        sum = sum + amp * noise2(p);
        // An offset with the doubling, so the octaves do not line up on the
        // lattice and print a grid into the result.
        p = p * 2.07 + vec2<f32>(1.7, 9.2);
        amp = amp * 0.5;
    }
    return sum;
}

/// What is behind the hole: stars, and enough nebula to make the lensing
/// visible on something continuous as well as on the points.
///
/// The stars are a hash over a grid of directions rather than a texture. A
/// cell is lit when its hash lands in the top fraction of a percent, which
/// puts a few hundred bright ones and a few thousand faint ones on the sphere.
fn sky(dir: vec3<f32>) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    var scale = 130.0;
    var weight = 2.2;
    for (var i = 0; i < 2; i++) {
        let s = dir * scale;
        let cell = floor(s);
        let mag = smoothstep(0.994, 1.0, hash31(cell));
        // Put the star at the centre of its cell and fall off with the
        // distance to it. Lighting the whole cell instead would draw the
        // lattice: every star would come out as the same little square.
        let d = length(s - cell - 0.5);
        let point = pow(smoothstep(0.62, 0.0, d), 2.5);
        let tint = hash31(cell + 17.0);
        let star = mix(vec3<f32>(0.62, 0.76, 1.0), vec3<f32>(1.0, 0.86, 0.62), tint);
        col = col + star * mag * point * weight;
        scale = scale * 2.4;
        weight = weight * 0.3;
    }
    // Two noise fields multiplied together, sampled on different pairs of
    // components so that the result is continuous over the whole sphere and
    // has no seam to hide. The `smoothstep` is what keeps it a few wisps on
    // near-black rather than a flat wash over the whole sky.
    let dust = fbm2(dir.xz * 2.2 + 3.0) * fbm2(dir.yx * 1.9 + 8.0);
    let cloud = smoothstep(0.22, 0.55, dust);
    return col + vec3<f32>(0.004, 0.006, 0.018) + vec3<f32>(0.075, 0.045, 0.13) * cloud;
}

/// One layer of the disk's gas, wound up by `age` seconds of Keplerian shear.
///
/// The inner gas laps the outer many times over. Rotating the *sample point*
/// by an angle that depends on its own radius is what turns plain noise into
/// spiral arms, and it costs one sin and one cos.
fn gas(p: vec3<f32>, radius: f32, age: f32) -> f32 {
    let a = -age * 1.6 / pow(radius, 1.5);
    let c = cos(a);
    let s = sin(a);
    let q = vec2<f32>(p.x * c - p.z * s, p.x * s + p.z * c);
    return fbm2(q * 1.3) * (0.4 + 0.8 * noise2(q * 3.4));
}

/// How long a layer of gas is wound before it is started over, in seconds.
const CHURN: f32 = 7.0;

/// The accretion disk, sampled where a ray crossed the y = 0 plane.
///
/// `ray_dir` is the direction the *trace* is travelling, which is the reverse
/// of the photon's own path — the Doppler term below needs the difference.
fn disk(p: vec3<f32>, ray_dir: vec3<f32>, t: f32, rs: f32, inner: f32) -> vec3<f32> {
    let radius = length(p.xz);
    if radius < inner || radius > DISK_OUTER {
        return vec3<f32>(0.0);
    }
    // 0 at the inner edge, 1 at the outer one.
    let f = (radius - inner) / (DISK_OUTER - inner);

    // Shear left to itself draws the noise out into ever finer filaments, and
    // before long into aliased rings. So two layers are wound half a cycle
    // apart and cross-faded: neither is ever more than half a cycle old, and
    // each is weighted to zero at the moment it starts over, which is what
    // hides the restart.
    let phase = fract(t / CHURN);
    let w = 1.0 - abs(2.0 * phase - 1.0);
    let turb = mix(
        gas(p, radius, fract(phase + 0.5) * CHURN),
        gas(p, radius, phase * CHURN),
        w,
    );

    // Soft at both edges, so the disk does not end on a hard circle.
    let edge = smoothstep(0.0, 0.09, f) * smoothstep(1.0, 0.72, f);
    let heat = pow(1.0 - f, 2.6);
    var col = mix(vec3<f32>(1.0, 0.30, 0.05), vec3<f32>(1.0, 0.95, 0.88), heat);

    // Relativistic beaming. The gas orbits at a good fraction of c, and the
    // half of the disk turning towards the camera comes out brighter and bluer
    // than the half turning away. It is what makes a real picture of one
    // lopsided rather than symmetric, so it is worth the four lines.
    let beta = clamp(sqrt(rs / (2.0 * radius)), 0.0, 0.8);
    let tangent = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), p));
    let approach = dot(tangent, -ray_dir);
    let doppler = 1.0 / max(1.0 - beta * approach, 0.12);
    col = mix(col, vec3<f32>(0.72, 0.86, 1.0), clamp((doppler - 1.0) * 0.55, 0.0, 0.55));

    return col * edge * turb * (0.5 + 3.2 * heat) * pow(doppler, 3.0) * 0.55;
}

/// The filmic curve everyone uses, because the disk is additive and goes well
/// past 1.0 where it is bright and clipping it would flatten the core to a
/// disc of pure white.
fn tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = x * (2.51 * x + 0.03);
    let b = x * (2.43 * x + 0.59) + 0.14;
    return clamp(a / b, vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // Square the coordinates up, so the picture does not stretch with the rect.
    let size = max(u.resolution, vec2<f32>(1.0, 1.0));
    let p = vec2<f32>(in.uv.x * size.x / size.y, in.uv.y);

    // Dragging orbits the camera: the drag total arrives in pixels. The pitch
    // starts just above the disk plane, which is the angle that shows both the
    // far side lensed over the top and the near side under the shadow.
    let yaw = u.mouse.x / size.y * 2.4;
    let pitch = clamp(0.15 + u.mouse.y / size.y * 2.4, -1.45, 1.45);
    let cam = vec3<f32>(sin(yaw) * cos(pitch), sin(pitch), cos(yaw) * cos(pitch))
        * CAM_DISTANCE;

    let fwd = normalize(-cam);
    let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), fwd));
    let up = cross(fwd, right);

    var pos = cam;
    // 1.7 is the focal length: larger is a narrower field of view.
    var vel = normalize(fwd * 1.7 + right * p.x + up * p.y);

    let rs = max(u.mass, 0.02);
    // The innermost stable circular orbit sits at 3 * rs, so that is where the
    // gas can start. A heavier hole eats into the disk from the inside.
    let inner = min(3.0 * rs, DISK_OUTER * 0.8);

    // Angular momentum. The pull below is central, so this is conserved along
    // the whole path and can be computed once.
    let hv = cross(pos, vel);
    let h2 = dot(hv, hv);

    var color = vec3<f32>(0.0);
    var escaped = false;

    for (var i = 0; i < STEPS; i++) {
        let r2 = dot(pos, pos);
        let r = sqrt(r2);
        // Through the horizon: nothing comes back out, so the pixel keeps
        // whatever the disk gave it on the way in and gets no sky.
        if r < rs {
            break;
        }
        // Outbound and far enough that nothing left can bend it back.
        if r > 22.0 && dot(pos, vel) > 0.0 {
            escaped = true;
            break;
        }

        // A step that shrinks as the ray falls in, so the turn near the hole is
        // resolved without spending the budget out in the empty part...
        var dt = clamp(0.08 * r, 0.02, 1.0);
        // ...and a shorter one near the disk plane, where the crossing below
        // is found by interpolating across a single step.
        if abs(pos.y) < 1.5 && r < DISK_OUTER + 1.0 {
            dt = min(dt, 0.15);
        }

        // Null geodesics in Schwarzschild, written in Cartesian coordinates: a
        // central pull that falls off as 1/r^5 rather than 1/r^2. Circular
        // motion under it needs r = 1.5 * rs, which is the photon sphere — the
        // thin bright ring that hugs the shadow is light that went round it.
        let acc = -1.5 * h2 * rs * pos / (r2 * r2 * r);
        let next = pos + vel * dt + 0.5 * acc * dt * dt;

        // The disk lies in y = 0, so a step that changes the sign of y crossed
        // it. The disk is thin and see-through, and a lensed ray can cross it
        // three or four times; the crossings add up, which is what draws the
        // far side of the disk above and below the shadow.
        if (pos.y < 0.0) != (next.y < 0.0) {
            let k = pos.y / (pos.y - next.y);
            color = color + disk(mix(pos, next, k), normalize(vel), u.time, rs, inner);
        }

        vel = vel + acc * dt;
        pos = next;
    }

    if escaped {
        color = color + sky(normalize(vel));
    }

    var c = tonemap(color);
    // Everything above is linear light. egui takes the surface format the
    // browser or the window system offers, and only an sRGB one has the
    // hardware encode on write; for the rest the shader has to do it, or the
    // disk comes out looking like soot.
    if u.srgb_target < 0.5 {
        c = pow(c, vec3<f32>(1.0 / 2.2));
    }
    return vec4<f32>(c, 1.0);
}
