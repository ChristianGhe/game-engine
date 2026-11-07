// The first-person sprites pass (eng-render; rung 2, T11; ADR-0020): each
// thing a billboard, drawn on top of `opaque`'s albedo, `info` and depth.
// One draw over the world's six shared billboard indices, one instance per
// thing; the corners come from the vertex index, and the thing from two of
// the frame's records (eng-render's `first_person/sprites.rs`): the first
// holds its feet minus the eye, taken in f64, and the material of the frame
// shown; the second its width (negative to mirror), height and region as
// `offset`, and its id as `material`. The quad stands upright on the feet
// along the camera's right axis, which is level, so it turns about Z only.
// The material's alpha test discards; what is left writes the texel's
// colour, `info` (region, the class Sprite, the id) and its depth.

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

struct Instance {
    offset: vec3<f32>,
    material: u32,
}

struct Material {
    bucket: u32,
    tag: u32,
    layer: u32,
    flags: u32,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> instances: array<Instance>;

@group(1) @binding(0) var<storage, read> materials: array<Material>;
@group(1) @binding(1) var textures_64_srgb: texture_2d_array<f32>;
@group(1) @binding(2) var textures_64_linear: texture_2d_array<f32>;
@group(1) @binding(3) var textures_128_srgb: texture_2d_array<f32>;
@group(1) @binding(4) var textures_128_linear: texture_2d_array<f32>;
@group(1) @binding(5) var textures_256_srgb: texture_2d_array<f32>;
@group(1) @binding(6) var textures_256_linear: texture_2d_array<f32>;
@group(1) @binding(7) var nearest: sampler;

// Surface class code of a sprite (eng-render's `SurfaceClass::code`).
const SPRITE: u32 = 6u;
// The largest region `info` holds.
const REGION_MASK: u32 = 0xffffu;

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) material: u32,
    @location(2) @interpolate(flat) info: vec2<u32>,
}

// Corners 0 to 3: bottom left, bottom right, top right, top left, as the
// world's billboard indices take them.
@vertex
fn sprite_vertex(
    @builtin(vertex_index) corner: u32,
    @builtin(instance_index) thing: u32,
) -> Varyings {
    let place = instances[2u * thing];
    let shape = instances[2u * thing + 1u];
    let across = select(-0.5, 0.5, corner == 1u || corner == 2u);
    let up = select(0.0, 1.0, corner >= 2u);
    // Level whatever the camera does: the right axis with its z dropped, so
    // a billboard turns about Z only even if a camera ever rolls.
    let right = normalize(vec3<f32>(frame.ray_right.xy, 0.0));
    let from_eye = place.offset + right * (across * shape.offset.x)
        + vec3<f32>(0.0, 0.0, up * shape.offset.y);
    var out: Varyings;
    out.position = frame.clip_from_eye * vec4<f32>(from_eye, 1.0);
    out.uv = vec2<f32>(across + 0.5, 1.0 - up);
    out.material = place.material;
    let region = min(u32(shape.offset.z), REGION_MASK);
    out.info = vec2<u32>(region | (SPRITE << 16u), shape.material);
    return out;
}

// Without binding arrays, a branch picks the material's array; sampling at
// level 0 needs no uniform control flow.
fn sample_material(material: Material, uv: vec2<f32>) -> vec4<f32> {
    let layer = material.layer;
    switch material.bucket * 2u + material.tag {
        case 0u: { return textureSampleLevel(textures_64_srgb, nearest, uv, layer, 0.0); }
        case 1u: { return textureSampleLevel(textures_64_linear, nearest, uv, layer, 0.0); }
        case 2u: { return textureSampleLevel(textures_128_srgb, nearest, uv, layer, 0.0); }
        case 3u: { return textureSampleLevel(textures_128_linear, nearest, uv, layer, 0.0); }
        case 4u: { return textureSampleLevel(textures_256_srgb, nearest, uv, layer, 0.0); }
        default: { return textureSampleLevel(textures_256_linear, nearest, uv, layer, 0.0); }
    }
}

struct Targets {
    @location(0) albedo: vec4<f32>,
    @location(1) info: vec4<u32>,
}

// The material's texel, opaque; with its alpha test (flag bit 0), a texel
// whose alpha is below one half is discarded, and writes no depth.
@fragment
fn sprite_fragment(in: Varyings) -> Targets {
    let material = materials[in.material];
    // The sampler repeats, so a coordinate on or past an edge would wrap
    // to the opposite one. Keep it half a texel inside the texture, not one
    // ulp: Vulkan allows as few as 4 sub-texel bits, so a coordinate an ulp
    // below 1 may round onto the edge and wrap to the top row there.
    let side = f32(64u << material.bucket);
    let uv = clamp(in.uv, vec2<f32>(0.5 / side), vec2<f32>(1.0 - 0.5 / side));
    let colour = sample_material(material, uv);
    if (material.flags & 1u) != 0u && colour.a < 0.5 {
        discard;
    }
    var out: Targets;
    out.albedo = vec4<f32>(colour.rgb, 1.0);
    out.info = vec4<u32>(in.info, 0u, 0u);
    return out;
}
