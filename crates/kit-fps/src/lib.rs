//! First-person movement as a pure function (spec 6.11), after Quake 3's
//! `bg_pmove`: friction, acceleration towards the commanded direction, then
//! collide-and-slide against up to [`MAX_PLANES`] planes.
//!
//! Rung 1a moves on a flat floor. Gravity, step-up, step-down and ground
//! snapping arrive with real levels at rung 2.
#![no_std]
#![forbid(unsafe_code)]

use eng_collision_api::{BoxHull, CollisionWorld};
use eng_core::PlayerCommand;
use eng_math::{BAngle, Real, Vec2, Vec3, WorldPos, clamp, max, min};
use eng_schema_core::Schema;

/// The most planes one move can be clipped against.
pub const MAX_PLANES: usize = 5;
const MAX_BUMPS: usize = 4;

/// Position and velocity of a moving hull.
#[derive(Debug, Clone, Copy, PartialEq, Default, Schema)]
#[schema(name = "kit_fps.MoveState", version = 1)]
pub struct MoveState {
    pub pos: WorldPos,
    pub vel: Vec3,
}

/// Movement tuning, in meters and seconds.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "kit_fps.MoveConfig", version = 1)]
pub struct MoveConfig {
    /// Top speed with the stick fully forward (m/s).
    pub max_speed: Real,
    /// Acceleration factor: the fraction of `max_speed` gained per second.
    pub accel: Real,
    /// Friction factor (1/s).
    pub friction: Real,
    /// Below this speed friction acts as if moving at it, so the hull stops.
    pub stop_speed: Real,
    pub hull: BoxHull,
}

/// The new state, plus the planes the move was clipped against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveOutcome {
    pub state: MoveState,
    planes: [Vec3; MAX_PLANES],
    plane_count: usize,
}

impl MoveOutcome {
    pub fn planes(&self) -> &[Vec3] {
        &self.planes[..self.plane_count]
    }
}

/// Moves the hull for one tick of length `dt` seconds.
pub fn pmove<C: CollisionWorld + ?Sized>(
    state: &MoveState,
    cmd: &PlayerCommand,
    config: &MoveConfig,
    dt: Real,
    world: &C,
) -> MoveOutcome {
    let (wish_dir, wish_speed) = wish(cmd, config);
    let mut vel = friction(state.vel, config, dt);
    vel = accelerate(vel, wish_dir, wish_speed, config.accel, dt);
    slide(state.pos, vel, dt, &config.hull, world)
}

/// Direction and speed the command asks for, from its absolute yaw.
fn wish(cmd: &PlayerCommand, config: &MoveConfig) -> (Vec3, Real) {
    let forward = BAngle::from_u16(cmd.view_yaw).direction();
    let right = Vec2::new(forward.y, -forward.x);
    let f = clamp(Real::from(cmd.forward) / 127.0, -1.0, 1.0);
    let s = clamp(Real::from(cmd.side) / 127.0, -1.0, 1.0);
    let wish = forward * f + right * s;
    let len = wish.length();
    if len == 0.0 {
        return (Vec3::ZERO, 0.0);
    }
    let dir = Vec3::new(wish.x / len, wish.y / len, 0.0);
    (dir, config.max_speed * min(len, 1.0))
}

fn friction(vel: Vec3, config: &MoveConfig, dt: Real) -> Vec3 {
    let speed = Vec2::new(vel.x, vel.y).length();
    if speed == 0.0 {
        return vel;
    }
    let control = max(speed, config.stop_speed);
    let new_speed = max(speed - control * config.friction * dt, 0.0);
    vel * (new_speed / speed)
}

fn accelerate(vel: Vec3, dir: Vec3, wish_speed: Real, accel: Real, dt: Real) -> Vec3 {
    let current = vel.dot(dir);
    let add = wish_speed - current;
    if add <= 0.0 {
        return vel;
    }
    vel + dir * min(accel * dt * wish_speed, add)
}

/// Removes the velocity component into `normal`.
fn clip(vel: Vec3, normal: Vec3) -> Vec3 {
    vel - normal * vel.dot(normal)
}

fn slide<C: CollisionWorld + ?Sized>(
    start: WorldPos,
    mut vel: Vec3,
    dt: Real,
    hull: &BoxHull,
    world: &C,
) -> MoveOutcome {
    let primal = vel;
    let mut pos = start;
    let mut planes = [Vec3::ZERO; MAX_PLANES];
    let mut count = 0;
    let mut time_left = f64::from(dt);
    for _ in 0..MAX_BUMPS {
        if vel == Vec3::ZERO {
            break;
        }
        let tr = world.trace(pos, pos.add_scaled(vel, time_left), hull);
        if tr.start_solid {
            vel = Vec3::ZERO;
            break;
        }
        pos = tr.end;
        if !tr.hit() {
            break;
        }
        time_left -= time_left * tr.fraction;
        if count == MAX_PLANES {
            vel = Vec3::ZERO;
            break;
        }
        planes[count] = tr.normal;
        count += 1;
        let mut clipped = clip(vel, tr.normal);
        for &other in &planes[..count - 1] {
            if clipped.dot(other) < 0.0 {
                // Into an earlier plane as well: slide along the crease.
                let crease = other.cross(tr.normal).normalize_or_zero();
                clipped = crease * crease.dot(vel);
            }
        }
        if clipped.dot(primal) <= 0.0 {
            // Never bounce back against the original motion.
            vel = Vec3::ZERO;
            break;
        }
        vel = clipped;
    }
    MoveOutcome {
        state: MoveState { pos, vel },
        planes,
        plane_count: count,
    }
}
