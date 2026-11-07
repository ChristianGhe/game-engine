//! A CPU oracle for the first-person light model and fog (rung 2, task
//! T10), on a real adapter. The room of `room/mod.rs` is drawn with every
//! light switch on, and every pixel that a CPU ray cast in f64 calls
//! unambiguous is checked against numbers written here, not taken from
//! eng-render:
//! - **light:** the step is the region's level over 8, plus 2 on a wall
//!   facing ±X and minus 2 on one facing ±Y, minus 0.5 per meter of the
//!   distance along the pixel's ray, clamped to 0 to 31 and, with banding,
//!   rounded down; its lux is `320 * 2^((step - 31) / 4)` at a whole step,
//!   linear between two; the luminance is albedo times lux over π. `lit`
//!   must hold it to half-float rounding, and the output must be within
//!   one level of the CPU's tone map of it (`tone/mod.rs`). With banding,
//!   a pixel whose step is within the distance's error of a whole step is
//!   left out: either side of it is right. The albedo is the GPU's own,
//!   read back from `albedo`: the oracle checks the light model, not the
//!   adapter's sRGB decode of the texture, which is the adapter's (Mesa
//!   lavapipe's is 1.1% below the formula at code 40, Metal's 0.09%), and
//!   which the decode is checked against only loosely, to see the texel
//!   is the face's;
//! - **fog:** `fogged` is `lit + (fog - lit) * (1 - e^(-density * distance))`
//!   with the pixel's region's fog, to half-float rounding and the
//!   distance's error; the sky keeps `lit` exactly.
//!
//! Fog and the palette are off for the light check (each adds to the
//! output, and both are checked apart), and so is everything they hold:
//! the dither works only with the palette, and low resolution only in a
//! window, so neither changes these offscreen frames. Without an adapter
//! each test skips, unless `ENG_REQUIRE_GPU=1` (CI) makes it fail.

mod look;
mod room;
mod tone;

use eng_gpu::Targets;
use eng_render::{
    CONTRAST_STEPS, FirstPersonRenderer, LIGHT_FALLOFF, LIGHT_STEP_LUX, Look, SurfaceClass,
};
use look::{H, W, foggy_regions, gpu, lut, render, rgb16, room_world, scene};
use room::{AROUND, Camera64, Face, Seen, depth_tolerance, seen};

/// Twice a half float's rounding, relative: 2^-10.
const HALF_ULP_TWICE: f64 = 1.0 / 1024.0;

/// How far the adapter's decoded albedo may be from the sRGB formula's,
/// relative: a check that the texel is the face's, not of the decode.
const DECODE_SLACK: f64 = 0.03;

/// A pixel the CPU knows: where it is, the face it shows and how far away.
struct Probe {
    x: u32,
    y: u32,
    face: Face,
    /// Along the ray through its centre, in meters.
    distance: f64,
    /// How far the GPU's distance may be from it: the depth's tolerance
    /// (`first_person.rs`), relative, times the distance.
    error: f64,
    /// Whether its albedo is known: the floor's two halves only away from
    /// their edge.
    colour_known: bool,
    albedo: [u8; 3],
}

/// Every pixel of the room's view that the CPU calls unambiguous.
fn probes() -> (Vec<Probe>, Vec<(u32, u32)>) {
    let scene = scene(Look::ON);
    let camera = Camera64::new(&scene.camera, room::EYE, W, H);
    let faces = room::faces();
    let (mut probes, mut sky) = (Vec::new(), Vec::new());
    for y in 0..H {
        for x in 0..W {
            let (cx, cy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let ray = camera.ray(cx, cy);
            match seen(&faces, &camera, x, y) {
                Seen::Edge => {}
                Seen::Sky => sky.push((x, y)),
                Seen::Face(index, t) => {
                    let face = faces[index];
                    let length = eng_math::sqrt_f64(ray.iter().map(|r| r * r).sum());
                    let depth = camera.near / t;
                    let relative = depth_tolerance(&face, &camera, cx, cy) / depth;
                    let colour_known = face.material != 0
                        || AROUND.iter().all(|(dx, dy)| {
                            let around = camera.ray(cx + dx, cy + dy);
                            let ta = room::plane_t(&face, camera.eye, around).unwrap_or(t);
                            room::floor_half(camera.eye, around, ta)
                                == room::floor_half(camera.eye, ray, t)
                        });
                    probes.push(Probe {
                        x,
                        y,
                        face,
                        distance: t * length,
                        error: t * length * relative + 1e-6,
                        colour_known,
                        albedo: room::face_colour(&face, camera.eye, ray, t),
                    });
                }
            }
        }
    }
    (probes, sky)
}

/// The lux of light step `step`, 0 to 31, by the model's own formula.
fn lux(step: f64) -> f64 {
    let at = |whole: f64| 320.0 * f64::from(eng_math::exp2(((whole - 31.0) / 4.0) as f32));
    let below = eng_math::floor_f64(step);
    let above = eng_math::min_f64(below + 1.0, 31.0);
    at(below) + (at(above) - at(below)) * (step - below)
}

/// The light step of `probe` before clamping and banding.
fn raw_step(probe: &Probe) -> f64 {
    let level = foggy_regions()[usize::from(probe.face.info.region)].light;
    let contrast = match probe.face.info.class {
        SurfaceClass::WallFacingX => 2.0,
        SurfaceClass::WallFacingY => -2.0,
        _ => 0.0,
    };
    f64::from(level) / 8.0 + contrast - 0.5 * probe.distance
}

#[test]
fn the_light_model_is_the_cpus_on_every_unambiguous_pixel() {
    let Some(gpu) = gpu() else { return };
    // The numbers the oracle writes out are the crate's.
    assert_eq!((CONTRAST_STEPS, LIGHT_FALLOFF), (2.0, 0.5));
    for (step, &table) in LIGHT_STEP_LUX.iter().enumerate() {
        let want = lux(step as f64);
        assert!(
            (f64::from(table) - want).abs() <= want * 1e-6,
            "step {step}"
        );
    }
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let world = room_world(&gpu, &lut());
    let targets = Targets::new(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    let (probes, sky) = probes();
    assert!(probes.len() > 50_000 && sky.len() > 5_000);
    for banding in [true, false] {
        let look = Look {
            fog: false,
            palette: false,
            banding,
            ..Look::ON
        };
        let scene = scene(look);
        let frame = render(&gpu, &renderer, &targets, &world, &scene, false);
        let lit = renderer
            .read_lit(&gpu, &targets)
            .unwrap_or_else(|e| panic!("{e}"));
        // Without fog, nothing draws over `albedo` after the light pass.
        let albedo = renderer
            .read_albedo(&gpu, &targets)
            .unwrap_or_else(|e| panic!("{e}"));
        let (mut checked, mut on_edges, mut failures) = (0, 0, Vec::new());
        let mut steps = std::collections::BTreeSet::new();
        for probe in &probes {
            let raw = raw_step(probe);
            let step_error = 0.5 * probe.error + 1e-5;
            // A whole step from 1 to 31 is an edge of the banded light.
            let nearest = eng_math::round_f64(raw);
            if banding
                && (1.0..=31.0).contains(&nearest)
                && (raw - nearest).abs() < 2.0 * step_error
            {
                on_edges += 1;
                continue;
            }
            let mut step = eng_math::clamp_f64(raw, 0.0, 31.0);
            if banding {
                step = eng_math::floor_f64(step);
            }
            steps.insert((step * 8.0) as i64);
            checked += 1;
            let decoded = rgb16(&albedo, probe.x, probe.y);
            if probe.colour_known {
                let formula = probe.albedo.map(tone::decode);
                if (0..3).any(|c| (decoded[c] - formula[c]).abs() > formula[c] * DECODE_SLACK) {
                    failures.push(format!(
                        "({}, {}) face {}: albedo {decoded:.4?}, not the face's {formula:.4?}",
                        probe.x, probe.y, probe.face.info.id
                    ));
                }
            }
            let nits = decoded.map(|a| a * lux(step) / std::f64::consts::PI);
            let got = rgb16(&lit, probe.x, probe.y);
            // `lit` rounds to a half float once (2^-11) after f32 sums; and
            // without banding the distance's error moves the step too, and
            // lux a quarter stop a step (under 0.2 relative per step).
            let tolerance = HALF_ULP_TWICE + if banding { 0.0 } else { 0.2 * step_error };
            for c in 0..3 {
                if (got[c] - nits[c]).abs() > nits[c] * tolerance + 1e-6 {
                    failures.push(format!(
                        "({}, {}) face {}: lit {got:.4?}, the CPU {nits:.4?} at step {step}",
                        probe.x, probe.y, probe.face.info.id
                    ));
                    break;
                }
            }
            let want = tone::toned_codes(nits, room::EV100);
            let pixel = frame.image.pixel(probe.x, probe.y);
            if !tone::within_one(pixel, want) {
                failures.push(format!(
                    "({}, {}) face {}: output {pixel:?}, the CPU {want:.2?}",
                    probe.x, probe.y, probe.face.info.id
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "banding {banding}: {} differences over {checked} pixels (albedo, lit and the \
             output each count), first:\n{}",
            failures.len(),
            failures[..failures.len().min(12)].join("\n")
        );
        assert!(checked > 40_000, "only {checked} checked");
        assert!(on_edges < probes.len() / 50, "{on_edges} on edges");
        // The view spans several steps, or a model of one step would pass.
        assert!(steps.len() >= 6, "steps (x8) {steps:?}");
        eprintln!(
            "banding {banding}: {checked} pixels checked, {on_edges} on step edges, {} steps",
            steps.len()
        );
    }
}

#[test]
fn fog_is_the_cpus_mix_toward_each_regions_colour() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let world = room_world(&gpu, &lut());
    let targets = Targets::new(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    let look = Look {
        palette: false,
        ..Look::ON
    };
    render(&gpu, &renderer, &targets, &world, &scene(look), false);
    let lit = renderer
        .read_lit(&gpu, &targets)
        .unwrap_or_else(|e| panic!("{e}"));
    let fogged = renderer
        .read_fogged(&gpu, &targets)
        .unwrap_or_else(|e| panic!("{e}"));
    let regions = foggy_regions();
    let (probes, sky) = probes();
    let mut failures = Vec::new();
    let mut per_region = [0usize; 2];
    for probe in &probes {
        let region = regions[usize::from(probe.face.info.region)];
        let fog = [region.fog.r, region.fog.g, region.fog.b].map(f64::from);
        let density = f64::from(region.fog_density);
        let share = 1.0 - f64::from(eng_math::exp((-density * probe.distance) as f32));
        let (l, f) = (
            rgb16(&lit, probe.x, probe.y),
            rgb16(&fogged, probe.x, probe.y),
        );
        for c in 0..3 {
            let want = l[c] + (fog[c] - l[c]) * share;
            // `fogged` rounds to a half float; the share moves with the
            // distance's error by density times e^(-density * distance).
            let tolerance = want.abs() * HALF_ULP_TWICE
                + (fog[c] - l[c]).abs() * density * probe.error * 2.0
                + 1e-6;
            if (f[c] - want).abs() > tolerance {
                failures.push(format!(
                    "({}, {}) region {}: fogged {f:.4?}, the CPU {want:.4} in channel {c} from \
                     lit {l:.4?} at {:.3} m",
                    probe.x, probe.y, probe.face.info.region, probe.distance
                ));
                break;
            }
        }
        per_region[usize::from(probe.face.info.region)] += 1;
    }
    assert!(
        failures.is_empty(),
        "{} of {} pixels differ, first:\n{}",
        failures.len(),
        probes.len(),
        failures[..failures.len().min(12)].join("\n")
    );
    assert!(
        per_region[0] > 40_000 && per_region[1] > 1_000,
        "{per_region:?}"
    );
    // The sky is not fogged at all.
    for &(x, y) in &sky {
        assert_eq!(fogged.texel(x, y), lit.texel(x, y), "sky at ({x}, {y})");
    }
}
