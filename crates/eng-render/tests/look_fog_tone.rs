//! The first-person look on a real adapter, continued (rung 2, task T10;
//! plan decision 13, with `look_checks.rs`):
//! - fog moves monotonically toward its colour with distance along probe
//!   rows, to within half-float rounding in `fogged`, exactly in 8 bits;
//! - the tone curve is within one 8-bit level of a CPU reference written
//!   from Khronos's definition (`tone/mod.rs`) on sky ramps.
//!
//! Without an adapter each test skips, unless `ENG_REQUIRE_GPU=1` (CI)
//! makes it fail.

mod look;
mod room;
mod tone;

use std::collections::BTreeSet;

use eng_gpu::Targets;
use eng_math::{BAngle, Ev100, WorldPos};
use eng_render::{
    FirstPersonCamera, FirstPersonRenderer, FirstPersonScene, LinearRgb, Look, NEAR_PLANE, Sky,
};
use look::{Frame, GREEN_FOG, H, W, flat_world, gpu, lut, render, rgb16, room_world, scene};

/// The probe rows: for each row of `rows`, the pixels of surface `id`,
/// each with its distance from the eye along its ray (as fog and falloff
/// measure it: `near / depth` ahead, times the ray's length per meter
/// ahead, the ray cast on the CPU), nearest first.
fn probe_rows(frame: &Frame, rows: &[u32], id: u32) -> Vec<Vec<(u32, u32, f64)>> {
    let scene = room::scene([0.0; 3]);
    let camera = room::Camera64::new(&scene.camera, room::EYE, W, H);
    rows.iter()
        .map(|&y| {
            let mut pixels: Vec<(u32, u32, f64)> = (0..W)
                .filter(|&x| frame.info.uint2(x, y)[1] == id)
                .map(|x| {
                    let ray = camera.ray(f64::from(x) + 0.5, f64::from(y) + 0.5);
                    let length = eng_math::sqrt_f64(ray.iter().map(|r| r * r).sum());
                    let ahead = camera.near / f64::from(frame.depth.depth(x, y));
                    (x, y, ahead * length)
                })
                .collect();
            pixels.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)));
            pixels
        })
        .collect()
}

/// Whether `b` is further than `a` by more than the GPU's and the CPU's
/// distances could disagree by (f32 rays, interpolated depth): so close,
/// two pixels' order is not known, and neither is their fog's.
fn further(a: f64, b: f64) -> bool {
    b > a * (1.0 + 1e-4)
}

/// The two walls across Y and the far wall, seen at an angle, each in
/// region 0.
const PROBED: [u32; 3] = [2, 4, 5];
const PROBE_ROWS: [u32; 6] = [70, 90, 110, 130, 150, 170];

#[test]
fn fog_moves_monotonically_toward_its_colour_with_depth_along_probe_rows() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let lut = lut();
    let targets = Targets::new(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));

    // In `fogged`, coloured, every other switch on: the share of the way
    // from `lit` to the fog's colour, in the channel that moves most,
    // grows with distance, to within the half floats' rounding.
    let world = room_world(&gpu, &lut);
    let frame = render(&gpu, &renderer, &targets, &world, &scene(Look::ON), false);
    let lit = renderer
        .read_lit(&gpu, &targets)
        .unwrap_or_else(|e| panic!("{e}"));
    let fogged = renderer
        .read_fogged(&gpu, &targets)
        .unwrap_or_else(|e| panic!("{e}"));
    let fog = [GREEN_FOG.r, GREEN_FOG.g, GREEN_FOG.b].map(f64::from);
    let mut probes = 0;
    for id in PROBED {
        for row in probe_rows(&frame, &PROBE_ROWS, id) {
            let shares: Vec<(f64, f64, f64)> = row
                .iter()
                .map(|&(x, y, distance)| {
                    let (l, f) = (rgb16(&lit, x, y), rgb16(&fogged, x, y));
                    let c = (0..3)
                        .max_by(|&a, &b| (fog[a] - l[a]).abs().total_cmp(&(fog[b] - l[b]).abs()))
                        .unwrap_or(1);
                    let span = fog[c] - l[c];
                    // `fogged` is rounded to a half float once: half a
                    // unit in the last place, 2^-11 relative, twice over.
                    let tolerance = 2.0 * f[c].abs() / 2048.0 / span.abs();
                    ((f[c] - l[c]) / span, tolerance, distance)
                })
                .collect();
            for pair in shares.windows(2) {
                let [(share, tol, near), (next, next_tol, far)] = [pair[0], pair[1]];
                assert!(
                    !further(near, far) || next >= share - tol - next_tol,
                    "surface {id}: the fog's share falls from {share} at {near} m to {next} at \
                     {far} m"
                );
            }
            for &(share, tol, _) in &shares {
                assert!((-tol..=1.0 + tol).contains(&share), "a share of {share}");
            }
            if let (Some(first), Some(last)) = (shares.first(), shares.last())
                && shares.len() > 20
            {
                assert!(last.0 > first.0, "surface {id}: no fog grows along the row");
            }
            probes += shares.len();
        }
    }
    assert!(probes > 500, "only {probes} probes");

    // In the 8-bit output, grey: with the light the same along each row
    // (no falloff or banding), each step moves toward the fog's code or
    // stays, never away, and the rows climb by many levels.
    let grey = flat_world(&gpu, [128, 128, 128], &lut);
    let mut look = Look {
        falloff: false,
        banding: false,
        palette: false,
        ..Look::ON
    };
    let mut grey_scene = scene(look);
    grey_scene.regions[0].fog = LinearRgb {
        r: 40.0,
        g: 40.0,
        b: 40.0,
    };
    let frame = render(&gpu, &renderer, &targets, &grey, &grey_scene, false);
    let fog_code = tone::toned_codes([40.0; 3], room::EV100);
    let mut climbed = 0;
    for id in PROBED {
        for row in probe_rows(&frame, &PROBE_ROWS, id) {
            let codes: Vec<u8> = row
                .iter()
                .map(|&(x, y, _)| frame.image.pixel(x, y)[0])
                .collect();
            for (pair, probes) in codes.windows(2).zip(row.windows(2)) {
                assert!(
                    !further(probes[0].2, probes[1].2) || pair[1] >= pair[0],
                    "surface {id}: {codes:?}"
                );
            }
            for &code in &codes {
                assert!(f64::from(code) <= fog_code[0] + 1.0, "past the fog: {code}");
            }
            if let (Some(first), Some(last)) = (codes.first(), codes.last()) {
                climbed = climbed.max(last - first);
            }
        }
    }
    assert!(climbed >= 10, "the fog climbs only {climbed} levels");
    // The check can fail: the same rows without fog do not climb.
    look.fog = false;
    grey_scene.look = look;
    let frame = render(&gpu, &renderer, &targets, &grey, &grey_scene, false);
    let row = &probe_rows(&frame, &[110], 5)[0];
    let codes: BTreeSet<u8> = row
        .iter()
        .map(|&(x, y, _)| frame.image.pixel(x, y)[0])
        .collect();
    assert!(codes.len() <= 2, "{codes:?}");
}

/// A camera above the room's walls, looking up 45° with a 90° field of
/// view: every pixel is sky, from the horizon at the bottom edge to near
/// the zenith at the top.
fn sky_scene(sky: Sky) -> FirstPersonScene {
    FirstPersonScene {
        camera: FirstPersonCamera {
            eye: WorldPos::new(3.0, 2.0, 10.0),
            yaw: BAngle(0),
            pitch: BAngle(1 << 29),
            fov_y: BAngle::QUARTER,
            near: NEAR_PLANE,
        },
        sky,
        ev100: Ev100(0.0),
        look: Look {
            palette: false,
            ..Look::ON
        },
        regions: Vec::new(),
        sprites: Vec::new(),
    }
}

const fn rgb(r: f32, g: f32, b: f32) -> LinearRgb {
    LinearRgb { r, g, b }
}

#[test]
fn the_tone_curve_is_within_one_level_of_the_cpu_reference_on_sky_ramps() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let world = room_world(&gpu, &lut());
    let targets = Targets::new(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    // EV100 0: 1.2 nits expose to 1. Grey up to 16 (deep in the
    // compression), grey up to 0.1 (the toe), and two colours that the
    // offset and the desaturation treat by channel.
    let ramps = [
        ("grey to 16", rgb(0.0, 0.0, 0.0), rgb(19.2, 19.2, 19.2)),
        ("grey to 0.1", rgb(0.0, 0.0, 0.0), rgb(0.12, 0.12, 0.12)),
        ("orange to 8", rgb(0.0, 0.0, 0.0), rgb(9.6, 2.4, 0.6)),
        ("violet to green", rgb(0.6, 0.06, 1.2), rgb(0.02, 3.0, 0.3)),
    ];
    for (name, horizon, zenith) in ramps {
        let scene = sky_scene(Sky { horizon, zenith });
        let frame = render(&gpu, &renderer, &targets, &world, &scene, false);
        let camera = room::Camera64::new(&scene.camera, [3.0, 2.0, 10.0], W, H);
        let mut worst = 0.0f64;
        let mut codes = BTreeSet::new();
        for y in 0..H {
            for x in 0..W {
                assert_eq!(frame.depth.depth(x, y), 0.0, "({x}, {y}) is not sky");
                let ray = camera.ray(f64::from(x) + 0.5, f64::from(y) + 0.5);
                let nits = room::sky_colour(&scene.sky, ray);
                let want = tone::toned_codes(nits, 0.0);
                let got = frame.image.pixel(x, y);
                for c in 0..3 {
                    worst = eng_math::max_f64(worst, (f64::from(got[c]) - want[c]).abs());
                }
                assert!(
                    tone::within_one(got, want),
                    "{name}: ({x}, {y}) is {got:?}, the CPU says {want:.2?} for {nits:.4?} nits"
                );
                codes.insert(got[0]);
            }
        }
        eprintln!(
            "{name}: worst {worst:.3} of a level, {} red codes",
            codes.len()
        );
        assert!(codes.len() >= 20, "{name}: a ramp of {} codes", codes.len());
    }
}
