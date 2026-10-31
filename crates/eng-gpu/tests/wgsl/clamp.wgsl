// A test tone map for eng-gpu's fullscreen passes: rung 1b's clamp to [0, 1]
// (plan decision 2). The sRGB output target does the encoding.

@group(0) @binding(0) var input: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;

@fragment
fn clamp_fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let colour = textureSample(input, input_sampler, uv);
    return clamp(colour, vec4<f32>(0.0), vec4<f32>(1.0));
}
