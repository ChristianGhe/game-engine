// A test copy for eng-gpu's fullscreen passes: each pixel takes the input's
// texel at the same place.

@group(0) @binding(0) var input: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;

@fragment
fn copy_fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(input, input_sampler, uv);
}
