// Test meshes for eng-gpu's scene layout (rung 2, T8; ADR-0020). Each
// vertex is relative to its chunk's origin; its instance record adds the
// chunk's offset from the eye, and the uniform scales the sum into clip
// space, depth included (w is 1). The fragment stages show a flat colour
// per material, fill four colour targets, or sample the material's texture.

struct Frame {
    scale: vec4<f32>,
    colour: vec4<f32>,
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
}

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) material: u32,
}

@vertex
fn mesh_vertex(vertex: Vertex, @builtin(instance_index) index: u32) -> Varyings {
    let instance = instances[index];
    var out: Varyings;
    out.position = vec4<f32>((vertex.position + instance.offset) * frame.scale.xyz, 1.0);
    out.uv = vertex.uv;
    out.material = instance.material;
    return out;
}

// Material 0 is red, 1 green, 2 blue, any other white.
fn flat_colour(material: u32) -> vec4<f32> {
    switch material {
        case 0u: { return vec4<f32>(1.0, 0.0, 0.0, 1.0); }
        case 1u: { return vec4<f32>(0.0, 1.0, 0.0, 1.0); }
        case 2u: { return vec4<f32>(0.0, 0.0, 1.0, 1.0); }
        default: { return vec4<f32>(1.0, 1.0, 1.0, 1.0); }
    }
}

@fragment
fn colour_fragment(in: Varyings) -> @location(0) vec4<f32> {
    return flat_colour(in.material);
}

struct Targets {
    @location(0) a: vec4<f32>,
    @location(1) info: vec4<u32>,
    @location(2) b: vec4<f32>,
    @location(3) c: vec4<f32>,
}

// Four colour targets: red, the integers (7, 9), green, then blue.
@fragment
fn targets_fragment(in: Varyings) -> Targets {
    var out: Targets;
    out.a = vec4<f32>(1.0, 0.0, 0.0, 1.0);
    out.info = vec4<u32>(7u, 9u, 0u, 0u);
    out.b = vec4<f32>(0.0, 1.0, 0.0, 1.0);
    out.c = vec4<f32>(0.0, 0.0, 1.0, 1.0);
    return out;
}

// Without binding arrays, a branch picks the material's array; sampling
// at level 0 needs no uniform control flow.
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

// The material's texture; with its alpha test (flag bit 0), a texel whose
// alpha is below one half is discarded.
@fragment
fn textured_fragment(in: Varyings) -> @location(0) vec4<f32> {
    let material = materials[in.material];
    let colour = sample_material(material, in.uv);
    if (material.flags & 1u) != 0u && colour.a < 0.5 {
        discard;
    }
    return colour;
}
