//! The first-person `opaque` and `sky` passes on a real adapter (rung 2,
//! task T9: "a known room renders with depth probes as computed on the
//! CPU"). A room of floor, walls and a step, open to the sky, a billion
//! meters from the origin, is drawn; then every pixel that a CPU ray cast
//! in f64 calls unambiguous is checked: its depth against `near / distance
//! ahead`, its `info` against the face's, its colour against the face's
//! texture or the sky's, lit by its region's light, exposed and tone mapped
//! as the CPU reference does it (`tone/mod.rs`; T10), with every look
//! feature off. Without an adapter each test skips, unless
//! `ENG_REQUIRE_GPU=1` (CI) makes it fail.
//!
//! **The depth tolerance** at a pixel is `1e-5 * depth`, plus how much the
//! CPU's depth changes over 1/16 of a pixel across and down:
//! - The GPU snaps each triangle's corners to its sub-pixel grid before it
//!   interpolates depth, which moves the plane by at most the depth's
//!   gradient times the snap. Vulkan guarantees at least 4 sub-pixel bits
//!   (D3D12 requires 8), so the snap is at most 1/32 pixel; 1/16 doubles
//!   it.
//! - The rest is f32: the chunk's offset rounded once, the vertex plus it,
//!   the matrix product (which a GPU may fuse) and the interpolation, each
//!   a few units in 2^-24 relative; 1e-5 is about 170 of them.
//!
//! So a wall seen head on, whose depth is flat, is checked to 1e-5
//! relative, and the floor near the horizon as loosely as its slope needs.
//! On an M2 Pro (Metal) the worst pixel used 3% of its tolerance.

mod room;
mod tone;

use eng_gpu::{Gpu, GpuError, GpuOptions, Image, Targets};
use eng_math::{BAngle, WorldPos};
use eng_render::{
    Chunk, FirstPersonCamera, FirstPersonRenderer, FirstPersonScene, FrameCounters, GpuWorld, Info,
    NEAR_PLANE, SurfaceClass, Texels, WorldMesh,
};
use room::{AROUND, Camera64, Seen, depth_tolerance, seen};
use tone::within_one;

fn gpu() -> Option<Gpu> {
    match Gpu::new(&GpuOptions::from_env()) {
        Ok(gpu) => Some(gpu),
        Err(e @ GpuError::NoAdapter { .. }) if !eng_gpu::gpu_required() => {
            eprintln!("skipped: {e}");
            None
        }
        Err(e) => panic!("{e}"),
    }
}

/// The shot size (rung 1b, decision 3).
const W: u32 = 320;
const H: u32 = 240;

/// The room's corner, far from the origin: f32 steps by 64 m and more
/// there, so only offsets taken in f64 draw it in place.
const FAR: [f64; 3] = [1.0e9, -2.0e9, 250.0];

/// What one frame left: the output, depth and `info`, and its counters.
struct Frame {
    image: Image,
    depth: Texels,
    info: Texels,
    counters: FrameCounters,
}

/// Loads `chunks` with the room's textures and materials.
fn load(gpu: &Gpu, chunks: &[Chunk]) -> GpuWorld {
    let texels = room::texels();
    let textures = room::textures(&texels);
    let materials = room::materials();
    GpuWorld::load(
        gpu,
        &WorldMesh {
            label: "room",
            chunks,
            materials: &materials,
            textures: &textures,
        },
    )
    .unwrap_or_else(|e| panic!("{e}"))
}

fn render(
    gpu: &Gpu,
    renderer: &FirstPersonRenderer,
    targets: &Targets,
    world: &GpuWorld,
    scene: &FirstPersonScene,
) -> Frame {
    let counters = renderer
        .render(gpu, targets, world, scene)
        .unwrap_or_else(|e| panic!("{e}"));
    Frame {
        image: targets.read_output(gpu).unwrap_or_else(|e| panic!("{e}")),
        depth: renderer
            .read_depth(gpu, targets)
            .unwrap_or_else(|e| panic!("{e}")),
        info: renderer
            .read_info(gpu, targets)
            .unwrap_or_else(|e| panic!("{e}")),
        counters,
    }
}

#[test]
fn a_known_room_renders_with_depth_info_and_sky_as_the_cpu_computes_them() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let world = load(&gpu, &room::world_chunks(FAR));
    assert_eq!(world.drawn_chunks(), 2);
    let targets = Targets::new(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    let scene = room::scene(FAR);
    let frame = render(&gpu, &renderer, &targets, &world, &scene);

    let faces = room::faces();
    let camera = Camera64::new(&scene.camera, room::EYE, W, H);
    let mut failures = Vec::new();
    let mut seen_ids = std::collections::BTreeMap::<u32, usize>::new();
    let (mut sky, mut edges, mut depth_probes, mut worst) = (0, 0, 0, 0.0f64);
    for y in 0..H {
        for x in 0..W {
            let (cx, cy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let ray = camera.ray(cx, cy);
            let depth = f64::from(frame.depth.depth(x, y));
            let info = Info::unpack(frame.info.uint2(x, y));
            let pixel = frame.image.pixel(x, y);
            match seen(&faces, &camera, x, y) {
                Seen::Edge => edges += 1,
                Seen::Sky => {
                    sky += 1;
                    // The look's sky is flat: the horizon's colour.
                    let horizon = room::SKY.horizon;
                    let nits = [horizon.r, horizon.g, horizon.b].map(f64::from);
                    let want = tone::toned_codes(nits, room::EV100);
                    if depth != 0.0 || info != Some(Info::NONE) || !within_one(pixel, want) {
                        failures.push(format!(
                            "({x}, {y}) should be sky {want:.1?}: depth {depth}, info {info:?}, \
                             colour {pixel:?}"
                        ));
                    }
                }
                Seen::Face(index, t) => {
                    let face = &faces[index];
                    *seen_ids.entry(face.info.id).or_default() += 1;
                    depth_probes += 1;
                    let want = camera.near / t;
                    let tolerance = depth_tolerance(face, &camera, cx, cy);
                    worst = eng_math::max_f64(worst, (depth - want).abs() / tolerance);
                    if (depth - want).abs() > tolerance {
                        failures.push(format!(
                            "({x}, {y}) on face {}: depth {depth}, CPU {want} (tolerance \
                             {tolerance:e})",
                            face.info.id
                        ));
                    }
                    if info != Some(face.info) {
                        failures.push(format!(
                            "({x}, {y}): info {info:?}, the CPU says {:?}",
                            face.info
                        ));
                    }
                    // The floor's colour only where its half is the same
                    // all around the pixel's centre.
                    let colour_known = face.material != 0
                        || AROUND.iter().all(|(dx, dy)| {
                            let around = camera.ray(cx + dx, cy + dy);
                            let ta = room::plane_t(face, camera.eye, around).unwrap_or(t);
                            room::floor_half(camera.eye, around, ta)
                                == room::floor_half(camera.eye, ray, t)
                        });
                    let albedo = room::face_colour(face, camera.eye, ray, t);
                    let lux = room::region_lux(face.info.region);
                    let want = tone::toned_codes(tone::lambert(albedo, lux), room::EV100);
                    if colour_known && !within_one(pixel, want) {
                        failures.push(format!(
                            "({x}, {y}) on face {}: colour {pixel:?}, texture {want:?}",
                            face.info.id
                        ));
                    }
                }
            }
        }
    }
    let shown: Vec<String> = failures.iter().take(12).cloned().collect();
    assert!(
        failures.is_empty(),
        "{} differences from the CPU over {} pixels (depth, info and colour each \
         count), first:\n{}",
        failures.len(),
        W * H,
        shown.join("\n")
    );
    eprintln!(
        "{depth_probes} depth probes, {sky} sky pixels, {edges} edges; worst depth error {:.3} \
         of its tolerance; pixels per face {seen_ids:?}",
        worst
    );
    // The view is the one meant: the floor, the far and both side walls,
    // the step's top, front and +Y side, and sky.
    for id in [1, 2, 4, 5, 10, 11, 14] {
        assert!(
            seen_ids.get(&id).copied().unwrap_or(0) >= 30,
            "face {id} shows on too few pixels: {seen_ids:?}"
        );
    }
    // No pixel at all, edges included, shows a face from behind: the
    // pipeline culls back faces.
    for y in 0..H {
        for x in 0..W {
            let id = frame.info.uint2(x, y)[1];
            assert!(
                !room::FACING_AWAY.contains(&id),
                "({x}, {y}) shows face {id} from behind"
            );
        }
    }
    assert!(sky >= 5_000, "only {sky} sky pixels");
    assert!(edges < W * H / 20, "{edges} edge pixels");
    // Two chunk draws, then light, sky and the tone map, fog and the
    // palette bypassed: four pipelines; opaque binds the frame and the
    // scene, the others the frame and their reads; the 624-byte uniform
    // and two records.
    let counters = frame.counters;
    assert_eq!(
        (
            counters.draws,
            counters.pipeline_switches,
            counters.bind_group_switches,
            counters.upload_bytes
        ),
        (5, 4, 8, 624 + 2 * 16)
    );
}

#[test]
fn the_room_a_billion_meters_out_renders_as_it_does_at_the_origin() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let targets = Targets::new(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    let frame_at = |corner: [f64; 3]| {
        let world = load(&gpu, &room::world_chunks(corner));
        render(&gpu, &renderer, &targets, &world, &room::scene(corner))
    };
    let (near, far) = (frame_at([0.0; 3]), frame_at(FAR));
    // Every coordinate is a multiple of 1/8, so each offset from the eye
    // is the same f64 out there, and the GPU gets the same bits.
    assert!(near.image == far.image, "the output differs");
    assert!(near.depth == far.depth, "the depth differs");
    assert!(near.info == far.info, "info differs");
}

#[test]
fn with_a_surface_the_frame_is_presented_too() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let world = load(&gpu, &room::world_chunks(FAR));
    let targets = Targets::with_offscreen_surface(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    let frame = render(&gpu, &renderer, &targets, &world, &room::scene(FAR));
    let surface = targets.read_surface(&gpu).unwrap_or_else(|e| panic!("{e}"));
    assert!(surface == frame.image, "the blit changed the output");
    let offscreen = Targets::new(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    let alone = render(&gpu, &renderer, &offscreen, &world, &room::scene(FAR));
    assert!(alone.image == frame.image && alone.depth == frame.depth && alone.info == frame.info);
    let counters = frame.counters;
    assert_eq!(
        (
            counters.draws,
            counters.pipeline_switches,
            counters.bind_group_switches
        ),
        (6, 5, 9)
    );
}

/// 80°, mouse look's pitch limit (plan decision 9), in binary angle.
const PITCH_LIMIT: u32 = 954_437_176;

/// A closed 2 m box, its ceiling a quarter meter above the eye, seen from a
/// quarter meter off two walls, as near as the 0.5 m hull lets an eye come;
/// how many pixels show nothing, over yaws every 30° and pitches from -80°
/// to 80°, through a 90° vertical field of view at 16:9 (121° across).
fn holes_in_the_corner(gpu: &Gpu, renderer: &FirstPersonRenderer, near: f32) -> usize {
    use SurfaceClass::{Ceiling, Floor, WallFacingX, WallFacingY};
    let ceiling = 1.6 + 0.25;
    let walls = vec![
        room::face(
            2,
            0.0,
            [(0.0, 2.0), (0.0, 2.0)],
            true,
            1,
            room::info(0, Floor, 1),
        ),
        room::face(
            2,
            ceiling,
            [(0.0, 2.0), (0.0, 2.0)],
            false,
            1,
            room::info(0, Ceiling, 2),
        ),
        room::face(
            0,
            0.0,
            [(0.0, 2.0), (0.0, ceiling)],
            true,
            2,
            room::info(0, WallFacingX, 3),
        ),
        room::face(
            0,
            2.0,
            [(0.0, 2.0), (0.0, ceiling)],
            false,
            2,
            room::info(0, WallFacingX, 4),
        ),
        room::face(
            1,
            0.0,
            [(0.0, ceiling), (0.0, 2.0)],
            true,
            3,
            room::info(0, WallFacingY, 5),
        ),
        room::face(
            1,
            2.0,
            [(0.0, ceiling), (0.0, 2.0)],
            false,
            3,
            room::info(0, WallFacingY, 6),
        ),
    ];
    let world = load(gpu, &room::build([0.0; 3], &[([1.0, 1.0, 1.0], walls)]));
    let targets = Targets::new(gpu, 160, 90).unwrap_or_else(|e| panic!("{e}"));
    let pitches = [
        0u32.wrapping_sub(PITCH_LIMIT),
        0u32.wrapping_sub(PITCH_LIMIT / 2),
        0,
        PITCH_LIMIT / 2,
        PITCH_LIMIT,
    ];
    let mut holes = 0;
    for turn in 0..12u32 {
        for pitch in pitches {
            let scene = FirstPersonScene {
                camera: FirstPersonCamera {
                    eye: WorldPos::new(0.25, 0.25, 1.6),
                    yaw: BAngle(turn * (u32::MAX / 12 + 1)),
                    pitch: BAngle(pitch),
                    fov_y: BAngle::QUARTER,
                    near,
                },
                ..room::scene([0.0; 3])
            };
            renderer
                .render(gpu, &targets, &world, &scene)
                .unwrap_or_else(|e| panic!("{e}"));
            let info = renderer
                .read_info(gpu, &targets)
                .unwrap_or_else(|e| panic!("{e}"));
            holes += (0..90)
                .flat_map(|y| (0..160).map(move |x| (x, y)))
                .filter(|&(x, y)| info.uint2(x, y) == Info::NONE.pack())
                .count();
        }
    }
    holes
}

#[test]
fn the_near_plane_opens_no_wall_as_close_as_the_hull_comes() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    // The near rectangle's farthest corner: 0.1 * sqrt(1 + (16/9)^2 + 1).
    let corner = f64::from(NEAR_PLANE) * eng_math::sqrt_f64(2.0 + (16.0 / 9.0) * (16.0 / 9.0));
    assert!(corner < 0.25, "{corner}");
    assert_eq!(holes_in_the_corner(&gpu, &renderer, NEAR_PLANE), 0);
    // The check can fail: a near plane beyond a quarter meter cuts the
    // walls open.
    assert!(holes_in_the_corner(&gpu, &renderer, 0.3) > 0);
}

#[test]
fn a_world_or_scene_that_cannot_be_drawn_is_refused() {
    let Some(gpu) = gpu() else { return };
    let texels = room::texels();
    let textures = room::textures(&texels);
    let materials = room::materials();
    let refusal = |chunks: &[Chunk]| {
        GpuWorld::load(
            &gpu,
            &WorldMesh {
                label: "bad",
                chunks,
                materials: &materials,
                textures: &textures,
            },
        )
        .expect_err("the world should be refused")
        .to_string()
    };
    let good = room::world_chunks([0.0; 3]);
    let with = |change: &dyn Fn(&mut Chunk)| {
        let mut chunks = good.clone();
        change(&mut chunks[0]);
        refusal(&chunks)
    };
    assert_eq!(
        with(&|c| c.indices[4] = 99),
        "[E-SCENE] chunk 0: index 99 is past its 20 vertices"
    );
    assert_eq!(
        with(&|c| {
            c.indices.pop();
        }),
        "[E-SCENE] chunk 0: 29 indices are not whole triangles"
    );
    assert_eq!(
        with(&|c| c.vertices[3].material = 5),
        "[E-SCENE] chunk 0: vertex 3 names material 5, but the world has 5"
    );
    assert_eq!(
        with(&|c| c.vertices[0].info = Info::NONE),
        "[E-SCENE] chunk 0: vertex 0 has class None, which `info` keeps for nothing drawn"
    );
    assert_eq!(
        with(&|c| c.vertices[2].uv[1] = f32::NAN),
        "[E-SCENE] chunk 0: vertex 2's position and texture coordinates must be finite"
    );
    assert_eq!(
        with(&|c| c.vertices[1].info.id = 99),
        "[E-SCENE] chunk 0: triangle 0's corners differ in material or `info`; a surface needs \
         its own vertices"
    );
    assert_eq!(
        with(&|c| c.origin.y = f64::INFINITY),
        "[E-SCENE] chunk 0: its origin must be finite"
    );

    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let world = load(&gpu, &good);
    let targets = Targets::new(&gpu, 8, 8).unwrap_or_else(|e| panic!("{e}"));
    let mut scene = room::scene([0.0; 3]);
    scene.sky.zenith.g = f32::INFINITY;
    let error = renderer
        .render(&gpu, &targets, &world, &scene)
        .expect_err("a sky that is not finite");
    assert_eq!(
        error.to_string(),
        "[E-SCENE] the sky's colours must be finite"
    );
    let mut scene = room::scene([0.0; 3]);
    scene.camera.near = 0.0;
    let error = renderer
        .render(&gpu, &targets, &world, &scene)
        .expect_err("a near plane at 0");
    assert!(
        error
            .to_string()
            .starts_with("[E-SCENE] the camera's near plane")
    );
    // Nothing reached the GPU, so there is nothing to read back.
    assert!(
        renderer
            .read_depth(&gpu, &targets)
            .is_err_and(|e| e.to_string().contains("no frame has been rendered"))
    );
}
