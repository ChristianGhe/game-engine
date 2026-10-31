// A test pipeline for eng-gpu's instanced draws: each instance is a
// rectangle given in clip space, four vertices as a triangle strip. Its
// colour keeps the channels whose bits are set in `mask` (red 1, green 2,
// blue 4), so each of the three attribute formats changes the picture.

struct Quad {
    @location(0) centre: vec2<f32>,
    @location(1) half_extents: vec2<f32>,
    @location(2) colour: vec3<f32>,
    @location(3) mask: u32,
}

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) colour: vec3<f32>,
}

@vertex
fn quad_vertex(@builtin(vertex_index) index: u32, quad: Quad) -> Varyings {
    // Corners (-1, -1), (1, -1), (-1, 1), (1, 1).
    let corner = vec2<f32>(f32(index & 1u), f32(index >> 1u)) * 2.0 - 1.0;
    let keep = vec3<f32>(
        f32(quad.mask & 1u),
        f32((quad.mask >> 1u) & 1u),
        f32((quad.mask >> 2u) & 1u),
    );
    var out: Varyings;
    out.position = vec4<f32>(quad.centre + corner * quad.half_extents, 0.0, 1.0);
    out.colour = quad.colour * keep;
    return out;
}

@fragment
fn quad_fragment(in: Varyings) -> @location(0) vec4<f32> {
    return vec4<f32>(in.colour, 1.0);
}
