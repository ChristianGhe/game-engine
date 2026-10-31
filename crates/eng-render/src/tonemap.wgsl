// The tone map (eng-render), the last pass that shapes the image: rung 1b's
// map clamps each channel of the linear, scene-referred scene to [0, 1]
// (plan decision 2), and the output's sRGB format encodes it. The curve and
// EV100 exposure come with lights in rung 2. Alpha is 1: shots are opaque.

@group(0) @binding(0) var input: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;

@fragment
fn tonemap_fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let scene = textureSample(input, input_sampler, uv).rgb;
    return vec4<f32>(clamp(scene, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
