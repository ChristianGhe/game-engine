//! The first-person look on a real adapter (rung 2, task T10; plan
//! decision 13's structured look checks, with `look_fog_tone.rs`). Each
//! compares the adapter with itself or tests a property, so it is exact on
//! every adapter:
//! - every look switch off gives the plain pipeline's image, byte for byte,
//!   and each switch alone changes it;
//! - with the palette on, every pixel is a palette colour, the one the CPU
//!   table gives the tone-mapped pixel (dithered as the CPU dithers it);
//! - on a flat material, banding gives at most 32 levels per region;
//!
//! and the frame's readbacks, counters, refusals and k×k present. The room
//! is `room/mod.rs`'s, a 6 by 4 m box open to the sky with a step. Without
//! an adapter each test skips, unless `ENG_REQUIRE_GPU=1` (CI) makes it
//! fail.

mod look;
mod room;

use std::collections::{BTreeMap, BTreeSet};

use eng_gpu::Targets;
use eng_math::Ev100;
use eng_render::{
    ColourTag, FirstPersonRenderer, FirstPersonScene, GpuWorld, Info, Look, RegionLook,
    SurfaceClass, WorldMesh, bayer4_offset,
};
use look::{Frame, H, W, flat_world, gpu, lut, render, room_world, scene};

/// Sets one switch of a look.
type Setter = fn(&mut Look, bool);

/// The names and setters of the look's switches that change a frame.
const SWITCHES: [(&str, Setter); 7] = [
    ("falloff", |l, on| l.falloff = on),
    ("banding", |l, on| l.banding = on),
    ("contrast", |l, on| l.contrast = on),
    ("sky gradient", |l, on| l.sky_gradient = on),
    ("fog", |l, on| l.fog = on),
    ("palette", |l, on| l.palette = on),
    ("dither", |l, on| l.dither = on),
];

#[test]
fn every_switch_off_gives_the_plain_pipelines_image_and_each_switches_alone() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let world = room_world(&gpu, &lut());
    let targets = Targets::new(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    let plain = render(&gpu, &renderer, &targets, &world, &scene(Look::ON), true);
    let off = render(&gpu, &renderer, &targets, &world, &scene(Look::OFF), false);
    assert!(
        off.image == plain.image,
        "every switch off is not the plain image"
    );
    assert!(off.depth == plain.depth && off.info == plain.info);
    // The same through the present pass, into an offscreen surface.
    let presented = Targets::with_offscreen_surface(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    let plain_shown = render(&gpu, &renderer, &presented, &world, &scene(Look::ON), true);
    let plain_surface = presented
        .read_surface(&gpu)
        .unwrap_or_else(|e| panic!("{e}"));
    let off_shown = render(
        &gpu,
        &renderer,
        &presented,
        &world,
        &scene(Look::OFF),
        false,
    );
    assert!(off_shown.image == plain.image && plain_shown.image == plain.image);
    assert!(
        presented
            .read_surface(&gpu)
            .unwrap_or_else(|e| panic!("{e}"))
            == plain_surface
    );
    assert!(plain_surface == plain.image, "the blit changed the output");

    // Each switch alone: off from all on, and on from all off, changes the
    // image (dither alone needs the palette on).
    let on = render(&gpu, &renderer, &targets, &world, &scene(Look::ON), false).image;
    assert!(on != plain.image);
    for (name, set) in SWITCHES {
        let mut look = Look::ON;
        set(&mut look, false);
        let without = render(&gpu, &renderer, &targets, &world, &scene(look), false);
        assert!(
            without.image != on,
            "switching {name} off alone changed nothing"
        );
        assert!(without.depth == plain.depth && without.info == plain.info);
        let mut look = Look::OFF;
        set(&mut look, true);
        let base = if name == "dither" {
            look.palette = true;
            let mut undithered = look;
            undithered.dither = false;
            render(&gpu, &renderer, &targets, &world, &scene(undithered), false).image
        } else {
            plain.image.clone()
        };
        let with = render(&gpu, &renderer, &targets, &world, &scene(look), false);
        assert!(
            with.image != base,
            "switching {name} on alone changed nothing"
        );
    }

    // Fog of no density is no fog: the same bytes as fog bypassed.
    let mut clear = scene(Look::ON);
    for region in &mut clear.regions {
        region.fog_density = 0.0;
    }
    let with_fog_pass = render(&gpu, &renderer, &targets, &world, &clear, false).image;
    clear.look.fog = false;
    let bypassed = render(&gpu, &renderer, &targets, &world, &clear, false).image;
    assert!(with_fog_pass == bypassed);
}

#[test]
fn reads_follow_the_graph_of_the_last_frame_and_fogs_bypass() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let world = room_world(&gpu, &lut());
    let targets = Targets::new(&gpu, 64, 48).unwrap_or_else(|e| panic!("{e}"));
    let read_both = |look: Look| {
        renderer
            .render(&gpu, &targets, &world, &scene(look))
            .unwrap_or_else(|e| panic!("{e}"));
        let lit = renderer.read_lit(&gpu, &targets);
        let fogged = renderer.read_fogged(&gpu, &targets);
        (
            lit.unwrap_or_else(|e| panic!("{e}")),
            fogged.unwrap_or_else(|e| panic!("{e}")),
        )
    };
    let (lit, fogged) = read_both(Look::ON);
    assert!(lit != fogged, "the room's fog shows");
    // Fog bypassed: `fogged` takes `lit`'s texture, and reads as it.
    let mut look = Look::ON;
    look.fog = false;
    let (lit_off, fogged_off) = read_both(look);
    assert!(fogged_off == lit_off && lit_off == lit);
    // Depth and info read back after any look, the same each time.
    let first = render(&gpu, &renderer, &targets, &world, &scene(Look::ON), false);
    for (palette, fog) in [(true, true), (false, true), (true, false), (false, false)] {
        let look = Look {
            palette,
            fog,
            ..Look::ON
        };
        let frame = render(&gpu, &renderer, &targets, &world, &scene(look), false);
        assert!(frame.depth == first.depth && frame.info == first.info);
    }
    // The plain pipeline has no fog to read.
    renderer
        .render_plain(&gpu, &targets, &world, &scene(Look::ON))
        .unwrap_or_else(|e| panic!("{e}"));
    let error = renderer
        .read_fogged(&gpu, &targets)
        .expect_err("no fog in the plain pipeline");
    assert_eq!(
        error.to_string(),
        "[E-GPU] reading back `fogged`: no pass of the graph that runs draws it"
    );
}

#[test]
fn with_the_palette_on_every_pixel_is_the_palette_colour_the_cpu_table_gives() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let lut = lut();
    let world = room_world(&gpu, &lut);
    let targets = Targets::new(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    let colours: BTreeSet<[u8; 3]> = lut.colours().iter().map(|c| [c.r, c.g, c.b]).collect();
    for dither in [false, true] {
        let look = Look { dither, ..Look::ON };
        // The palette off draws the tone map's bytes into the output: what
        // the palette pass reads.
        let toned = render(
            &gpu,
            &renderer,
            &targets,
            &world,
            &scene(Look {
                palette: false,
                ..look
            }),
            false,
        )
        .image;
        let image = render(&gpu, &renderer, &targets, &world, &scene(look), false).image;
        let mut used = BTreeSet::new();
        for y in 0..H {
            for x in 0..W {
                let [r, g, b, a] = image.pixel(x, y);
                assert_eq!(a, 255);
                assert!(
                    colours.contains(&[r, g, b]),
                    "({x}, {y}) is {:?}, no palette colour",
                    [r, g, b]
                );
                used.insert([r, g, b]);
                // The CPU's cell: each code, dithered as the shader does,
                // shifted down by 3.
                let step = if dither { 8.0 } else { 0.0 };
                let offset = bayer4_offset(x, y) * step;
                let t = toned.pixel(x, y);
                let cell = |code: u8| {
                    let dithered = eng_math::clamp(f32::from(code) + offset, 0.0, 255.0);
                    dithered as usize >> 3
                };
                let index = lut.cell(cell(t[0]), cell(t[1]), cell(t[2]));
                let want = lut.colours()[usize::from(index)];
                assert_eq!(
                    [r, g, b],
                    [want.r, want.g, want.b],
                    "({x}, {y}): toned {t:?}, dither {dither}"
                );
            }
        }
        assert!(used.len() >= 8, "only {} palette colours show", used.len());
    }
}

/// The distinct output colours per region, over the pixels that show a
/// surface.
fn levels_per_region(frame: &Frame) -> BTreeMap<u16, BTreeSet<[u8; 4]>> {
    let mut levels: BTreeMap<u16, BTreeSet<[u8; 4]>> = BTreeMap::new();
    for y in 0..H {
        for x in 0..W {
            let info = Info::unpack(frame.info.uint2(x, y)).expect("info unpacks");
            if info.class != SurfaceClass::None {
                levels
                    .entry(info.region)
                    .or_default()
                    .insert(frame.image.pixel(x, y));
            }
        }
    }
    levels
}

#[test]
fn on_a_flat_material_banding_gives_at_most_32_levels_per_region() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let world = flat_world(&gpu, [180, 160, 140], &lut());
    let targets = Targets::new(&gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    // Falloff, contrast and banding on; no fog or palette to add levels.
    let banded = Look {
        fog: false,
        palette: false,
        ..Look::ON
    };
    let frame = render(&gpu, &renderer, &targets, &world, &scene(banded), false);
    let levels = levels_per_region(&frame);
    assert_eq!(levels.keys().copied().collect::<Vec<_>>(), [0, 1]);
    for (region, colours) in &levels {
        assert!(
            colours.len() <= 32,
            "region {region} shows {} levels",
            colours.len()
        );
    }
    // The room's falloff spans several steps.
    assert!(levels[&0].len() >= 4, "{:?}", levels[&0]);
    // The check can fail: without banding, falloff gives far more.
    let smooth = Look {
        banding: false,
        ..banded
    };
    let frame = render(&gpu, &renderer, &targets, &world, &scene(smooth), false);
    let smooth_levels = levels_per_region(&frame);
    assert!(smooth_levels[&0].len() > 32, "{}", smooth_levels[&0].len());
}

#[test]
fn the_first_person_view_presents_k_by_k_into_an_upscaled_window() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let world = room_world(&gpu, &lut());
    // 161x121 texels, three window pixels each, a pixel's border left over.
    let targets = Targets::upscaled_with_offscreen_surface(&gpu, 485, 365, 3)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(targets.size(), (161, 121));
    let frame = render(&gpu, &renderer, &targets, &world, &scene(Look::ON), false);
    let surface = targets.read_surface(&gpu).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((surface.width, surface.height), (485, 365));
    for y in 0..365 {
        for x in 0..485 {
            let inside = (1..484).contains(&x) && (1..364).contains(&y);
            let want = if inside {
                frame.image.pixel((x - 1) / 3, (y - 1) / 3)
            } else {
                [0, 0, 0, 255]
            };
            assert_eq!(surface.pixel(x, y), want, "window pixel ({x}, {y})");
        }
    }
    // Same as a 1:1 frame of the targets' size.
    let alone = Targets::new(&gpu, 161, 121).unwrap_or_else(|e| panic!("{e}"));
    let one = render(&gpu, &renderer, &alone, &world, &scene(Look::ON), false);
    assert!(one.image == frame.image);
}

#[test]
fn a_frame_of_every_look_costs_what_its_passes_draw() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let world = room_world(&gpu, &lut());
    let counts = |targets: &Targets, look: Look| {
        let c = renderer
            .render(&gpu, targets, &world, &scene(look))
            .unwrap_or_else(|e| panic!("{e}"));
        (
            c.draws,
            c.pipeline_switches,
            c.bind_group_switches,
            c.upload_bytes,
        )
    };
    let offscreen = Targets::new(&gpu, 32, 24).unwrap_or_else(|e| panic!("{e}"));
    let presented = Targets::with_offscreen_surface(&gpu, 32, 24).unwrap_or_else(|e| panic!("{e}"));
    let uploads = 624 + 2 * 16;
    // Two chunk draws, then light, sky, fog, the tone map and the palette:
    // opaque and the palette bind the frame and the scene, the palette and
    // the rest their reads too.
    assert_eq!(counts(&offscreen, Look::ON), (7, 6, 13, uploads));
    assert_eq!(counts(&presented, Look::ON), (8, 7, 14, uploads));
    // Each bypassed pass costs nothing.
    let no_fog = Look {
        fog: false,
        ..Look::ON
    };
    assert_eq!(counts(&offscreen, no_fog), (6, 5, 11, uploads));
    assert_eq!(counts(&offscreen, Look::OFF), (5, 4, 8, uploads));
    // With every texture in use: albedo and `fogged` share one, as their
    // lives do not overlap.
    assert_eq!(offscreen.transient_textures(), 5);
}

#[test]
fn a_look_or_scene_the_frame_cannot_draw_is_refused_before_the_gpu() {
    let Some(gpu) = gpu() else { return };
    let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let texels = room::texels();
    let textures = room::textures(&texels);
    let materials = room::materials();
    let chunks = room::world_chunks([0.0; 3]);
    let mesh = WorldMesh {
        label: "no palette",
        chunks: &chunks,
        materials: &materials,
        textures: &textures,
    };
    let bare = GpuWorld::load(&gpu, &mesh).unwrap_or_else(|e| panic!("{e}"));
    assert!(!bare.has_palette());
    let with = GpuWorld::load_with_palette(&gpu, &mesh, &lut()).unwrap_or_else(|e| panic!("{e}"));
    assert!(with.has_palette());
    // The table's eight layers follow the room's five textures.
    let slots: Vec<_> = (5..13).map(|i| with.texture_slot(i)).collect();
    for (n, slot) in (0u32..).zip(&slots) {
        let slot = slot.expect("a slot");
        assert_eq!(
            (slot.side, slot.tag, slot.layer),
            (64, ColourTag::Srgb, 5 + n)
        );
    }
    let targets = Targets::new(&gpu, 8, 8).unwrap_or_else(|e| panic!("{e}"));
    let refusal = |world: &GpuWorld, scene: &FirstPersonScene| {
        renderer
            .render(&gpu, &targets, world, scene)
            .expect_err("the frame should be refused")
            .to_string()
    };
    assert_eq!(
        refusal(&bare, &scene(Look::ON)),
        "[E-SCENE] the look's palette is on, but the world was loaded without a palette table \
         (GpuWorld::load_with_palette)"
    );
    // Without the palette, or plain, it draws.
    let no_palette = Look {
        palette: false,
        ..Look::ON
    };
    renderer
        .render(&gpu, &targets, &bare, &scene(no_palette))
        .unwrap_or_else(|e| panic!("{e}"));
    renderer
        .render_plain(&gpu, &targets, &bare, &scene(Look::ON))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut bad = scene(Look::ON);
    bad.sky.horizon.g = -0.5;
    assert_eq!(
        refusal(&with, &bad),
        "[E-SCENE] the sky's colours must not be negative"
    );
    let mut bad = scene(Look::ON);
    bad.regions[1].fog_density = f32::NAN;
    assert_eq!(
        refusal(&with, &bad),
        "[E-SCENE] region 1's fog colour and density must be finite and not negative"
    );
    let mut bad = scene(Look::ON);
    bad.regions[0].fog.b = -1.0;
    assert!(refusal(&with, &bad).starts_with("[E-SCENE] region 0's fog"));
    let mut bad = scene(Look::ON);
    bad.regions = vec![RegionLook::DEFAULT; 17];
    assert_eq!(
        refusal(&with, &bad),
        "[E-SCENE] 17 region looks; a frame carries at most 16"
    );
    let mut bad = scene(Look::ON);
    bad.ev100 = Ev100(200.0);
    assert_eq!(
        refusal(&with, &bad),
        "[E-SCENE] EV100 200 gives no exposure a frame can use"
    );
    // Refused before any GPU work, a frame leaves the last one readable.
    assert!(renderer.read_depth(&gpu, &targets).is_ok());
}
