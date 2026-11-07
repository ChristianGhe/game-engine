//! The `sprites` pass with T10's look (rung 2, task T11; criterion 8
//! `sprites`): a thing's region brings its light and fog, through `info`,
//! but no fake contrast; what sprites cost a frame; the hall a billion
//! meters out; and the sprites a frame refuses. The CPU cast and the
//! things are `things/mod.rs`'s. Without an adapter each test skips,
//! unless `ENG_REQUIRE_GPU=1` (CI) makes it fail.

mod look;
mod room;
mod things;
mod tone;

use eng_gpu::{Gpu, Targets, Texels};
use eng_render::{
    FirstPersonRenderer, FirstPersonScene, Info, LinearRgb, Look, RenderError, Sprite, SurfaceClass,
};
use room::{Camera64, Face};
use things::{Billboard, EYE, FAR, H, Hit, W, hall_sprites, load, pixels_of, render, sprite};

fn gpu() -> Option<Gpu> {
    look::gpu()
}

/// Whether `got` is within `relative` of `want`, and `absolute` more.
fn close(got: [f64; 3], want: [f64; 3], relative: f64, absolute: f64) -> bool {
    (0..3).all(|i| (got[i] - want[i]).abs() <= relative * want[i].abs() + absolute)
}

#[test]
fn light_and_fog_reach_a_sprite_by_its_region_without_contrast() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let hall = things::hall();
    let world = load(&gpu, [0.0; 3], &hall);
    let targets = Targets::new(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    let faces: Vec<Face> = hall.into_iter().flat_map(|(_, f)| f).collect();
    let placed = hall_sprites();
    // Each region its own fog.
    let mut regions = room::REGIONS.to_vec();
    regions[0].fog = look::GREEN_FOG;
    regions[0].fog_density = 0.15;
    regions[1].fog = LinearRgb {
        r: 20.0,
        g: 4.0,
        b: 1.0,
    };
    regions[1].fog_density = 0.4;
    let scene = |fog: bool| FirstPersonScene {
        sprites: placed.iter().map(|(_, s)| *s).collect(),
        look: Look {
            contrast: true,
            fog,
            ..Look::OFF
        },
        regions: regions.clone(),
        ..things::scene([0.0; 3], EYE, 0, 0)
    };
    let read = |what: fn(&FirstPersonRenderer, &Gpu, &Targets) -> Result<Texels, _>| {
        what(&renderer, &gpu, &targets).unwrap_or_else(|e| panic!("{e}"))
    };
    // Fake contrast on, fog off: the albedo as the adapter decoded it,
    // and the light on it. Then fog on too (which pools `fogged` into
    // albedo's texture): what fog made of the same light.
    let draw = |fog: bool| {
        let info = render(&gpu, &renderer, &targets, &world, &scene(fog)).info;
        (info, read(FirstPersonRenderer::read_lit))
    };
    let (info, lit) = draw(false);
    let albedo = read(FirstPersonRenderer::read_albedo);
    let (_, lit_fog) = draw(true);
    let fogged = read(FirstPersonRenderer::read_fogged);
    let camera = Camera64::new(&scene(true).camera, EYE, W, H);
    let billboards: Vec<Billboard> = placed
        .iter()
        .map(|(feet, s)| Billboard::eight(s, *feet, EYE))
        .collect();
    let (mut checked, mut failures, mut contrasted) = ([0usize; 2], Vec::new(), 0);
    let pi = std::f64::consts::PI;
    for y in 0..H {
        for x in 0..W {
            let a = look::rgb16(&albedo, x, y);
            let (l, lf, f) = (
                look::rgb16(&lit, x, y),
                look::rgb16(&lit_fog, x, y),
                look::rgb16(&fogged, x, y),
            );
            let words = Info::unpack(info.uint2(x, y)).expect("info unpacks");
            // Walls facing X take the contrast: the switch is on.
            if words.class == SurfaceClass::WallFacingX {
                let plain = a.map(|v| v * room::region_lux(words.region) / pi);
                contrasted += usize::from(!close(l, plain, 0.1, 0.0));
            }
            let Some(Hit::Sprite(index, t, _)) = things::seen(&faces, &billboards, &camera, x, y)
            else {
                continue;
            };
            let region = billboards[index].info.region;
            // Lambertian under the region's light alone: no contrast step.
            let want_lit = a.map(|v| v * room::region_lux(region) / pi);
            let ray = camera.ray(f64::from(x) + 0.5, f64::from(y) + 0.5);
            let distance = t * eng_math::sqrt_f64(ray.iter().map(|r| r * r).sum());
            let look = regions[usize::from(region)];
            let amount = 1.0 - f64::from(eng_math::exp(-look.fog_density * distance as f32));
            let fog = [look.fog.r, look.fog.g, look.fog.b].map(f64::from);
            let want_fogged = [0, 1, 2].map(|i| lf[i] + (fog[i] - lf[i]) * amount);
            // f16 keeps 11 bits; the fog's distance and exponential are f32.
            if !close(l, want_lit, 2e-3, 1e-4) || lf != l || !close(f, want_fogged, 5e-3, 5e-3) {
                failures.push(format!(
                    "({x}, {y}) region {region}: albedo {a:.4?}, lit {l:.3?} (CPU {want_lit:.3?}), \
                     fogged {f:.3?} (CPU {want_fogged:.3?})"
                ));
            }
            checked[usize::from(region)] += 1;
        }
    }
    let first: Vec<&String> = failures.iter().take(8).collect();
    assert!(
        failures.is_empty(),
        "{} pixels differ, first: {first:#?}",
        failures.len()
    );
    assert!(checked[0] >= 200 && checked[1] >= 300, "{checked:?}");
    assert!(
        contrasted >= 1_000,
        "{contrasted} wall pixels show contrast"
    );
}

#[test]
fn sprites_cost_one_draw_and_two_records_each() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let world = load(&gpu, [0.0; 3], &things::hall());
    let targets = Targets::new(&gpu, 32, 24).unwrap_or_else(|e| panic!("{e}"));
    let counts = |sprites: Vec<Sprite>| {
        let scene = FirstPersonScene {
            sprites,
            ..things::scene([0.0; 3], EYE, 0, 0)
        };
        let c = renderer
            .render(&gpu, &targets, &world, &scene)
            .unwrap_or_else(|e| panic!("{e}"));
        (
            c.draws,
            c.pipeline_switches,
            c.bind_group_switches,
            c.upload_bytes,
        )
    };
    // Two chunks, light, sky and the tone map, as T9 counted them.
    assert_eq!(counts(Vec::new()), (5, 4, 8, 624 + 2 * 16));
    // The sprites add one draw, its pipeline, the frame and the scene,
    // and 32 bytes each.
    let four: Vec<Sprite> = hall_sprites().into_iter().map(|(_, s)| s).collect();
    assert_eq!(counts(four), (6, 5, 10, 624 + 2 * 16 + 4 * 32));
}

#[test]
fn the_hall_a_billion_meters_out_renders_as_it_does_at_the_origin() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let targets = Targets::new(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    let frame_at = |corner: [f64; 3]| {
        let world = load(&gpu, corner, &things::hall());
        let scene = FirstPersonScene {
            sprites: hall_sprites()
                .into_iter()
                .map(|(feet, s)| Sprite {
                    feet: room::add(corner, feet),
                    ..s
                })
                .collect(),
            ..things::scene(corner, EYE, 0, 0)
        };
        render(&gpu, &renderer, &targets, &world, &scene)
    };
    let (near, far) = (frame_at([0.0; 3]), frame_at(FAR));
    assert!(!pixels_of(&near, 101).is_empty());
    assert!(near.image == far.image, "the output differs");
    assert!(near.depth == far.depth && near.info == far.info);
}

#[test]
fn a_sprite_the_frame_cannot_draw_is_refused_before_the_gpu() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let world = load(&gpu, [0.0; 3], &things::hall());
    let targets = Targets::new(&gpu, 32, 24).unwrap_or_else(|e| panic!("{e}"));
    let good = sprite(1, 0, [0.0; 3], [6.0, 0.0, 0.0], 0);
    let refused = |bad: Sprite| {
        let scene = FirstPersonScene {
            sprites: vec![good, bad],
            ..things::scene([0.0; 3], EYE, 0, 0)
        };
        match renderer.render(&gpu, &targets, &world, &scene) {
            Err(e @ RenderError::Scene { .. }) => e.to_string(),
            other => panic!("{other:?}"),
        }
    };
    let mut materials = things::FRAMES;
    materials[3] = 13;
    assert!(
        refused(Sprite { materials, ..good })
            .ends_with("sprite 1: it names material 13, but the world has 13"),
    );
    let message = refused(Sprite {
        height: -1.0,
        ..good
    });
    assert!(message.starts_with("[E-SCENE]"), "{message}");
    assert!(
        message.contains("sprite 1: its size is 1.5 x -1 m"),
        "{message}"
    );
}
