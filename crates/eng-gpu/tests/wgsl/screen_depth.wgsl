// A test screen pass for eng-gpu (rung 2, T8): it reads a colour target and
// a depth target, and shows the colour with the depth as its alpha.

@group(2) @binding(0) var albedo: texture_2d<f32>;
@group(2) @binding(1) var depth: texture_depth_2d;

@fragment
fn albedo_depth_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let texel = vec2<i32>(position.xy);
    let colour = textureLoad(albedo, texel, 0);
    return vec4<f32>(colour.rgb, textureLoad(depth, texel, 0));
}
