// A test screen pass for eng-gpu (rung 2, T8): it reads what
// `mesh.wgsl`'s `targets_fragment` drew into four colour targets and a
// depth target, one read of each kind. Red, green and blue are the first,
// third and fourth targets' own; alpha is 1 where the integer target and
// the depth hold what was drawn there (7, 9 and 0.5 on the quad, 0 and 0
// around it, the clear values), else 0.

@group(2) @binding(0) var a: texture_2d<f32>;
@group(2) @binding(1) var info: texture_2d<u32>;
@group(2) @binding(2) var b: texture_2d<f32>;
@group(2) @binding(3) var c: texture_2d<f32>;
@group(2) @binding(4) var depth: texture_depth_2d;
@group(2) @binding(5) var reads_sampler: sampler;

@fragment
fn verify_fragment(
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    let texel = vec2<i32>(position.xy);
    let inside = textureLoad(a, texel, 0).r > 0.5;
    let ids = textureLoad(info, texel, 0).xy;
    let z = textureLoad(depth, texel, 0);
    var held = all(ids == vec2<u32>(0u, 0u)) && z == 0.0;
    if inside {
        held = all(ids == vec2<u32>(7u, 9u)) && abs(z - 0.5) < 0.000001;
    }
    // The nearest-texel sampler gives what the load does.
    let sampled = textureSampleLevel(b, reads_sampler, uv, 0.0);
    return vec4<f32>(
        textureLoad(a, texel, 0).r,
        sampled.g,
        textureLoad(c, texel, 0).b,
        select(0.0, 1.0, held),
    );
}
