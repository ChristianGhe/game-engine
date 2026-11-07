// The first-person fog pass (eng-render; rung 2, T10; ADR-0020): each
// drawn pixel moves toward its region's fog colour with its distance from
// the eye, `1 - e^(-density * distance)` of the way, so further is always
// foggier. The sky (depth 0) keeps its colour. Switched off, the pass is
// bypassed and the tone map reads `lit`.

// The per-frame uniform; every first-person shader declares it alike
// (eng-render's `first_person/frame.rs`).
struct Frame {
    clip_from_eye: mat4x4<f32>,
    forward: vec4<f32>,
    ray_right: vec4<f32>,
    ray_up: vec4<f32>,
    sky_horizon: vec4<f32>,
    sky_zenith: vec4<f32>,
    look: vec4<f32>,
    switches: vec4<u32>,
    light_steps: array<vec4<f32>, 8>,
    region_light: array<vec4<f32>, 4>,
    region_fog: array<vec4<f32>, 16>,
}

@group(0) @binding(0) var<uniform> frame: Frame;

@group(2) @binding(0) var lit: texture_2d<f32>;
@group(2) @binding(1) var info: texture_2d<u32>;
@group(2) @binding(2) var depth: texture_depth_2d;

// The last region the uniform holds a look for.
const LAST_REGION: u32 = 15u;

// As the light pass measures it: along the ray through the pixel's centre.
fn distance(uv: vec2<f32>, d: f32) -> f32 {
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let ray = frame.forward.xyz + ndc.x * frame.ray_right.xyz + ndc.y * frame.ray_up.xyz;
    return frame.forward.w / d * length(ray);
}

@fragment
fn fog_fragment(
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    let p = vec2<i32>(position.xy);
    let colour = textureLoad(lit, p, 0);
    let d = textureLoad(depth, p, 0);
    if d == 0.0 {
        return colour;
    }
    let region = min(textureLoad(info, p, 0).r & 0xffffu, LAST_REGION);
    let fog = frame.region_fog[region];
    let amount = 1.0 - exp(-fog.a * distance(uv, d));
    return vec4<f32>(mix(colour.rgb, fog.rgb, amount), 1.0);
}
