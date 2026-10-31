// The present blit (eng-render): draws the output into the surface's
// format, texel for texel, rather than copying it, because a swapchain image
// cannot be relied on for copies (spec 6.7). Both are sRGB-encoded, so each
// texel decodes on the way in and encodes on the way out to the same bytes.

@group(0) @binding(0) var input: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;

@fragment
fn present_fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(input, input_sampler, uv);
}
