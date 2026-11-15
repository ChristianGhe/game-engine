//! Planted render bugs (plan addition A1: "a planted 'not drawn' bug and a
//! 'drawn black' bug each change it"), drawn in-process exactly as `shot
//! --map room --at-tick 0` draws the room-spawn shot, which the first test
//! holds to the binary's own record. The objects asked for always come
//! from the unchanged extraction, as a renderer bug would leave them; only
//! what reaches the GPU is planted:
//! - **a thing not drawn**: the marker's sprite left out of the frame;
//! - **a wall face not drawn**: the triangles of the most-seen face facing
//!   X, the pillar's west face, left out of the world;
//! - **a thing drawn black**: the marker's frames drawn from copies of
//!   their textures with red, green and blue at 0, so its albedo is 0.
//!
//! Each fails the check against the unchanged frame, as against a
//! reference, within room-spawn's tolerances, at the field it should.

use std::sync::{Mutex, PoisonError};

use eng_cli::frame_record::{
    self, AskedObject, FrameEntry, FrameRecord, ObjectKey, ObjectRecord, RecordView,
};
use eng_cli::{FirstPersonFrame, FirstPersonWorld, WorldTexture};
use eng_gpu::{Gpu, GpuError, GpuOptions, Targets};
use eng_render::{FirstPersonRenderer, FirstPersonScene, Material, SurfaceClass};
use eng_sim::{Sim, SimConfig};
use game_testbed_client::room;

use super::harness;

const W: u32 = 320;
const H: u32 = 240;

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

/// The room at tick 0, one player, seed 1: the room-spawn shot's state.
fn sim() -> Sim {
    let config = SimConfig {
        tick_rate: game_testbed_sim::TICK_RATE,
        players: 1,
        seed: 1,
    };
    game_testbed_sim::room::build(config).unwrap()
}

/// One room, so one in-process device, for every test here: used by one
/// test at a time and never dropped. Debian's lavapipe (Mesa 25.0.7,
/// arm64) crashed with SIGSEGV, or drew a frame wrong, when each test
/// opened a device of its own, at once or one after another.
static ROOM: Mutex<Option<Room>> = Mutex::new(None);

/// Runs `test` with the room, opening it first if no test has; without an
/// adapter it skips (unless CI requires one).
fn with_room(test: impl FnOnce(&Room)) {
    // A test that failed holding it poisons it; the next may go on.
    let mut room = ROOM.lock().unwrap_or_else(PoisonError::into_inner);
    if room.is_none() {
        *room = Room::new();
    }
    if let Some(room) = room.as_ref() {
        test(room);
    }
}

/// The room's GPU side and its unchanged extraction at tick 0.
struct Room {
    gpu: Gpu,
    renderer: FirstPersonRenderer,
    targets: Targets,
    built: FirstPersonWorld,
    scene: FirstPersonScene,
    asked: Vec<AskedObject>,
}

impl Room {
    fn new() -> Option<Self> {
        let gpu = gpu()?;
        let renderer = ok(FirstPersonRenderer::new(&gpu));
        let targets = ok(Targets::new(&gpu, W, H));
        let sim = sim();
        let frame = FirstPersonFrame {
            previous: &sim,
            current: &sim,
            alpha: 1.0,
            width: W,
            height: H,
        };
        let built = room::world(&sim);
        let scene = room::scene(&frame);
        let world = frame_record::world_objects(&built);
        let asked = frame_record::asked_objects(&world, &scene.sprites, &room::names(&frame));
        Some(Self {
            gpu,
            renderer,
            targets,
            built,
            scene,
            asked,
        })
    }

    /// `built` drawn as `scene` sees it, recorded as `shot` records it,
    /// with the unchanged extraction's objects.
    fn record(&self, built: &FirstPersonWorld, scene: &FirstPersonScene) -> FrameEntry {
        let (gpu, targets, renderer) = (&self.gpu, &self.targets, &self.renderer);
        let world = ok(built.load(gpu));
        ok(renderer.render(gpu, targets, &world, scene));
        let image = ok(targets.read_output(gpu));
        let info = ok(renderer.read_info(gpu, targets));
        let depth = ok(renderer.read_depth(gpu, targets));
        let camera = &scene.camera;
        let asked = &self.asked;
        ok(frame_record::first_person_entry(
            0, camera, asked, &info, &depth, &image,
        ))
    }

    fn baseline(&self) -> FrameEntry {
        self.record(&self.built, &self.scene)
    }

    fn key_named(&self, name: &str) -> ObjectKey {
        let found = self.asked.iter().find(|o| o.name.as_deref() == Some(name));
        found.unwrap_or_else(|| panic!("no {name}")).key
    }
}

fn ok<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|e| panic!("{e}"))
}

fn object(frame: &FrameEntry, key: ObjectKey) -> &ObjectRecord {
    frame.objects.iter().find(|o| o.key == key).unwrap()
}

/// What the frame-record check says of `bug` held to `base` as to a
/// reference record off the pinned lavapipe: the failures within
/// room-spawn's tolerances (`frame-tolerances.toml`).
fn gate(base: &FrameEntry, bug: &FrameEntry) -> Vec<String> {
    let tolerances = harness::tolerances(&harness::shots()).for_shot("room-spawn");
    let record = |entry: &FrameEntry| FrameRecord {
        view: RecordView::FirstPerson,
        width: W,
        height: H,
        frames: vec![entry.clone()],
    };
    record(base).compare(&record(bug), &tolerances).failures
}

/// Whether one of `failures` is at `field` of the object `key` of `frame`.
fn fails_at(failures: &[String], frame: &FrameEntry, key: ObjectKey, field: &str) -> bool {
    let index = frame.objects.iter().position(|o| o.key == key).unwrap();
    let at = format!("/frames/0/objects/{index}/{field}:");
    failures.iter().any(|f| f.starts_with(&at))
}

/// The keys of the objects whose `field` differs between the two.
fn changed<T: PartialEq>(
    base: &FrameEntry,
    bug: &FrameEntry,
    field: impl Fn(&ObjectRecord) -> T,
) -> Vec<ObjectKey> {
    base.objects
        .iter()
        .zip(&bug.objects)
        .filter(|(a, b)| field(a) != field(b))
        .map(|(a, _)| a.key)
        .collect()
}

#[test]
fn the_in_process_frame_is_the_room_spawn_shots() {
    with_room(|room| {
        let base = room.baseline();
        let dir = harness::scratch("in-process");
        let args = ["shot", "--map", "room", "--at-tick", "0", "--seed", "1"];
        let more = ["--size", "320x240", "--out", "s.png"];
        // In this test's own environment, as the in-process GPU opened, so
        // that lavapipe's vector width is the same on both sides.
        let out = harness::probe_as_is(&dir, &[&args[..], &more[..]].concat()).unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", harness::text(&out));
        let shot = harness::read_record(&dir.join("s.png.frame.json"));
        assert_eq!(shot.frames, std::slice::from_ref(&base));
        // What the room-spawn shot is known to show: the marker in front of
        // the pillar, the barrel on the platform, the crate hidden.
        let marker = object(&base, room.key_named("marker"));
        assert!(marker.pixels >= 50, "{marker:?}");
        assert!(object(&base, room.key_named("barrel")).pixels > 0);
        assert_eq!(object(&base, room.key_named("crate")).pixels, 0);
        assert!(base.unlisted.is_empty() && base.invalid_info == 0);
    });
}

#[test]
fn a_thing_not_drawn_has_no_pixels_and_shows_what_was_behind_it() {
    with_room(|room| {
        let base = room.baseline();
        let marker = room.key_named("marker");
        let mut scene = room.scene.clone();
        scene.sprites.retain(|s| s.id != marker.id);
        let bug = room.record(&room.built, &scene);
        // The check fails it at the marker's pixels.
        let failures = gate(&base, &bug);
        assert!(fails_at(&failures, &base, marker, "pixels"), "{failures:?}");
        let (was, now) = (object(&base, marker), object(&bug, marker));
        assert!(was.pixels >= 50, "{was:?}");
        assert_eq!(
            (now.pixels, now.screen_box, now.luminance, now.contrast),
            (0, None, None, None)
        );
        // Its pixels went to what stood behind it, and only there.
        let others = || (bug.objects.iter().zip(&base.objects)).filter(|(o, _)| o.key != marker);
        let kept = others().all(|(o, b)| o.pixels >= b.pixels);
        assert!(kept, "nothing else lost pixels");
        let gained: Vec<_> = others()
            .filter(|(o, b)| o.pixels > b.pixels)
            .map(|(o, b)| (o.key, o.pixels - b.pixels))
            .collect();
        eprintln!(
            "marker not drawn: its {} pixels went to {gained:?}",
            was.pixels
        );
        assert_eq!(gained.iter().map(|(_, n)| n).sum::<u64>(), was.pixels);
        let sprite = usize::from(SurfaceClass::Sprite.code());
        let (base_bands, bug_bands) = (base.classes.unwrap(), bug.classes.unwrap());
        let lost: u64 =
            base_bands[sprite].iter().sum::<u64>() - bug_bands[sprite].iter().sum::<u64>();
        assert_eq!(lost, was.pixels);
        assert_ne!(base.histogram, bug.histogram);
        // A probe that saw the marker sees something else; the rest see what
        // they saw (in room-spawn no probe falls on it).
        for (was, now) in base.probes.iter().zip(&bug.probes) {
            if was.key == marker {
                assert_ne!(now.key, marker);
            } else {
                assert_eq!((now.key, now.depth), (was.key, was.depth));
            }
        }
    });
}

#[test]
fn a_wall_face_not_drawn_has_no_pixels_and_reveals_the_crate() {
    with_room(|room| {
        let base = room.baseline();
        let face = base
            .objects
            .iter()
            .filter(|o| o.key.class == SurfaceClass::WallFacingX)
            .max_by_key(|o| o.pixels)
            .unwrap()
            .key;
        let mut built = room.built.clone();
        for chunk in &mut built.chunks {
            let vertices = &chunk.vertices;
            let kept: Vec<u32> = chunk
                .indices
                .chunks(3)
                .filter(|t| {
                    let info = vertices[t[0] as usize].info;
                    (info.class, info.id) != (face.class, face.id)
                })
                .flatten()
                .copied()
                .collect();
            chunk.indices = kept;
        }
        let bug = room.record(&built, &room.scene);
        // The check fails it at the face's pixels and at the crate's.
        let failures = gate(&base, &bug);
        let crate_ = room.key_named("crate");
        assert!(fails_at(&failures, &base, face, "pixels"), "{failures:?}");
        assert!(fails_at(&failures, &base, crate_, "pixels"), "{failures:?}");
        assert!(object(&base, face).pixels > 1000);
        assert_eq!(object(&bug, face).pixels, 0);
        assert_eq!(object(&bug, face).screen_box, None);
        // Through the pillar's missing face, the crate behind it shows.
        eprintln!(
            "{face:?} not drawn: its {} pixels gone, the crate's {} -> {}",
            object(&base, face).pixels,
            object(&base, crate_).pixels,
            object(&bug, crate_).pixels
        );
        assert_eq!(object(&base, crate_).pixels, 0);
        assert!(
            object(&bug, crate_).pixels > 0,
            "{:?}",
            object(&bug, crate_)
        );
        assert!(bug.unlisted.is_empty());
        assert_ne!(base.histogram, bug.histogram);
    });
}

#[test]
fn a_thing_drawn_black_keeps_its_pixels_and_loses_its_light() {
    with_room(|room| {
        let base = room.baseline();
        let marker = room.key_named("marker");
        let mut built = room.built.clone();
        let mut scene = room.scene.clone();
        let sprite = scene.sprites.iter_mut().find(|s| s.id == marker.id);
        let sprite = sprite.unwrap();
        for material in &mut sprite.materials {
            let source = built.materials[*material as usize];
            let texture = &built.textures[source.texture as usize];
            let mut black = texture.rgba8.clone();
            for texel in black.as_chunks_mut::<4>().0 {
                texel[..3].fill(0);
            }
            built.textures.push(WorldTexture {
                side: texture.side,
                tag: texture.tag,
                rgba8: black,
            });
            built.materials.push(Material {
                texture: (built.textures.len() - 1) as u32,
                alpha_test: source.alpha_test,
            });
            *material = (built.materials.len() - 1) as u32;
        }
        let bug = room.record(&built, &scene);
        // The check fails it at the marker's luminance, and not its pixels.
        let failures = gate(&base, &bug);
        let at = |field: &str| fails_at(&failures, &base, marker, field);
        assert!(at("luminance") && !at("pixels"), "{failures:?}");
        let (was, now) = (object(&base, marker), object(&bug, marker));
        // The same silhouette, its surroundings untouched, and dark.
        assert_eq!((now.pixels, now.screen_box), (was.pixels, was.screen_box));
        assert_eq!(now.surround_luminance, was.surround_luminance);
        let (dark, lit) = (now.luminance.unwrap(), was.luminance.unwrap());
        eprintln!(
            "marker drawn black: luminance {lit} -> {dark}, contrast {:?} -> {:?}, \
             image {} -> {}",
            was.contrast, now.contrast, base.luminance, bug.luminance
        );
        assert!(dark < lit / 2.0, "{dark} against {lit}");
        assert_ne!(now.contrast, was.contrast);
        // Only the marker's light changed: every object's pixels are as they
        // were, and so are the probes' depths and keys.
        assert!(changed(&base, &bug, |o| o.pixels).is_empty());
        assert_eq!(changed(&base, &bug, |o| o.luminance), [marker]);
        assert_eq!(base.probes, bug.probes);
        assert_ne!(base.histogram, bug.histogram);
        assert!(bug.luminance < base.luminance);
    });
}
