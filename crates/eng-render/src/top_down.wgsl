// The top-down pass (eng-render): each instance is one shape, a quad in clip
// space drawn as a four-vertex triangle strip. A disc keeps only the
// ellipse inside its quad, which is a circle in square pixels. Colours are
// linear and scene-referred; the tone map comes later.

struct Shape {
    @location(0) centre: vec2<f32>,
    @location(1) half_extents: vec2<f32>,
    @location(2) colour: vec3<f32>,
    // 1 for a disc, 0 for a rectangle.
    @location(3) disc: u32,
}

struct Varyings {
    @builtin(position) position: vec4<f32>,
    // Where the fragment lies in its quad, from (-1, -1) to (1, 1).
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) colour: vec3<f32>,
    @location(2) @interpolate(flat) disc: u32,
}

@vertex
fn top_down_vertex(@builtin(vertex_index) index: u32, shape: Shape) -> Varyings {
    // Corners (-1, -1), (1, -1), (-1, 1), (1, 1).
    let corner = vec2<f32>(f32(index & 1u), f32(index >> 1u)) * 2.0 - 1.0;
    var out: Varyings;
    out.position = vec4<f32>(shape.centre + corner * shape.half_extents, 0.0, 1.0);
    out.local = corner;
    out.colour = shape.colour;
    out.disc = shape.disc;
    return out;
}

@fragment
fn top_down_fragment(in: Varyings) -> @location(0) vec4<f32> {
    if in.disc != 0u && dot(in.local, in.local) > 1.0 {
        discard;
    }
    return vec4<f32>(in.colour, 1.0);
}
