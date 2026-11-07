//! The room's first-person view (rung 2, T12): its world, built once from
//! the sim's constants, and each frame's scene, from the states. Both read
//! the sim and never write to it; nothing here is simulation.
//!
//! - **The world** ([`world`]): the chunks of [`geometry`], the five
//!   surface textures and the things' sprite frames, all made by code, and
//!   the palette from `palettes/room.gpl` ([`crate::palette`]). Materials
//!   0 to 4 are the surfaces ([`Surface`]); then each kind of thing's
//!   frames in turn ([`sprites::look`]), alpha-tested.
//! - **The camera** ([`camera`]): player 0's, from its `MoveState` (the
//!   hull's centre, interpolated at the frame's alpha) with the eye
//!   [`EYE_HEIGHT`] above its feet, and the angles of its `testbed.View`
//!   (the current state's: `play`'s live view will replace them), with a
//!   field of view of [`FOV_Y`] and eng-render's [`NEAR_PLANE`].
//! - **Regions** ([`regions`]): each region's light level from the room's
//!   map, and its fog from [`FOG`]: region 2, the alcove, is green.
//! - **The sky** ([`SKY`]) shows through the skylight, where nothing is
//!   drawn; the exposure is [`EV100`].
//! - **Sprites** ([`sprites()`]): one per `testbed.Thing`, numbered in entity
//!   order from 0, in the region its feet stand in.

pub mod geometry;

use eng_cli::{FirstPersonFrame, FirstPersonWorld, WorldTexture};
use eng_collision_grid::Grid;
use eng_core::PlayerSlot;
use eng_math::{BAngle, Ev100, WorldPos};
use eng_render::{
    ColourTag, FirstPersonCamera, FirstPersonScene, LinearRgb, Look, MAX_REGIONS, Material,
    NEAR_PLANE, RegionLook, Sky, Sprite,
};
use eng_sim::Sim;
use game_testbed_sim::room::layout::EYE_HEIGHT;
use game_testbed_sim::room::{RoomMap, Thing, View};
use kit_fps::{MoveConfig, MoveState};

use crate::sprites::{self, KINDS};
use crate::textures::Surface;

/// The vertical field of view: 2 atan(3/4), 73.74 degrees, so a 4:3 image
/// (the shots' 320x240) shows 90 degrees across, as Doom does; wider
/// images show more across. The 0.1 m near plane stays clear of the walls
/// up to an aspect of about 2.9 at it (eng-render's `NEAR_PLANE`).
pub const FOV_Y: BAngle = BAngle(879_750_026);

/// The camera's exposure: EV100 5 maps 38.4 nits to 1 before the tone
/// curve. The hall (light 160, about 48 lux) then shows its textures as
/// dark mid-tones, Doom's mood, and the floor under the skylight (320 lux)
/// near white.
pub const EV100: Ev100 = Ev100(5.0);

/// The sky through the skylight, in nits: a pale blue at the horizon to a
/// deeper one overhead.
pub const SKY: Sky = Sky {
    horizon: LinearRgb {
        r: 20.0,
        g: 28.0,
        b: 40.0,
    },
    zenith: LinearRgb {
        r: 4.0,
        g: 10.0,
        b: 30.0,
    },
};

/// Each region's fog, by region index: its colour in nits and its density
/// per meter. Region 2, the alcove, is green and thick enough to show from
/// across the hall; the rest have none.
pub const FOG: [(u16, LinearRgb, f32); 1] = [(
    2,
    LinearRgb {
        r: 0.5,
        g: 3.0,
        b: 0.9,
    },
    0.15,
)];

/// The room's world, from its sim (any tick: it reads the constants).
///
/// # Panics
/// If the sim is not the room's: it has no `eng.collision.grid.Grid`,
/// `testbed.RoomMap` or `kit_fps.MoveConfig` constant.
pub fn world(sim: &Sim) -> FirstPersonWorld {
    let world = sim.world();
    let grid = world.constant::<Grid>();
    let map = world.constant::<RoomMap>();
    let hull = world.constant::<MoveConfig>().hull;
    let hull_height = 2.0 * f64::from(hull.half_extents.z);
    let mut textures: Vec<WorldTexture> = Surface::ALL
        .iter()
        .map(|surface| WorldTexture {
            side: crate::textures::SIDE as u32,
            tag: ColourTag::Srgb,
            rgba8: surface.texture(),
        })
        .collect();
    let mut materials: Vec<Material> = Surface::ALL
        .iter()
        .map(|surface| Material {
            texture: surface.material(),
            alpha_test: false,
        })
        .collect();
    for kind in KINDS {
        debug_assert_eq!(sprites::look(kind).first_material as usize, materials.len());
        for rgba8 in sprites::frames(kind) {
            materials.push(Material {
                texture: textures.len() as u32,
                alpha_test: true,
            });
            textures.push(WorldTexture {
                side: sprites::SIDE as u32,
                tag: ColourTag::Srgb,
                rgba8,
            });
        }
    }
    FirstPersonWorld {
        label: "testbed room",
        chunks: geometry::chunks(grid, map, hull_height),
        materials,
        textures,
        palette: Some(crate::palette::room()),
    }
}

/// One frame of the room in first person: player 0's view with every look
/// feature on.
pub fn scene(frame: &FirstPersonFrame<'_>) -> FirstPersonScene {
    FirstPersonScene {
        camera: camera(frame),
        sky: SKY,
        ev100: EV100,
        look: Look::ON,
        regions: regions(frame.current.world().constant::<RoomMap>()),
        sprites: sprites(frame),
    }
}

/// From `a` at `t` = 0 to `b` at `t` = 1, exact at both ends.
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a * (1.0 - t) + b * t
}

fn lerp_pos(a: WorldPos, b: WorldPos, t: f64) -> WorldPos {
    WorldPos::new(lerp(a.x, b.x, t), lerp(a.y, b.y, t), lerp(a.z, b.z, t))
}

/// Player 0's camera: its eye between the two states at the frame's alpha,
/// [`EYE_HEIGHT`] above its feet, and its current view angles. A sim
/// without player 0 looks from the map's first spawn.
pub fn camera(frame: &FirstPersonFrame<'_>) -> FirstPersonCamera {
    let world = frame.current.world();
    let half_height = f64::from(world.constant::<MoveConfig>().hull.half_extents.z);
    let player = world
        .storage::<PlayerSlot>()
        .iter()
        .find(|(_, slot)| slot.0 == 0)
        .map(|(e, _)| e);
    let current = player.and_then(|e| world.get::<MoveState>(e));
    let (eye, view) = match current {
        Some(state) => {
            let previous = player
                .and_then(|e| frame.previous.world().get::<MoveState>(e))
                .map_or(state.pos, |s| s.pos);
            let centre = lerp_pos(previous, state.pos, frame.alpha);
            let view = player
                .and_then(|e| world.get::<View>(e))
                .copied()
                .unwrap_or(View {
                    yaw: BAngle::ZERO,
                    pitch: BAngle::ZERO,
                });
            (
                WorldPos::new(centre.x, centre.y, centre.z - half_height + EYE_HEIGHT),
                view,
            )
        }
        None => {
            let spawn = world.constant::<RoomMap>().spawns[0];
            let feet = spawn.feet;
            (
                WorldPos::new(feet.x, feet.y, feet.z + EYE_HEIGHT),
                View {
                    yaw: spawn.yaw,
                    pitch: BAngle::ZERO,
                },
            )
        }
    };
    FirstPersonCamera {
        eye,
        yaw: view.yaw,
        pitch: view.pitch,
        fov_y: FOV_Y,
        near: NEAR_PLANE,
    }
}

/// Each of the map's regions as it looks, at most [`MAX_REGIONS`]: its
/// light level, and its fog from [`FOG`] or none.
pub fn regions(map: &RoomMap) -> Vec<RegionLook> {
    map.regions
        .iter()
        .take(MAX_REGIONS)
        .enumerate()
        .map(|(index, region)| {
            let fog = FOG.iter().find(|(r, _, _)| usize::from(*r) == index);
            RegionLook {
                light: region.light,
                fog: fog.map_or(RegionLook::DEFAULT.fog, |f| f.1),
                fog_density: fog.map_or(0.0, |f| f.2),
            }
        })
        .collect()
}

/// One sprite per thing, numbered in entity order from 0: its feet between
/// the two states at the frame's alpha, its current facing, and the region
/// its feet stand in (0 outside them all).
pub fn sprites(frame: &FirstPersonFrame<'_>) -> Vec<Sprite> {
    let world = frame.current.world();
    let map = world.constant::<RoomMap>();
    let previous = frame.previous.world();
    world
        .storage::<Thing>()
        .iter()
        .enumerate()
        .map(|(number, (e, thing))| {
            let from = previous.get::<Thing>(e).map_or(thing.feet, |t| t.feet);
            let feet = lerp_pos(from, thing.feet, frame.alpha);
            let look = sprites::look(thing.kind);
            let (first, frames) = (look.first_material, look.sheet.frames());
            Sprite {
                id: number as u32,
                region: map
                    .region_at(feet.x, feet.y)
                    .map_or(0, |r| u16::try_from(r).unwrap_or(u16::MAX)),
                feet,
                facing: thing.facing,
                width: look.size,
                height: look.size,
                sheet: look.sheet,
                materials: core::array::from_fn(|k| {
                    first + sheet_frame(k as u32, u32::from(frames))
                }),
            }
        })
        .collect()
}

/// The frame of a sheet of `frames` drawn for rotation `k`: `k` itself, or
/// on a five-rotation sheet `8 - k` above 4, the frame mirrored there
/// (eng-render's `RotationSheet::frame`). eng-render reads only a five
/// sheet's first five materials; the last three name their sources too.
fn sheet_frame(k: u32, frames: u32) -> u32 {
    if k < frames { k } else { 8 - k }
}

#[cfg(test)]
mod tests;
