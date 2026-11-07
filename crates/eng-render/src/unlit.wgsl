// The first-person light pass as rung 2's T9 leaves it (eng-render;
// ADR-0020): no light model yet, so `lit` is `albedo` as it is, one unit of
// luminance per unit of albedo. T10 replaces it with region illuminance in
// lux, Lambertian albedo, falloff, banding and fake contrast. The pass
// already declares the light pass's reads, albedo, `info` and depth, so
// its place in the graph does not move; this stage reads only albedo.

@group(2) @binding(0) var albedo: texture_2d<f32>;

@fragment
fn unlit_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(textureLoad(albedo, vec2<i32>(position.xy), 0).rgb, 1.0);
}
