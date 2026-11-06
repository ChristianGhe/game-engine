//! `ground_move` on slopes, which boxes cannot make: the floor rule
//! (`GroundConfig::min_floor_normal_z`) decides what the hull can walk on
//! and climb.

use eng_collision_api::{BoxHull, CollisionWorld, Trace};
use eng_core::{PlayerCommand, Tick};
use eng_math::{Vec3, WorldPos, clamp_f64, max_f64};
use kit_fps::{GroundConfig, GroundOutcome, MoveConfig, MoveState, ground_move, pmove};

/// Rung 2's room hull: 0.5 m x 0.5 m x 1.75 m.
const MOVE: MoveConfig = MoveConfig {
    max_speed: 4.0,
    accel: 10.0,
    friction: 6.0,
    stop_speed: 1.0,
    hull: BoxHull {
        half_extents: Vec3::new(0.25, 0.25, 0.875),
    },
};
const GROUND: GroundConfig = GroundConfig::DEFAULT;
const DT: f32 = 1.0 / 60.0;
/// The hull's half height: its centre's height above the floor it stands on.
const H: f64 = 0.875;

fn cmd(forward: i8, side: i8, yaw: u16) -> PlayerCommand {
    PlayerCommand {
        forward,
        side,
        view_yaw: yaw,
        ..PlayerCommand::idle(Tick(1))
    }
}

/// Full stick towards +x: uphill on every slope here.
fn east() -> PlayerCommand {
    cmd(127, 0, 0)
}

fn tick<C: CollisionWorld + ?Sized>(world: &C, s: &MoveState, c: &PlayerCommand) -> GroundOutcome {
    ground_move(s, c, &MOVE, &GROUND, DT, world)
}

fn state(pos: [f64; 3], vel: [f32; 3]) -> MoveState {
    MoveState {
        pos: WorldPos::new(pos[0], pos[1], pos[2]),
        vel: Vec3::new(vel[0], vel[1], vel[2]),
    }
}
/// Solid below a plane through the origin with unit normal `n`, since boxes
/// have no slopes. A hit stops at contact, and only a hull more than
/// [`SKIN`] inside counts as overlapping.
struct Slope(Vec3);

/// The f32 velocity `pmove` clips against a tilted plane is still a little
/// into it, and its slide zeroes the velocity when it hits the same plane
/// twice. A 1 µm skin keeps that rounding from stopping the hull, so this
/// test sees only the floor rule.
const SKIN: f64 = 1e-6;

impl Slope {
    /// The hull's distance from the plane; negative inside.
    fn gap(&self, at: WorldPos, hull: &BoxHull) -> f64 {
        let (n, h) = (self.0, hull.half_extents);
        let reach = f64::from(n.x.abs() * h.x + n.y.abs() * h.y + n.z.abs() * h.z);
        f64::from(n.x) * at.x + f64::from(n.y) * at.y + f64::from(n.z) * at.z - reach
    }

    /// A hull resting on the plane at (x, 0).
    fn resting(&self, x: f64) -> MoveState {
        let base = state([x, 0.0, 0.0], [0.0; 3]);
        let lift = -self.gap(base.pos, &MOVE.hull) / f64::from(self.0.z);
        state([x, 0.0, lift], [0.0; 3])
    }
}

impl CollisionWorld for Slope {
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace {
        let (g0, g1) = (self.gap(start, hull), self.gap(end, hull));
        if g0 < -SKIN || g1 >= -SKIN {
            let solid = g0 < -SKIN;
            return Trace {
                fraction: if solid { 0.0 } else { 1.0 },
                end: if solid { start } else { end },
                normal: Vec3::ZERO,
                start_solid: solid,
            };
        }
        let d = WorldPos::new(end.x - start.x, end.y - start.y, end.z - start.z);
        let mut t = clamp_f64(g0 / (g0 - g1), 0.0, 1.0f64.next_down());
        loop {
            let p = WorldPos::new(start.x + d.x * t, start.y + d.y * t, start.z + d.z * t);
            if !self.overlaps(p, hull) {
                return Trace {
                    fraction: t,
                    end: p,
                    normal: self.0,
                    start_solid: false,
                };
            }
            t *= 0.5;
        }
    }

    fn overlaps(&self, at: WorldPos, hull: &BoxHull) -> bool {
        self.gap(at, hull) < -SKIN
    }
}

/// Rising towards +x, so `east()` pushes uphill. Normal z 0.8 (about 37
/// degrees) is floor.
const GENTLE: Slope = Slope(Vec3::new(-0.6, 0.0, 0.8));

#[test]
fn a_walkable_slope_holds_an_idle_hull() {
    let start = GENTLE.resting(1.0);
    let mut s = start;
    for _ in 0..60 {
        let out = tick(&GENTLE, &s, &cmd(0, 0, 0));
        assert!(out.on_ground && out.state.vel == Vec3::ZERO, "{out:?}");
        s = out.state;
    }
    assert!(
        s.pos.distance(start.pos) < 1e-6,
        "crept from {start:?} to {s:?}"
    );
}

#[test]
fn a_walkable_slope_can_be_climbed() {
    let start = GENTLE.resting(1.0);
    let mut s = start;
    for _ in 0..60 {
        let out = tick(&GENTLE, &s, &east());
        assert!(
            out.on_ground && !GENTLE.overlaps(out.state.pos, &MOVE.hull),
            "{out:?}"
        );
        s = out.state;
    }
    assert!(s.pos.z > start.pos.z + 1.0, "{s:?}");
}

#[test]
fn pushing_uphill_on_a_steep_slope_gains_no_height() {
    // Normal z 0.6 (about 53 degrees), 0.17 (80) and 0.017 (89, a wall).
    for nz in [0.6f32, 0.17, 0.017] {
        let steep = steep(nz);
        let start = steep.resting(1.0);
        let mut s = start;
        for i in 0..60 {
            let out = tick(&steep, &s, &east());
            // Off the floor only gravity acts. Clipped along a plane whose
            // normal has z > 0, a downward velocity stays downward, so the
            // bound is exact: z never rises, on any tick.
            assert!(
                out.state.pos.z <= s.pos.z,
                "nz {nz}, tick {i}: rose from {s:?} to {out:?}"
            );
            assert!(
                !out.on_ground && !steep.overlaps(out.state.pos, &MOVE.hull),
                "nz {nz}, tick {i}: {out:?}"
            );
            s = out.state;
        }
        assert!(
            s.pos.z < start.pos.z - 1.0,
            "nz {nz}: did not slide down, {s:?}"
        );
    }
}
/// A slope rising towards +x whose normal has this z.
fn steep(nz: f32) -> Slope {
    Slope(Vec3::new(-eng_math::sqrt(1.0 - nz * nz), 0.0, nz))
}

/// Two half-spaces. A trace stops at whichever it meets first; a tie goes
/// to the first.
struct Both(Slope, Slope);

impl CollisionWorld for Both {
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace {
        let (a, b) = (
            self.0.trace(start, end, hull),
            self.1.trace(start, end, hull),
        );
        if b.start_solid || (!a.start_solid && b.fraction < a.fraction) {
            b
        } else {
            a
        }
    }

    fn overlaps(&self, at: WorldPos, hull: &BoxHull) -> bool {
        self.0.overlaps(at, hull) || self.1.overlaps(at, hull)
    }
}

/// A floor at z = 0 that meets `slope` at x = 0.
fn foot(slope: Slope) -> Both {
    Both(Slope(Vec3::Z), slope)
}

/// From flat floor, pushing into a slope for 120 ticks.
fn push_from_the_floor(world: &Both) -> f64 {
    let mut s = state([-1.0, 0.0, H], [0.0; 3]);
    let mut top = s.pos.z;
    for i in 0..120 {
        let out = tick(world, &s, &east());
        assert!(
            !world.overlaps(out.state.pos, &MOVE.hull),
            "tick {i}: {out:?}"
        );
        s = out.state;
        top = max_f64(top, s.pos.z);
    }
    top
}

#[test]
fn from_the_floor_a_steep_slope_gives_only_what_the_slide_gives() {
    // The hull is on the ground only on the floor, at z = H, since the slope
    // is not floor. There its speed is at most max_speed. The slide's clip
    // turns some of that speed upwards but never adds any, the stepped move
    // must land on floor, and in the air only gravity acts. So the hull rises
    // at most one tick's slide plus the height its speed buys against
    // gravity: max_speed * dt + max_speed^2 / (2 g), 0.88 m.
    let (v, g) = (f64::from(MOVE.max_speed), f64::from(GROUND.gravity));
    let bound = H + v * f64::from(DT) + v * v / (2.0 * g);
    for nz in [0.6, 0.17, 0.017] {
        let top = push_from_the_floor(&foot(steep(nz)));
        assert!(top <= bound, "nz {nz}: rose to {top}, over {bound}");
    }
    let top = push_from_the_floor(&foot(GENTLE));
    assert!(
        top > bound + 1.0,
        "the walkable slope was not climbed: {top}"
    );
}

#[test]
fn a_stepped_move_is_kept_only_if_it_lands_on_floor() {
    // On the floor at a slope's foot, at full speed into it. The stepped move
    // goes farther, but on a steep slope it lands on the slope, so the flat
    // slide is kept: exactly `pmove`'s result.
    let s = state([-0.25, 0.0, H], [4.0, 0.0, 0.0]);
    for nz in [0.6, 0.17, 0.017] {
        let world = foot(steep(nz));
        let flat = pmove(&s, &east(), &MOVE, DT, &world);
        assert_eq!(tick(&world, &s, &east()).state, flat.state, "nz {nz}");
    }
    let world = foot(GENTLE);
    let flat = pmove(&s, &east(), &MOVE, DT, &world);
    let out = tick(&world, &s, &east());
    assert!(
        out.state.pos.x > flat.state.pos.x && out.on_ground,
        "{out:?}"
    );
}
