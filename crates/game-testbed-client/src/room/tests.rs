//! The room's view without a GPU: the world's materials and textures, the
//! camera from player 0, the regions' looks, and one sprite per thing.

use eng_cli::FirstPersonFrame;
use eng_core::{PlayerCommand, Tick};
use eng_math::{BAngle, WorldPos, sin_cos_f64};
use eng_render::{ColourTag, LinearRgb, Look, NEAR_PLANE, RotationSheet};
use eng_sim::{Sim, SimConfig};
use game_testbed_sim::room::ThingKind;

use super::{EV100, FOV_Y, SKY, camera, regions, scene, sprites, world};

fn room(players: u8) -> Sim {
    let config = SimConfig {
        tick_rate: game_testbed_sim::TICK_RATE,
        players,
        seed: 3,
    };
    game_testbed_sim::room::build(config).unwrap()
}

fn frame<'a>(previous: &'a Sim, current: &'a Sim, alpha: f64) -> FirstPersonFrame<'a> {
    FirstPersonFrame {
        previous,
        current,
        alpha,
        width: 320,
        height: 240,
    }
}

#[test]
fn the_world_has_the_surfaces_then_each_things_frames_and_the_palette() {
    let sim = room(1);
    let built = world(&sim);
    assert_eq!(built.label, "testbed room");
    assert_eq!(built.chunks.len(), 1);
    // Five surfaces, then the marker's 8 frames and the crate's and the
    // barrel's 5 each.
    assert_eq!(built.materials.len(), 23);
    assert_eq!(built.textures.len(), 23);
    for (i, material) in built.materials.iter().enumerate() {
        assert_eq!(material.texture as usize, i);
        assert_eq!(material.alpha_test, i >= 5, "material {i}");
    }
    for texture in &built.textures {
        assert_eq!((texture.side, texture.tag), (64, ColourTag::Srgb));
        assert_eq!(texture.rgba8.len(), 64 * 64 * 4);
    }
    assert_eq!(built.palette, Some(crate::palette::room()));
    assert_eq!(built, world(&sim), "the same world each time");
}

/// Two 4:3 halves of 90 degrees across: tan(fov_y / 2) is 3/4.
#[test]
fn the_field_of_view_is_ninety_degrees_across_a_four_by_three_image() {
    let (sin, cos) = sin_cos_f64(FOV_Y.radians_f64() / 2.0);
    assert!((sin / cos - 0.75).abs() < 1e-9, "{}", sin / cos);
}

#[test]
fn the_camera_is_player_0s_eye_at_its_view_angles() {
    let start = room(2);
    let cam = camera(&frame(&start, &start, 1.0));
    // Spawn 0, (1.5, 4.5) on the floor, facing east; the eye 1.6 m up.
    assert_eq!(cam.eye, WorldPos::new(1.5, 4.5, 1.6));
    assert_eq!((cam.yaw, cam.pitch), (BAngle::ZERO, BAngle::ZERO));
    assert_eq!((cam.fov_y, cam.near), (FOV_Y, NEAR_PLANE));

    // Walking north-east, looking a little up: between the two states at
    // alpha, the angles of the newest.
    let walk = |ticks: u64| {
        let mut sim = room(2);
        for tick in 1..=ticks {
            let player_0 = PlayerCommand {
                forward: 127,
                view_yaw: 0x2000,
                view_pitch: 0x0400,
                ..PlayerCommand::idle(Tick(tick))
            };
            sim.step(&[player_0, PlayerCommand::idle(Tick(tick))])
                .unwrap();
        }
        sim
    };
    let (previous, moved) = (walk(9), walk(10));
    let at = |alpha| camera(&frame(&previous, &moved, alpha));
    let state = |sim: &Sim| {
        let world = sim.world();
        let (player_0, _) = world
            .storage::<eng_core::PlayerSlot>()
            .iter()
            .find(|(_, slot)| slot.0 == 0)
            .unwrap();
        world.get::<kit_fps::MoveState>(player_0).unwrap().pos
    };
    let (p, c) = (state(&previous), state(&moved));
    assert!(c.x > p.x && c.y > p.y);
    assert_eq!(at(1.0).eye, WorldPos::new(c.x, c.y, c.z - 0.875 + 1.6));
    assert_eq!(at(0.0).eye, WorldPos::new(p.x, p.y, p.z - 0.875 + 1.6));
    let half = at(0.5).eye;
    assert!((half.x - (p.x + c.x) / 2.0).abs() < 1e-12);
    assert_eq!(at(0.5).yaw, BAngle::from_u16(0x2000));
    assert_eq!(at(0.5).pitch, BAngle::from_u16(0x0400));
}

#[test]
fn each_region_takes_its_light_and_the_alcove_its_green_fog() {
    let sim = room(1);
    let map = sim.world().constant::<game_testbed_sim::room::RoomMap>();
    let looks = regions(map);
    let lights: Vec<u8> = looks.iter().map(|l| l.light).collect();
    assert_eq!(lights, [160, 192, 112, 255]);
    for (i, look) in looks.iter().enumerate() {
        if i == 2 {
            let LinearRgb { r, g, b } = look.fog;
            assert!(g > 2.0 * r && g > 2.0 * b, "green: {:?}", look.fog);
            assert!(look.fog_density > 0.0);
        } else {
            assert_eq!(look.fog_density, 0.0, "region {i}");
        }
    }
}

#[test]
fn each_thing_is_a_sprite_in_the_region_its_feet_stand_in() {
    let sim = room(1);
    let all = sprites(&frame(&sim, &sim, 1.0));
    let kinds = [ThingKind::Marker, ThingKind::Crate, ThingKind::Barrel];
    assert_eq!(all.len(), 3);
    for (number, (sprite, kind)) in all.iter().zip(kinds).enumerate() {
        let look = crate::sprites::look(kind);
        assert_eq!(sprite.id, number as u32);
        assert_eq!((sprite.width, sprite.height), (look.size, look.size));
        assert_eq!(sprite.sheet, look.sheet);
        // Rotation k names the frame its sheet draws for it, mirrored or
        // not, so no slot points at another thing's frames.
        for (k, m) in sprite.materials.iter().enumerate() {
            let frame = u32::from(look.sheet.frame(k as u8).frame);
            assert_eq!(*m, look.first_material + frame, "{kind:?} {k}");
        }
    }
    // The marker and the crate in the hall, the barrel on the platform.
    let regions: Vec<u16> = all.iter().map(|s| s.region).collect();
    assert_eq!(regions, [0, 0, 1]);
    assert_eq!(all[0].feet, WorldPos::new(4.0, 4.5, 0.0));
    assert_eq!(all[2].feet, WorldPos::new(10.5, 7.5, 0.4));
    assert_eq!(all[0].sheet, RotationSheet::Eight);
    // The marker turns: an eighth of a turn every second.
    let mut later = room(1);
    for tick in 1..=60 {
        later.step(&[PlayerCommand::idle(Tick(tick))]).unwrap();
    }
    let turned = sprites(&frame(&later, &later, 1.0))[0];
    assert_eq!(turned.facing.0, BAngle::HALF.0.wrapping_add(1 << 29));
}

#[test]
fn a_frame_shows_player_0s_view_with_every_look_feature_on() {
    let sim = room(1);
    let s = scene(&frame(&sim, &sim, 1.0));
    assert_eq!(s.look, Look::ON);
    assert_eq!((s.sky, s.ev100), (SKY, EV100));
    assert_eq!(s.regions.len(), 4);
    assert_eq!(s.sprites.len(), 3);
    assert_eq!(s.camera.eye, WorldPos::new(1.5, 4.5, 1.6));
}
