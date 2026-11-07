// The first-person tone map (eng-render; rung 2, T10; plan decision 7,
// O10; ADR-0020), the last scene-referred pass: the luminance in nits is
// scaled by the camera's exposure (from EV100, in the uniform), then
// mapped by the Khronos PBR Neutral tone mapper into [0, 1], which the
// 8-bit sRGB target encodes. PBR Neutral keeps base colours as they are up
// to its compression start, which suits the palette after it.
//
// The curve, after Khronos's definition (github.com/KhronosGroup/
// ToneMapping, PBR_Neutral): with F90 = 0.04, Ks = 0.8 - F90, Kd = 0.15,
// x the least channel, the offset f is x - x^2 / (4 F90) for x <= 2 F90
// and F90 above; the colour minus f is kept if its peak p is at most Ks;
// above, the peak is compressed to p' = 1 - (1 - Ks)^2 / (p + 1 - 2 Ks)
// and the colour, scaled by p' / p, is desaturated toward p' by
// 1 - 1 / (Kd (p - p') + 1).

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

// `fogged`, or `lit` in the plain pipeline and with fog bypassed.
@group(2) @binding(0) var scene: texture_2d<f32>;

const F90: f32 = 0.04;
const START_COMPRESSION: f32 = 0.8 - F90;
const DESATURATION: f32 = 0.15;

fn pbr_neutral(colour: vec3<f32>) -> vec3<f32> {
    let x = min(colour.r, min(colour.g, colour.b));
    var offset = F90;
    if x < 2.0 * F90 {
        offset = x - x * x / (4.0 * F90);
    }
    let c = colour - offset;
    let peak = max(c.r, max(c.g, c.b));
    if peak < START_COMPRESSION {
        return c;
    }
    let d = 1.0 - START_COMPRESSION;
    let new_peak = 1.0 - d * d / (peak + d - START_COMPRESSION);
    let g = 1.0 - 1.0 / (DESATURATION * (peak - new_peak) + 1.0);
    return mix(c * (new_peak / peak), vec3<f32>(new_peak), g);
}

@fragment
fn tonemap_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let luminance = textureLoad(scene, vec2<i32>(position.xy), 0).rgb;
    // The light passes draw no negative light; the curve takes none.
    let exposed = max(luminance * frame.look.x, vec3<f32>(0.0));
    return vec4<f32>(pbr_neutral(exposed), 1.0);
}
