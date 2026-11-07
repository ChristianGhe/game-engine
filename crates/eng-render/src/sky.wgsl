// The first-person sky pass (eng-render; rung 2, T9 and T10; ADR-0020). It
// continues the light pass, drawing into `lit` where nothing was drawn:
// depth is reversed and cleared to 0, the far plane, which no geometry
// reaches with an infinite far plane, so a depth of exactly 0 means sky.
// The colour runs from the horizon's to the zenith's with the sine of the
// view ray's elevation, the ray rebuilt from the uniform as eng-render's
// `ViewProj::ray` builds it; with the look's sky gradient off, and in the
// plain pipeline, it is the horizon's everywhere.

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

@group(2) @binding(0) var depth: texture_depth_2d;

// `switches.x & 2`: the sky runs from horizon to zenith.
const SKY_GRADIENT: u32 = 2u;

// What the sky shows along the ray through the pixel at `uv`.
fn sky(uv: vec2<f32>, gradient: bool) -> vec4<f32> {
    if !gradient {
        return vec4<f32>(frame.sky_horizon.rgb, 1.0);
    }
    // Normalized device coordinates of the pixel's centre: y up.
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let ray = frame.forward.xyz + ndc.x * frame.ray_right.xyz + ndc.y * frame.ray_up.xyz;
    let rise = clamp(ray.z / length(ray), 0.0, 1.0);
    return vec4<f32>(mix(frame.sky_horizon.rgb, frame.sky_zenith.rgb, rise), 1.0);
}

@fragment
fn sky_fragment(
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    if textureLoad(depth, vec2<i32>(position.xy), 0) != 0.0 {
        discard;
    }
    return sky(uv, (frame.switches.x & SKY_GRADIENT) != 0u);
}

// The plain pipeline's sky: flat.
@fragment
fn plain_sky_fragment(
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    if textureLoad(depth, vec2<i32>(position.xy), 0) != 0.0 {
        discard;
    }
    return sky(uv, false);
}
