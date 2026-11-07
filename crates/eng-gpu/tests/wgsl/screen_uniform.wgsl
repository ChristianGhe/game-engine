// A test screen pass for eng-gpu (rung 2, T8): every pixel takes the
// colour in the frame's uniform.

struct Frame {
    scale: vec4<f32>,
    colour: vec4<f32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;

@fragment
fn uniform_fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    return frame.colour;
}
