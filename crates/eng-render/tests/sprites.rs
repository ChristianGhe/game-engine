//! The first-person `sprites` pass on a real adapter (rung 2, task T11;
//! plan decision 4; criterion 8 `sprites`). Each test draws things as
//! billboards and checks every pixel that a CPU ray cast in f64 calls
//! unambiguous (`things/mod.rs`): its `info`, its depth to T9's tolerance
//! (`first_person.rs`), and its colour, lit by its region and tone mapped
//! as the CPU reference does it, to one level. The frames' test textures
//! are told apart by colour, and each has a marker on its right half, so a
//! wrong rotation or a mirror shows. Without an adapter each test skips,
//! unless `ENG_REQUIRE_GPU=1` (CI) makes it fail.

mod look;
mod room;
mod things;
mod tone;

use eng_gpu::{Gpu, Targets};
use eng_math::{TAU_F64, sin_cos_f64};
use eng_render::{FirstPersonRenderer, FirstPersonScene, Info, Look, RotationSheet, Sprite};
use room::{Camera64, Face};
use things::{Billboard, EYE, Frame, H, Hit, W, hall_sprites, load, pixels_of, render, sprite};

fn gpu() -> Option<Gpu> {
    look::gpu()
}

/// An eighth of a turn, in binary angle.
const EIGHTH: u32 = 1 << 29;

/// What the comparison with the CPU found.
#[derive(Default)]
struct Compared {
    failures: Vec<String>,
    /// Pixels the CPU and the GPU agree show each sprite, by index.
    sprite_pixels: Vec<usize>,
    /// Pixels inside a billboard where its texel is cut out and a face
    /// shows through.
    through: usize,
    /// Pixels where a sprite's opaque texel hides another's farther along
    /// the same ray.
    over_sprite: usize,
    edges: usize,
}

/// Compares `frame` with the CPU's cast of `faces` and `billboards` from
/// `camera`, every look feature off.
fn compare(frame: &Frame, faces: &[Face], billboards: &[Billboard], camera: &Camera64) -> Compared {
    let mut c = Compared {
        sprite_pixels: vec![0; billboards.len()],
        ..Compared::default()
    };
    for y in 0..H {
        for x in 0..W {
            let (cx, cy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let depth = f64::from(frame.depth.depth(x, y));
            let info = Info::unpack(frame.info.uint2(x, y));
            let pixel = frame.image.pixel(x, y);
            let mut fail = |what: String| c.failures.push(format!("({x}, {y}): {what}"));
            let Some(hit) = things::seen(faces, billboards, camera, x, y) else {
                c.edges += 1;
                continue;
            };
            let (want_info, want_depth, tolerance, srgb) = match hit {
                Hit::Sky => {
                    if depth != 0.0 || info != Some(Info::NONE) {
                        fail(format!("sky, but depth {depth}, info {info:?}"));
                    }
                    continue;
                }
                Hit::Face(index, t) => {
                    let face = &faces[index];
                    let ray = camera.ray(cx, cy);
                    let inside = billboards.iter().any(|b| b.texel(camera, ray).is_some());
                    c.through += usize::from(inside);
                    let tolerance = room::depth_tolerance(face, camera, cx, cy);
                    let srgb = room::face_colour(face, camera.eye, ray, t);
                    (face.info, camera.near / t, tolerance, srgb)
                }
                Hit::Sprite(index, t, srgb) => {
                    c.sprite_pixels[index] += 1;
                    let ray = camera.ray(cx, cy);
                    let behind = billboards.iter().enumerate().any(|(other, b)| {
                        b.texel(camera, ray).is_some_and(|(tb, column, row)| {
                            other != index && tb > t && things::opaque(column, row)
                        })
                    });
                    c.over_sprite += usize::from(behind);
                    let b = &billboards[index];
                    let tolerance = things::depth_tolerance(b, camera, cx, cy);
                    (b.info, camera.near / t, tolerance, srgb)
                }
            };
            if info != Some(want_info) {
                fail(format!("info {info:?}, the CPU says {want_info:?}"));
            }
            if (depth - want_depth).abs() > tolerance {
                fail(format!(
                    "depth {depth}, the CPU {want_depth} (± {tolerance:e})"
                ));
            }
            let lux = room::region_lux(want_info.region);
            let want = tone::toned_codes(tone::lambert(srgb, lux), room::EV100);
            if !tone::within_one(pixel, want) {
                fail(format!(
                    "colour {pixel:?}, the CPU {want:.1?} ({want_info:?})"
                ));
            }
        }
    }
    c
}

fn assert_no_failures(c: &Compared, what: &str) {
    let first: Vec<&str> = c.failures.iter().take(12).map(String::as_str).collect();
    assert!(
        c.failures.is_empty(),
        "{what}: {} differences from the CPU (info, depth and colour each count), first:\n{}",
        c.failures.len(),
        first.join("\n")
    );
    assert!(
        c.edges < (W * H / 10) as usize,
        "{what}: {} edge pixels",
        c.edges
    );
}

#[test]
fn the_hall_renders_its_sprites_as_the_cpu_casts_them() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let hall = things::hall();
    let world = load(&gpu, [0.0; 3], &hall);
    let targets = Targets::new(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    let placed = hall_sprites();
    let scene = FirstPersonScene {
        sprites: placed.iter().map(|(_, s)| *s).collect(),
        ..things::scene([0.0; 3], EYE, 0, 0)
    };
    let frame = render(&gpu, &renderer, &targets, &world, &scene);
    let camera = Camera64::new(&scene.camera, EYE, W, H);
    let billboards: Vec<Billboard> = placed
        .iter()
        .map(|(feet, s)| Billboard::eight(s, *feet, EYE))
        .collect();
    let faces: Vec<Face> = hall.into_iter().flat_map(|(_, f)| f).collect();
    let c = compare(&frame, &faces, &billboards, &camera);
    assert_no_failures(&c, "the hall");
    eprintln!(
        "sprite pixels {:?}, {} seen through cut texels, {} of a sprite over another, {} edges",
        c.sprite_pixels, c.through, c.over_sprite, c.edges
    );
    // The thing behind the pillar shows nowhere, edges included: the
    // pillar's depth hides it.
    assert_eq!(c.sprite_pixels[0], 0);
    assert!(
        pixels_of(&frame, 100).is_empty(),
        "100 shows past the pillar"
    );
    // The others show, hiding the wall and each other, and the cut texels
    // let the wall and floor show through.
    for (index, least) in [(1, 200), (2, 200), (3, 50)] {
        assert!(c.sprite_pixels[index] >= least, "{:?}", c.sprite_pixels);
    }
    assert!(c.through >= 300, "{} pixels seen through", c.through);
    // 103, drawn after 101 and farther, shows only where 101's depth lets
    // it: behind 101's opaque texels it is hidden.
    assert!(
        c.over_sprite >= 50,
        "{} pixels of 101 over 103",
        c.over_sprite
    );
    // Every look: the plain pipeline draws the same sprites as the look
    // does with every switch off, byte for byte, and no switch moves a
    // sprite's depth or `info`.
    let plain = renderer
        .render_plain(&gpu, &targets, &world, &scene)
        .map(|_| targets.read_output(&gpu).unwrap_or_else(|e| panic!("{e}")))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        plain == frame.image,
        "Look::OFF differs from the plain pipeline"
    );
    let one_on = [
        Look {
            falloff: true,
            ..Look::OFF
        },
        Look {
            banding: true,
            ..Look::OFF
        },
        Look {
            contrast: true,
            ..Look::OFF
        },
        Look {
            sky_gradient: true,
            ..Look::OFF
        },
        Look {
            fog: true,
            ..Look::OFF
        },
        Look {
            palette: true,
            ..Look::OFF
        },
        Look::ON,
    ];
    for look in one_on {
        let with = render(
            &gpu,
            &renderer,
            &targets,
            &world,
            &FirstPersonScene {
                look,
                ..scene.clone()
            },
        );
        assert!(
            with.depth == frame.depth && with.info == frame.info,
            "{look:?}"
        );
    }
}

/// An eye `eighths` eighths of a turn counter-clockwise of `facing`, 3 m
/// from a thing at the origin, and a yaw looking back at it.
fn eye_around(facing: u32, eighths: u32) -> ([f64; 3], u32) {
    let toward = facing.wrapping_add(eighths.wrapping_mul(EIGHTH));
    let (s, c) = sin_cos_f64(f64::from(toward) * TAU_F64 / 4_294_967_296.0);
    ([3.0 * c, 3.0 * s, 1.625], toward.wrapping_add(1 << 31))
}

/// Draws one thing of `sheet` from each of its 8 rotations' views, and
/// checks that rotation `k` shows `frame(k)`, mirrored as it says.
fn each_rotation(sheet: RotationSheet, frame: impl Fn(u32) -> (usize, bool)) {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let world = load(&gpu, [0.0; 3], &[]);
    let targets = Targets::new(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    let facing = 0x1234_5678;
    let thing = Sprite {
        sheet,
        ..sprite(7, 0, [0.0; 3], [0.0; 3], facing)
    };
    for k in 0..8 {
        let (eye, yaw) = eye_around(facing, k);
        let scene = FirstPersonScene {
            sprites: vec![thing],
            ..things::scene([0.0; 3], eye, yaw, 0)
        };
        let shot = render(&gpu, &renderer, &targets, &world, &scene);
        let (want, mirrored) = frame(k);
        // The CPU's geometry agrees: the eye is in rotation k's sector.
        assert_eq!(Billboard::eight(&thing, [0.0; 3], eye).frame, k as usize);
        let camera = Camera64::new(&scene.camera, eye, W, H);
        let billboard = Billboard::shown(&thing, [0.0; 3], want, mirrored);
        let c = compare(&shot, &[], &[billboard], &camera);
        assert_no_failures(&c, &format!("rotation {k}"));
        // By colour alone: the frame's colour shows, no other frame's, and
        // the marker on the right unless mirrored.
        let lux = room::region_lux(0);
        let toned = |srgb| tone::toned_codes(tone::lambert(srgb, lux), room::EV100);
        let mut columns = vec![Vec::new(); 9];
        for (x, y) in pixels_of(&shot, 7) {
            let pixel = shot.image.pixel(x, y);
            let mut colours = things::FRAME_COLOURS.to_vec();
            colours.push(things::MARKER);
            for (index, srgb) in colours.into_iter().enumerate() {
                if tone::within_one(pixel, toned(srgb)) {
                    columns[index].push(f64::from(x));
                }
            }
        }
        let shown: Vec<usize> = columns[..8].iter().map(Vec::len).collect();
        for (index, &count) in shown.iter().enumerate() {
            let enough = if index == want {
                count >= 1_000
            } else {
                count == 0
            };
            assert!(enough, "rotation {k} should show frame {want}: {shown:?}");
        }
        assert!(!columns[8].is_empty(), "rotation {k}: no marker shows");
        let mean = |xs: &[f64]| xs.iter().sum::<f64>() / xs.len() as f64;
        let marker_right = mean(&columns[8]) > mean(&columns[want]);
        assert_eq!(marker_right, !mirrored, "rotation {k}: the marker's side");
    }
}

#[test]
fn each_of_the_eight_rotations_shows_from_its_own_view() {
    each_rotation(RotationSheet::Eight, |k| (k as usize, false));
}

#[test]
fn a_five_rotation_sheet_shows_rotations_5_to_7_mirrored() {
    each_rotation(RotationSheet::Five, |k| {
        if k > 4 {
            ((8 - k) as usize, true)
        } else {
            (k as usize, false)
        }
    });
}

#[test]
fn billboards_turn_about_z_only() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let hall = things::hall();
    let world = load(&gpu, [0.0; 3], &hall);
    let targets = Targets::new(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    let faces: Vec<Face> = hall.into_iter().flat_map(|(_, f)| f).collect();
    // The hall's things and a near one, tall, so that a billboard tilted
    // with the camera would move far from the upright one.
    let mut placed = hall_sprites();
    placed.push((
        [2.5, 0.75, 0.0],
        sprite(104, 0, [0.0; 3], [2.5, 0.75, 0.0], 0),
    ));
    // Pitched 22.5° up, then down, and turned a little.
    for pitch in [1u32 << 28, (1u32 << 28).wrapping_neg()] {
        let scene = FirstPersonScene {
            sprites: placed.iter().map(|(_, s)| *s).collect(),
            ..things::scene([0.0; 3], EYE, 0x0400_0000, pitch)
        };
        let frame = render(&gpu, &renderer, &targets, &world, &scene);
        let camera = Camera64::new(&scene.camera, EYE, W, H);
        let billboards: Vec<Billboard> = placed
            .iter()
            .map(|(feet, s)| Billboard::eight(s, *feet, EYE))
            .collect();
        let c = compare(&frame, &faces, &billboards, &camera);
        assert_no_failures(&c, &format!("pitch {:#x}", pitch));
        assert!(c.sprite_pixels[4] >= 2_000, "{:?}", c.sprite_pixels);
    }
}
