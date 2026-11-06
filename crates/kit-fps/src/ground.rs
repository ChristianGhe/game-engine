//! Ground movement (spec 6.11, rung 2): [`pmove`](crate::pmove) plus
//! gravity, step-up, step-down and ground snapping, after Quake 2's
//! `PM_StepSlideMove` and Quake 3's `PM_GroundTrace`. There is no jumping
//! (Doom has none).
//!
//! One tick of [`ground_move`], in this order:
//!
//! 1. **Ground check.** A trace [`GROUND_PROBE`] down from the start looks for
//!    floor: a hit whose normal's z is at least
//!    [`GroundConfig::min_floor_normal_z`]. Nothing about the ground is kept
//!    between ticks, so [`MoveState`] keeps its v1 encoding. Its `vel.z` is
//!    the vertical velocity.
//! 2. **Velocity.** On the ground, `vel.z` is set to 0, friction applies, and
//!    the hull accelerates towards the command as in `pmove`. In the air only
//!    gravity acts, `vel.z -= gravity * dt`: no friction and no air control,
//!    as rung 2's plan says. With air control, a hull pushed into a slope too
//!    steep to be floor would climb it, because the slide turns the push
//!    into motion up the slope.
//! 3. **Position, after velocity** (semi-implicit Euler). The hull slides by
//!    the new velocity times `dt`, so a hull at rest in the air ends its first
//!    tick `gravity * dt * dt` lower, not at the same height.
//! 4. **Step up, on the ground only.** The slide runs flat from the start,
//!    and again from the start raised by up to [`GroundConfig::step_height`]
//!    (less under a ceiling), then pushed down by the rise plus
//!    [`GROUND_PROBE`], so floor at the start's own height is found. The
//!    stepped result is kept only if that push lands on floor (Quake 2's
//!    `MIN_STEP_NORMAL` check) and its horizontal travel is strictly greater,
//!    comparing squared distances exactly in f64. Otherwise the flat one is
//!    kept. A tie keeps the flat one, so on flat ground with nothing to step
//!    onto the result is `pmove`'s, bit for bit.
//! 5. **Step down.** If the hull was on the ground and the kept move is not
//!    going up (`vel.z <= 0`), the hull drops onto floor at most
//!    [`GroundConfig::snap_distance`] below. It walks down stairs instead of
//!    falling off each one. A deeper drop leaves the ground.
//!
//! [`GroundOutcome::on_ground`] is step 1's check at the end position, which
//! is what the next tick finds.

use eng_collision_api::{BoxHull, CollisionWorld};
use eng_core::PlayerCommand;
use eng_math::{Real, Vec3, WorldPos};
use eng_schema_core::Schema;

use crate::{MAX_PLANES, MoveConfig, MoveOutcome, MoveState, accelerate, friction, slide, wish};

/// How far below the hull the ground check looks (m), about 1 mm. The
/// step-up's push-down and the step-down also look this much further, so
/// floor exactly at the end of their reach is found.
///
/// This assumes every [`CollisionWorld`] stops a trace exactly at contact, as
/// `BoxRoom`, the test box world and exact f64 sweeps do, so a resting hull's
/// probe hits at fraction 0. Neither the collision contract nor
/// `conformance::check` promises that. A world that stops traces short by
/// more than this would leave a resting hull in the air, to fall a little
/// every tick.
pub const GROUND_PROBE: f64 = 1.0 / 1024.0;

/// Ground-movement tuning, in meters and seconds.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "kit_fps.GroundConfig", version = 1)]
pub struct GroundConfig {
    /// Downward acceleration in the air (m/s²).
    pub gravity: Real,
    /// The tallest ledge the hull walks up onto (m).
    pub step_height: Real,
    /// The deepest drop a grounded hull follows down instead of falling (m).
    pub snap_distance: Real,
    /// The steepest walkable floor, as the smallest z its unit normal may
    /// have: the cosine of its slope. 0.7 (about 45.6 degrees) is Quake's
    /// `MIN_WALK_NORMAL`.
    pub min_floor_normal_z: Real,
}

impl GroundConfig {
    /// Earth's gravity, a 0.4 m step and snap, and Quake's steepest floor.
    pub const DEFAULT: Self = Self {
        gravity: 9.81,
        step_height: 0.4,
        snap_distance: 0.4,
        min_floor_normal_z: 0.7,
    };
}

impl Default for GroundConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The new state, whether the hull ends on the ground, and the planes the
/// kept slide (flat or stepped) was clipped against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundOutcome {
    pub state: MoveState,
    /// The hull stands on floor at the end of the tick.
    pub on_ground: bool,
    planes: [Vec3; MAX_PLANES],
    plane_count: usize,
}

impl GroundOutcome {
    pub fn planes(&self) -> &[Vec3] {
        &self.planes[..self.plane_count]
    }
}

/// Moves the hull for one tick of length `dt` seconds, under gravity and over
/// steps. The five steps and their order are in [`crate::ground`]'s docs.
pub fn ground_move<C: CollisionWorld + ?Sized>(
    state: &MoveState,
    cmd: &PlayerCommand,
    config: &MoveConfig,
    ground: &GroundConfig,
    dt: Real,
    world: &C,
) -> GroundOutcome {
    let hull = &config.hull;
    let grounded = on_floor(state.pos, hull, ground, world);
    let mut vel = state.vel;
    let kept = if grounded {
        vel.z = 0.0;
        vel = friction(vel, config, dt);
        let (wish_dir, wish_speed) = wish(cmd, config);
        vel = accelerate(vel, wish_dir, wish_speed, config.accel, dt);
        let flat = slide(state.pos, vel, dt, hull, world);
        match step_up(state.pos, vel, dt, hull, ground, world) {
            Some(stepped)
                if travel(state.pos, stepped.state.pos) > travel(state.pos, flat.state.pos) =>
            {
                stepped
            }
            _ => flat,
        }
    } else {
        vel.z -= ground.gravity * dt;
        slide(state.pos, vel, dt, hull, world)
    };
    let mut end = kept.state;
    if grounded && end.vel.z <= 0.0 {
        end.pos = step_down(end.pos, hull, ground, world);
    }
    GroundOutcome {
        state: end,
        on_ground: on_floor(end.pos, hull, ground, world),
        planes: kept.planes,
        plane_count: kept.plane_count,
    }
}

/// Whether the hull stands on floor.
fn on_floor<C: CollisionWorld + ?Sized>(
    pos: WorldPos,
    hull: &BoxHull,
    ground: &GroundConfig,
    world: &C,
) -> bool {
    let tr = world.trace(pos, below(pos, GROUND_PROBE), hull);
    !tr.start_solid && tr.hit() && is_floor(tr.normal, ground)
}

fn is_floor(normal: Vec3, ground: &GroundConfig) -> bool {
    normal.z >= ground.min_floor_normal_z
}

fn below(pos: WorldPos, depth: f64) -> WorldPos {
    WorldPos::new(pos.x, pos.y, pos.z - depth)
}

/// Squared horizontal distance, in f64.
fn travel(from: WorldPos, to: WorldPos) -> f64 {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    dx * dx + dy * dy
}

/// Step 4's second try: raise, slide, push down. `None` when the hull
/// cannot rise, or the push does not land on floor.
fn step_up<C: CollisionWorld + ?Sized>(
    start: WorldPos,
    vel: Vec3,
    dt: Real,
    hull: &BoxHull,
    ground: &GroundConfig,
    world: &C,
) -> Option<MoveOutcome> {
    let top = WorldPos::new(start.x, start.y, start.z + f64::from(ground.step_height));
    let up = world.trace(start, top, hull);
    let rise = up.end.z - start.z;
    if up.start_solid || rise <= 0.0 {
        return None;
    }
    let mut out = slide(up.end, vel, dt, hull, world);
    let from = out.state.pos;
    let down = world.trace(from, below(from, rise + GROUND_PROBE), hull);
    if down.start_solid || !down.hit() || !is_floor(down.normal, ground) {
        return None;
    }
    out.state.pos = down.end;
    Some(out)
}

/// Step 5: the hull dropped onto floor at most the snap distance below, or
/// left where it is.
fn step_down<C: CollisionWorld + ?Sized>(
    from: WorldPos,
    hull: &BoxHull,
    ground: &GroundConfig,
    world: &C,
) -> WorldPos {
    let snap = f64::from(ground.snap_distance);
    // A trace that ends exactly touching floor reports no hit, so look one
    // probe further and then check the drop, which keeps the snap distance
    // itself inclusive.
    let tr = world.trace(from, below(from, snap + GROUND_PROBE), hull);
    let lands = !tr.start_solid
        && tr.hit()
        && tr.fraction > 0.0
        && is_floor(tr.normal, ground)
        && from.z - tr.end.z <= snap;
    if lands { tr.end } else { from }
}
