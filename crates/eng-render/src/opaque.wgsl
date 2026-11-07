// The first-person opaque pass (eng-render; rung 2, T9; ADR-0020): the
// world's chunks from eng-gpu's megabuffers. Each vertex is relative to its
// chunk's origin, and the chunk's instance record holds that origin minus
// the eye, subtracted in f64 on the CPU, so their sum is the vertex's offset
// from the eye, small wherever the world is. `clip_from_eye` projects it:
// reversed-Z with an infinite far plane, depth `near / distance ahead`.
// The pass writes the material's colour into `albedo` and the vertex's
// `info` words (region and class, then id) into `info`.

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

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) material: u32,
    // Region in bits 0-15, class in bits 16-23.
    @location(3) region_class: u32,
    @location(4) id: u32,
}

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) material: u32,
    @location(2) @interpolate(flat) info: vec2<u32>,
}

@vertex
fn opaque_vertex(vertex: Vertex, @builtin(instance_index) index: u32) -> Varyings {
    let from_eye = vertex.position + instances[index].offset;
    var out: Varyings;
    out.position = frame.clip_from_eye * vec4<f32>(from_eye, 1.0);
    out.uv = vertex.uv;
    out.material = vertex.material;
    out.info = vec2<u32>(vertex.region_class, vertex.id);
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
// whose alpha is below one half is discarded.
@fragment
fn opaque_fragment(in: Varyings) -> Targets {
    let material = materials[in.material];
    let colour = sample_material(material, in.uv);
    if (material.flags & 1u) != 0u && colour.a < 0.5 {
        discard;
    }
    var out: Targets;
    out.albedo = vec4<f32>(colour.rgb, 1.0);
    out.info = vec4<u32>(in.info, 0u, 0u);
    return out;
}
