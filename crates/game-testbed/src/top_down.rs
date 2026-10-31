//! The testbed's extraction (plan difference 6): the top-down view of its
//! world, for `shot`, `video` and `play`. It reads the room, the hull and
//! each dot from the simulation's state and never writes to it.
//!
//! The camera looks down on the room's centre, zoomed so the whole room
//! fits the image: 40 px/m on the 320x240 shot, where the room fills it
//! exactly. The room is a wall-coloured rectangle under a floor inset by
//! the walls' thickness, so its edges show; each dot is a disc as wide as
//! its hull, coloured by player slot, drawn where it is between the
//! previous and current states at the frame's alpha.

use eng_cli::TopDownFrame;
use eng_collision_api::BoxRoom;
use eng_core::PlayerSlot;
use eng_math::{WorldPos, min_f64};
use eng_render::{LinearRgb, Shape, TopDownCamera, TopDownScene};
use game_testbed_sim::Dot;
use kit_fps::{MoveConfig, MoveState};

const fn rgb(r: f32, g: f32, b: f32) -> LinearRgb {
    LinearRgb { r, g, b }
}

/// Colours are linear (AGENTS.md conventions), each channel the centre of
/// an 8-bit sRGB level, given in the comments: half a level from either
/// neighbour, so every adapter's sRGB output stores the same byte and only
/// the dots' edges can differ between adapters (the goldens, plan decision
/// 3). Outside the room:
pub(crate) const BACKGROUND: LinearRgb = rgb(0.002_124_69, 0.003_035_27, 0.008_023_19); // 7, 10, 22
pub(crate) const WALL: LinearRgb = rgb(0.300_544, 0.300_544, 0.327_778); // 149, 149, 155
pub(crate) const FLOOR: LinearRgb = rgb(0.035_601_3, 0.039_546_2, 0.045_186_2); // 53, 56, 60
/// The walls' thickness as drawn, inside the room's bounds: 4 pixels on the
/// shot. A dot against a wall is drawn over it, as dots come last.
pub(crate) const WALL_THICKNESS: f64 = 0.1;
/// Each player's dot, by slot, repeating after the eighth.
pub(crate) const DOTS: [LinearRgb; 8] = [
    rgb(1.0, 0.318_547, 0.020_288_6),       // 255, 153, 39: orange
    rgb(0.020_288_6, 0.552_011, 1.0),       // 39, 196, 255: blue
    rgb(0.080_219_8, 0.799_103, 0.119_538), // 80, 231, 97: green
    rgb(0.846_873, 0.080_219_8, 0.752_942), // 237, 80, 225: magenta
    rgb(1.0, 0.846_873, 0.049_706_6),       // 255, 237, 63: yellow
    rgb(0.049_706_6, 0.896_269, 0.846_873), // 63, 243, 237: cyan
    rgb(0.947_307, 0.947_307, 0.947_307),   // 249, 249, 249: white
    rgb(0.597_202, 0.250_158, 1.0),         // 203, 137, 255: violet
];

/// From `a` at `t` = 0 to `b` at `t` = 1, exact at both ends.
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a * (1.0 - t) + b * t
}

/// The top-down view of one frame.
pub(crate) fn scene(frame: &TopDownFrame<'_>) -> TopDownScene {
    let world = frame.current.world();
    let room = *world.resource::<BoxRoom>();
    let radius = world.resource::<MoveConfig>().hull.half_extents.x;
    let (width, height) = (room.max.x - room.min.x, room.max.y - room.min.y);
    let at = |x: f64, y: f64| WorldPos::new(x, y, 0.0);
    let t = WALL_THICKNESS;
    let mut shapes = vec![
        Shape::Rect {
            min: at(room.min.x, room.min.y),
            max: at(room.max.x, room.max.y),
            colour: WALL,
        },
        Shape::Rect {
            min: at(room.min.x + t, room.min.y + t),
            max: at(room.max.x - t, room.max.y - t),
            colour: FLOOR,
        },
    ];
    let previous = frame.previous.world();
    for (dot, Dot) in world.storage::<Dot>().iter() {
        let (Some(state), Some(slot)) = (world.get::<MoveState>(dot), world.get::<PlayerSlot>(dot))
        else {
            continue;
        };
        let from = previous.get::<MoveState>(dot).map_or(state.pos, |s| s.pos);
        let lerp = |a: f64, b: f64| lerp(a, b, frame.alpha);
        shapes.push(Shape::Disc {
            centre: WorldPos::new(
                lerp(from.x, state.pos.x),
                lerp(from.y, state.pos.y),
                lerp(from.z, state.pos.z),
            ),
            radius,
            colour: DOTS[usize::from(slot.0) % DOTS.len()],
        });
    }
    TopDownScene {
        camera: TopDownCamera {
            centre: at(room.min.x + width / 2.0, room.min.y + height / 2.0),
            pixels_per_meter: min_f64(
                f64::from(frame.width) / width,
                f64::from(frame.height) / height,
            ),
        },
        background: BACKGROUND,
        shapes,
    }
}

#[cfg(test)]
mod tests {
    use eng_cli::TopDownFrame;
    use eng_core::{PlayerCommand, Tick};
    use eng_math::WorldPos;
    use eng_render::{LinearRgb, Shape, TopDownCamera, TopDownScene};
    use eng_sim::{Sim, SimConfig};
    use kit_fps::MoveState;

    use super::{BACKGROUND, DOTS, FLOOR, WALL, WALL_THICKNESS, scene};

    fn sim(players: u8, seed: u64) -> Sim {
        game_testbed_sim::build(SimConfig {
            tick_rate: game_testbed_sim::TICK_RATE,
            players,
            seed,
        })
        .unwrap()
    }

    /// Where each dot is, in slot order.
    fn dots(sim: &Sim) -> Vec<WorldPos> {
        let world = sim.world();
        world
            .storage::<MoveState>()
            .iter()
            .map(|(_, s)| s.pos)
            .collect()
    }

    fn frame<'a>(
        previous: &'a Sim,
        current: &'a Sim,
        alpha: f64,
        size: (u32, u32),
    ) -> TopDownFrame<'a> {
        TopDownFrame {
            previous,
            current,
            alpha,
            width: size.0,
            height: size.1,
        }
    }

    fn at(x: f64, y: f64) -> WorldPos {
        WorldPos::new(x, y, 0.0)
    }

    #[test]
    fn the_shot_shows_the_room_and_its_dot() {
        let sim = sim(1, 1);
        let dot = dots(&sim)[0];
        let t = WALL_THICKNESS;
        assert_eq!(
            scene(&frame(&sim, &sim, 1.0, (320, 240))),
            TopDownScene {
                // 40 px/m fits the 8 m by 6 m room into 320x240 exactly.
                camera: TopDownCamera {
                    centre: at(4.0, 3.0),
                    pixels_per_meter: 40.0,
                },
                background: BACKGROUND,
                shapes: vec![
                    Shape::Rect {
                        min: at(0.0, 0.0),
                        max: at(8.0, 6.0),
                        colour: WALL,
                    },
                    Shape::Rect {
                        min: at(t, t),
                        max: at(8.0 - t, 6.0 - t),
                        colour: FLOOR,
                    },
                    // The hull's half-width.
                    Shape::Disc {
                        centre: dot,
                        radius: 0.25,
                        colour: DOTS[0],
                    },
                ],
            }
        );
    }

    #[test]
    fn the_room_fits_any_view() {
        let sim = sim(1, 1);
        for (size, zoom) in [((1280, 720), 120.0), ((100, 300), 12.5), ((321, 240), 40.0)] {
            let scene = scene(&frame(&sim, &sim, 1.0, size));
            assert_eq!(scene.camera.pixels_per_meter, zoom, "{size:?}");
            assert_eq!(scene.camera.centre, at(4.0, 3.0));
        }
    }

    #[test]
    fn each_player_has_a_dot_in_its_colour() {
        let sim = sim(3, 9);
        let scene = scene(&frame(&sim, &sim, 1.0, (320, 240)));
        let discs: Vec<(WorldPos, LinearRgb)> = scene
            .shapes
            .iter()
            .filter_map(|s| match *s {
                Shape::Disc { centre, colour, .. } => Some((centre, colour)),
                Shape::Rect { .. } => None,
            })
            .collect();
        let expected: Vec<_> = dots(&sim).into_iter().zip(DOTS).collect();
        assert_eq!(discs, expected);
        assert!(DOTS.windows(2).all(|w| w[0] != w[1]), "the colours differ");
    }

    #[test]
    fn dots_are_drawn_between_the_two_states() {
        let previous = sim(1, 4);
        let mut current = sim(1, 4);
        let walk = PlayerCommand {
            forward: 127,
            side: -60,
            ..PlayerCommand::idle(Tick(1))
        };
        for tick in 1..=20 {
            current
                .step(&[PlayerCommand {
                    tick: Tick(tick),
                    ..walk
                }])
                .unwrap();
        }
        let (p, c) = (dots(&previous)[0], dots(&current)[0]);
        assert!(p.x != c.x && p.y != c.y, "the dot moved: {p:?} {c:?}");
        let centre = |alpha| match scene(&frame(&previous, &current, alpha, (320, 240))).shapes[2] {
            Shape::Disc { centre, .. } => centre,
            Shape::Rect { .. } => panic!("the third shape is the dot"),
        };
        assert_eq!(centre(1.0), c);
        assert_eq!(centre(0.0), p);
        let half = centre(0.5);
        for (got, want) in [(half.x, (p.x + c.x) / 2.0), (half.y, (p.y + c.y) / 2.0)] {
            assert!((got - want).abs() < 1e-12, "{got} {want}");
        }
    }
}

#[cfg(test)]
mod lerp_tests {
    use super::lerp;

    #[test]
    fn the_ends_are_exact() {
        // 1e9 + 0.1 + (3.7 - (1e9 + 0.1)) rounds away from 3.7.
        let (a, b) = (1.0e9 + 0.1, 3.7);
        assert_eq!(lerp(a, b, 0.0), a);
        assert_eq!(lerp(a, b, 1.0), b);
        assert_eq!(lerp(2.0, 4.0, 0.25), 2.5);
    }
}
