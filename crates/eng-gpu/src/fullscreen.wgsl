// The vertex stage of every fullscreen pass (eng-gpu): one triangle that
// covers the target. `uv` runs from (0, 0) at the target's top left to (1, 1)
// at its bottom right, so each pixel gets the uv of its centre.

struct Fullscreen {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn fullscreen_vertex(@builtin(vertex_index) index: u32) -> Fullscreen {
    // Vertices 0, 1 and 2 at uv (0, 0), (2, 0) and (0, 2).
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: Fullscreen;
    out.position = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    out.uv = uv;
    return out;
}
