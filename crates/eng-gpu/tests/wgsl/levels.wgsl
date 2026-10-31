// A test pattern for eng-gpu's surface: column x holds sRGB level x in red,
// 255 - x in green and 3x (wrapping) in blue, written as the linear values
// that encode to those levels, so an sRGB target stores every level from 0
// to 255. It ignores its input.

fn linear(level: u32) -> f32 {
    let encoded = f32(level & 255u) / 255.0;
    if encoded <= 0.04045 {
        return encoded / 12.92;
    }
    return pow((encoded + 0.055) / 1.055, 2.4);
}

@fragment
fn levels_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let x = u32(position.x);
    return vec4<f32>(linear(x), linear(255u - x), linear(3u * x), 1.0);
}
