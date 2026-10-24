use eng_math::{Vec3, WorldPos, clamp_f64};
use eng_schema_core::Schema;

use crate::{BoxHull, CollisionWorld, Trace};

/// The trivial implementation: the open inside of one axis-aligned room.
/// Everything outside `[min, max]` is solid.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "eng.collision.BoxRoom", version = 1)]
pub struct BoxRoom {
    pub min: WorldPos,
    pub max: WorldPos,
}

impl BoxRoom {
    /// The range the hull's centre may occupy on each axis.
    fn allowed(&self, hull: &BoxHull) -> [(f64, f64); 3] {
        let h = hull.half_extents;
        [
            (self.min.x + f64::from(h.x), self.max.x - f64::from(h.x)),
            (self.min.y + f64::from(h.y), self.max.y - f64::from(h.y)),
            (self.min.z + f64::from(h.z), self.max.z - f64::from(h.z)),
        ]
    }
}

fn axes(p: WorldPos) -> [f64; 3] {
    [p.x, p.y, p.z]
}

impl CollisionWorld for BoxRoom {
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace {
        if self.overlaps(start, hull) {
            return Trace {
                fraction: 0.0,
                end: start,
                normal: Vec3::ZERO,
                start_solid: true,
            };
        }
        let allowed = self.allowed(hull);
        let (s, e) = (axes(start), axes(end));
        // The first wall crossed; ties go to the lower axis (X, then Y, Z).
        let mut best: Option<(f64, usize, f64)> = None;
        for axis in 0..3 {
            let d = e[axis] - s[axis];
            let (lo, hi) = allowed[axis];
            let limit = if d > 0.0 && e[axis] > hi {
                hi
            } else if d < 0.0 && e[axis] < lo {
                lo
            } else {
                continue;
            };
            // A hit always has fraction < 1, even when the crossing is so close
            // to the end that the division rounds to 1.
            let t = clamp_f64((limit - s[axis]) / d, 0.0, 1.0f64.next_down());
            if best.is_none_or(|(bt, _, _)| t < bt) {
                best = Some((t, axis, limit));
            }
        }
        let Some((t, axis, limit)) = best else {
            return Trace {
                fraction: 1.0,
                end,
                normal: Vec3::ZERO,
                start_solid: false,
            };
        };
        let mut p = [0.0; 3];
        for i in 0..3 {
            // Rounding must never leave the hull inside a wall.
            p[i] = clamp_f64(s[i] + (e[i] - s[i]) * t, allowed[i].0, allowed[i].1);
        }
        p[axis] = limit;
        let sign = if e[axis] > s[axis] { -1.0 } else { 1.0 };
        let mut normal = Vec3::ZERO;
        match axis {
            0 => normal.x = sign,
            1 => normal.y = sign,
            _ => normal.z = sign,
        }
        Trace {
            fraction: t,
            end: WorldPos::new(p[0], p[1], p[2]),
            normal,
            start_solid: false,
        }
    }

    fn overlaps(&self, at: WorldPos, hull: &BoxHull) -> bool {
        self.allowed(hull)
            .iter()
            .zip(axes(at))
            .any(|(&(lo, hi), x)| !(lo..=hi).contains(&x))
    }
}
