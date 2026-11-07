// The first-person light pass (eng-render; rung 2, T10; ADR-0020): each
// pixel's albedo lit by its region's light, as luminance in nits. The
// light step starts at the region's Doom light level over 8; fake contrast
// adds to walls facing +-X and takes from walls facing +-Y; falloff takes
// steps per meter of the distance from the eye; then the step is clamped
// to 0 to 31 and, with banding, rounded down to a whole step. Its lux is
// the uniform's table, interpolated between whole steps, and a Lambertian
// surface shows albedo times lux over pi (eng-render's `first_person/look.rs`).
// The sky pass then draws over the pixels where nothing was drawn.

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

@group(2) @binding(0) var albedo: texture_2d<f32>;
@group(2) @binding(1) var info: texture_2d<u32>;
@group(2) @binding(2) var depth: texture_depth_2d;

const PI: f32 = 3.141592653589793;
// `switches.x & 1`: light comes in whole steps.
const BANDING: u32 = 1u;
// Surface class codes (eng-render's `SurfaceClass::code`).
const WALL_FACING_X: u32 = 4u;
const WALL_FACING_Y: u32 = 5u;
// The last region the uniform holds a look for.
const LAST_REGION: u32 = 15u;
const LAST_STEP: f32 = 31.0;

// The illuminance of light step `step`, 0 to 31, in lux: the table's at a
// whole step, linearly between two.
fn step_lux(step: f32) -> f32 {
    let below = u32(step);
    let above = min(below + 1u, 31u);
    let low = frame.light_steps[below / 4u][below % 4u];
    let high = frame.light_steps[above / 4u][above % 4u];
    return low + (high - low) * (step - f32(below));
}

// How far from the eye the pixel at `uv` shows something at depth `d`
// (`near / distance ahead`): along the ray through its centre, whose part
// along the forward axis is 1.
fn distance(uv: vec2<f32>, d: f32) -> f32 {
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let ray = frame.forward.xyz + ndc.x * frame.ray_right.xyz + ndc.y * frame.ray_up.xyz;
    return frame.forward.w / d * length(ray);
}

fn light(p: vec2<i32>, uv: vec2<f32>, falloff: f32, contrast: f32, banding: bool) -> vec4<f32> {
    let colour = textureLoad(albedo, p, 0).rgb;
    let words = textureLoad(info, p, 0).r;
    let region = min(words & 0xffffu, LAST_REGION);
    let surface = (words >> 16u) & 0xffu;
    var step = frame.region_light[region / 4u][region % 4u];
    if surface == WALL_FACING_X {
        step += contrast;
    } else if surface == WALL_FACING_Y {
        step -= contrast;
    }
    let d = textureLoad(depth, p, 0);
    // Depth 0 is the sky's, which the sky pass draws over.
    if d > 0.0 {
        step -= falloff * distance(uv, d);
    }
    step = clamp(step, 0.0, LAST_STEP);
    if banding {
        step = floor(step);
    }
    return vec4<f32>(colour * (step_lux(step) / PI), 1.0);
}

@fragment
fn light_fragment(
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    let banding = (frame.switches.x & BANDING) != 0u;
    return light(vec2<i32>(position.xy), uv, frame.look.y, frame.look.w, banding);
}

// The plain pipeline's light: the region's light alone, no falloff,
// contrast or banding.
@fragment
fn plain_light_fragment(
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    return light(vec2<i32>(position.xy), uv, 0.0, 0.0, false);
}
