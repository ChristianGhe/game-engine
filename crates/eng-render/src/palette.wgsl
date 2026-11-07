// The first-person palette pass (eng-render; rung 2, T10; plan decision 8;
// ADR-0020), display-referred, after the tone map: each pixel's 8-bit sRGB
// codes index the 32x32x32 table baked on the CPU (eng-render's
// `PaletteLut`), cell `code >> 3` per channel, and the pixel takes the
// palette colour there. With the look's dither, the 4x4 Bayer offset times
// the dither (in codes) is added to each code first. Switched off, the
// pass is bypassed and the tone map draws into the output.
//
// The table is 32,768 sRGB texels, eight layers of the scene's 64 px sRGB
// array from `switches.y` on, cell `i = r + 32 g + 1024 b` at layer
// `i / 4096`, column `i % 64`, row `(i % 4096) / 64`. A load decodes it to
// linear, and the output's sRGB format encodes it back to the same codes.

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

@group(1) @binding(1) var textures_64_srgb: texture_2d_array<f32>;

@group(2) @binding(0) var toned: texture_2d<f32>;

// eng-render's `BAYER_4X4`, row by row (`[y][x]`).
const BAYER_4X4: array<u32, 16> = array<u32, 16>(
    0u, 8u, 2u, 10u,
    12u, 4u, 14u, 6u,
    3u, 11u, 1u, 9u,
    15u, 7u, 13u, 5u,
);

// Linear light as sRGB-encoded values in [0, 1] (IEC 61966-2-1).
fn encode(linear: vec3<f32>) -> vec3<f32> {
    let low = linear * 12.92;
    let high = 1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, linear <= vec3<f32>(0.0031308));
}

@fragment
fn palette_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<u32>(position.xy);
    // `toned` is sRGB, so a load gives linear values: encoding them again
    // and rounding gives back the codes it stores, each well within half a
    // code of a whole one.
    let codes = round(encode(textureLoad(toned, vec2<i32>(p), 0).rgb) * 255.0);
    let m = f32(BAYER_4X4[(p.y % 4u) * 4u + p.x % 4u]);
    let dithered = codes + ((m + 0.5) / 16.0 - 0.5) * frame.look.z;
    let cell = vec3<u32>(clamp(dithered, vec3<f32>(0.0), vec3<f32>(255.0))) >> vec3<u32>(3u);
    let index = cell.r + 32u * cell.g + 1024u * cell.b;
    let texel = vec2<u32>(index % 64u, (index % 4096u) / 64u);
    let layer = frame.switches.y + index / 4096u;
    return vec4<f32>(textureLoad(textures_64_srgb, texel, layer, 0).rgb, 1.0);
}
