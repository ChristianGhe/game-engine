//! The room in first person on a real adapter (rung 2, T12: "a
//! first-person shot renders"): the room's world loaded as `shot --map
//! room` loads it, frames drawn from the sim's states, and what each pixel
//! shows read back from `info` (region, surface class, object id) and the
//! output. Without an adapter each test skips, unless `ENG_REQUIRE_GPU=1`
//! (CI) makes it fail.
//!
//! From spawn 0 at tick 60, looking east: floor along the bottom, ceiling
//! along the top, the marker in front of the pillar, the crate hidden
//! behind it, the barrel on the platform and the alcove's green fog. From
//! spawn 0 looking up into the skylight: the sky.

use eng_core::{PlayerCommand, Tick};
use eng_gpu::{Gpu, GpuError, GpuOptions, Image, Targets, Texels};
use eng_render::{FirstPersonRenderer, Info, SurfaceClass};
use eng_sim::{Sim, SimConfig};
use game_testbed_client::room;

/// The shots' size.
const W: u32 = 320;
const H: u32 = 240;

/// The things' ids: their numbers in entity order.
const MARKER: u32 = 0;
const CRATE: u32 = 1;
const BARREL: u32 = 2;

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

/// The room with one player after `ticks` ticks of `command` (its tick
/// set for each), as `shot --map room --seed 0` runs it.
fn room_after(ticks: u64, command: PlayerCommand) -> Sim {
    let config = SimConfig {
        tick_rate: game_testbed_sim::TICK_RATE,
        players: 1,
        seed: 0,
    };
    let mut sim = game_testbed_sim::room::build(config).unwrap();
    for tick in 1..=ticks {
        sim.step(&[PlayerCommand {
            tick: Tick(tick),
            ..command
        }])
        .unwrap();
    }
    sim
}

/// One frame of the room's view of `sim`, at alpha 1, as `shot` draws it.
struct Frame {
    image: Image,
    info: Texels,
}

impl Frame {
    fn info(&self, x: u32, y: u32) -> Info {
        Info::unpack(self.info.uint2(x, y)).expect("info holds only valid words")
    }

    fn rgb(&self, x: u32, y: u32) -> [u8; 3] {
        let at = ((y * self.image.width + x) * 4) as usize;
        let p = &self.image.rgba8[at..at + 3];
        [p[0], p[1], p[2]]
    }

    /// Every pixel where `keep` holds of its `info`.
    fn pixels(&self, keep: impl Fn(Info) -> bool) -> Vec<(u32, u32)> {
        let mut found = Vec::new();
        for y in 0..H {
            for x in 0..W {
                if keep(self.info(x, y)) {
                    found.push((x, y));
                }
            }
        }
        found
    }
}

fn render(gpu: &Gpu, sim: &Sim) -> Frame {
    // As `shot` loads it: with its palette, which the look needs.
    let built = room::world(sim);
    assert!(built.palette.is_some());
    let world = built.load(gpu).unwrap_or_else(|e| panic!("{e}"));
    assert!(world.has_palette());
    let renderer = FirstPersonRenderer::new(gpu).unwrap_or_else(|e| panic!("{e}"));
    let targets = Targets::new(gpu, W, H).unwrap_or_else(|e| panic!("{e}"));
    let scene = room::scene(&eng_cli::FirstPersonFrame {
        previous: sim,
        current: sim,
        alpha: 1.0,
        width: W,
        height: H,
    });
    renderer
        .render(gpu, &targets, &world, &scene)
        .unwrap_or_else(|e| panic!("{e}"));
    Frame {
        image: targets.read_output(gpu).unwrap_or_else(|e| panic!("{e}")),
        info: renderer
            .read_info(gpu, &targets)
            .unwrap_or_else(|e| panic!("{e}")),
    }
}

fn sprite(id: u32) -> impl Fn(Info) -> bool {
    move |info| info.class == SurfaceClass::Sprite && info.id == id
}

#[test]
fn from_spawn_0_the_floor_is_below_the_ceiling_above_the_marker_seen_and_the_crate_hidden() {
    let Some(gpu) = gpu() else { return };
    let frame = render(&gpu, &room_after(60, PlayerCommand::idle(Tick(0))));

    // The bottom 16 rows are floor, but where the marker stands on it.
    for y in H - 16..H {
        for x in 0..W {
            let info = frame.info(x, y);
            let marker = sprite(MARKER)(info);
            assert!(
                info.class == SurfaceClass::Floor || marker,
                "({x}, {y}): {info:?}"
            );
        }
    }
    // The top 16 rows are ceiling, or the skylight (region 3: its shaft at
    // the top left, or sky), and mostly ceiling.
    let top: Vec<Info> = (0..16)
        .flat_map(|y| (0..W).map(move |x| (x, y)))
        .map(|(x, y)| frame.info(x, y))
        .collect();
    let skylight = |i: &Info| i.region == 3 || i.class == SurfaceClass::None;
    assert!(
        top.iter()
            .all(|i| i.class == SurfaceClass::Ceiling || skylight(i))
    );
    let ceiling = top
        .iter()
        .filter(|i| i.class == SurfaceClass::Ceiling)
        .count();
    assert!(ceiling * 4 >= top.len() * 3, "{ceiling} of {}", top.len());

    // The marker, 2.5 m ahead in the hall, low in the middle: rotation 7
    // at second 1, its yellow right side on the image's left and its red
    // front on the right.
    let marker = frame.pixels(sprite(MARKER));
    assert!(marker.len() >= 800, "{} marker pixels", marker.len());
    assert!(
        marker
            .iter()
            .all(|&(x, y)| (130..190).contains(&x) && y >= 150)
    );
    assert!(marker.iter().all(|&(x, y)| frame.info(x, y).region == 0));
    let (yellow, red) = (frame.rgb(150, 200), frame.rgb(170, 210));
    assert!(
        yellow[0] > yellow[2] + 40 && yellow[1] > yellow[2] + 40,
        "{yellow:?}"
    );
    assert!(red[0] > red[1] + 30 && red[0] > red[2] + 30, "{red:?}");

    // The crate stands behind the pillar: not one pixel.
    assert_eq!(frame.pixels(sprite(CRATE)), []);
    // The barrel, on the platform (region 1), shows to the left.
    let barrel = frame.pixels(sprite(BARREL));
    assert!(barrel.len() >= 50, "{} barrel pixels", barrel.len());
    assert!(
        barrel
            .iter()
            .all(|&(x, y)| x < 160 && frame.info(x, y).region == 1)
    );

    // The alcove (region 2) through its opening, right of the pillar, deep
    // in its green fog.
    let alcove = frame.pixels(|info| info.region == 2);
    assert!(alcove.len() >= 400, "{} alcove pixels", alcove.len());
    for &(x, y) in &alcove {
        let [r, g, b] = frame.rgb(x, y);
        assert!(g > r + 20 && g > b + 10, "({x}, {y}): {:?}", [r, g, b]);
    }
    // The pillar's west face fills the middle column above the marker.
    let pillar = frame.info(W / 2, 100);
    assert_eq!(pillar.class, SurfaceClass::WallFacingX, "{pillar:?}");
}

#[test]
fn looking_up_into_the_skylight_shows_the_sky() {
    let Some(gpu) = gpu() else { return };
    // Spawn 0, turned 45 degrees toward the skylight and 40 up.
    let look = PlayerCommand {
        view_yaw: 0x2000,
        view_pitch: 7_282,
        ..PlayerCommand::idle(Tick(0))
    };
    let frame = render(&gpu, &room_after(1, look));
    let sky = frame.pixels(|info| info.class == SurfaceClass::None);
    assert!(sky.len() >= 2_000, "{} sky pixels", sky.len());
    assert_eq!(frame.info(W / 2, H / 2).class, SurfaceClass::None);
    for &(x, y) in &sky {
        let [r, g, b] = frame.rgb(x, y);
        assert!(b > r + 40 && b >= g, "({x}, {y}): {:?}", [r, g, b]);
    }
    // The shaft's far faces, lit as the skylight's region (3).
    let shaft = frame.pixels(|info| info.region == 3 && info.class != SurfaceClass::None);
    assert!(!shaft.is_empty());
}
