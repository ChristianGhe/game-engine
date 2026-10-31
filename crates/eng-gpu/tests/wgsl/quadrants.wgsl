// A test pattern for eng-gpu's fullscreen passes: red on the left half,
// green on the top half and blue on alternate pixels, so a readback shows
// whether each row and column lands where it should, to the pixel. It
// ignores its input.

@fragment
fn quadrants_fragment(
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    let red = select(0.0, 1.0, uv.x < 0.5);
    let green = select(0.0, 1.0, uv.y < 0.5);
    let pixel = vec2<u32>(position.xy);
    let blue = select(0.0, 1.0, ((pixel.x + pixel.y) & 1u) == 1u);
    return vec4<f32>(red, green, blue, 1.0);
}
