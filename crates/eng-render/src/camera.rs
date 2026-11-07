//! The first-person camera and its projection (spec 6.7; rung 2 plan, queue
//! item Q4): where the eye is, where it looks, and how a world position
//! lands in clip space, all on the CPU.
//!
//! - **Axes** (AGENTS.md conventions): right-handed, Z up, meters. Yaw 0
//!   looks along +X and positive yaw turns toward +Y, counter-clockwise
//!   seen from above. Positive pitch looks up, toward +Z.
//! - **Camera-relative.** A position is subtracted from the eye in f64
//!   ([`WorldPos::relative_to`]), and only that difference is rounded to
//!   f32 and projected; the matrix never holds a translation. So the
//!   projection depends on the f64 difference (point minus eye) alone:
//!   a camera 10^9 m from the origin projects a point bit for bit as a
//!   camera at the origin projects the same offset, for any offset the
//!   f64 grid can represent at that distance (steps of 2^-23 m there).
//! - **Reversed-Z, infinite far.** A point `d` meters ahead of the eye has
//!   clip `w = d` and `z = near`, so its depth `z / w` is `near / d`: 1 on
//!   the near plane, falling toward 0 with distance and never reaching it.
//!   A point nearer than the near plane has depth above 1, and one beside
//!   the eye (on the plane through it, square to the view) or behind it
//!   has `w <= 0`; the GPU clips both. Depth is meant for a `Depth32Float`
//!   target cleared to 0 and tested with "greater".
//! - **Clip space** is wgpu's: x from -1 at the image's left edge to 1 at
//!   its right, y from -1 at the bottom to 1 at the top, depth in [0, 1].
//! - All trigonometry is f64 through eng-math (the pinned, plain-Rust
//!   libm), rounded to f32 once, so the matrices do not depend on the
//!   target's own maths library.

use eng_math::{BAngle, Real, Vec3, WorldPos, sin_cos_f64};

use crate::RenderError;

/// The near plane's distance for first-person views, in meters (rung 2,
/// T9). The plan left it open; the hull bounds it from above:
/// - **No wall is cut open.** The room's hull is 0.5 m wide, so the eye
///   stays at least 0.25 m from any wall's plane (more off the axes: a box
///   touches an angled wall with a corner). The near rectangle's farthest
///   corner is `near * sqrt(1 + ty^2 * (1 + aspect^2))` from the eye, `ty`
///   the tangent of half the vertical field of view. At 0.1 m it stays
///   inside 0.25 m while `ty^2 * (1 + aspect^2)` is at most 5.25: a 90°
///   vertical field of view up to an aspect of 2.06 (16:9, 121° across,
///   but not 21:9), or up to 126° across at 16:9. A ceiling at the hull's
///   top would sit 0.15 m above the eye (1.6 m of 1.75 m), which holds
///   while the same sum is at most 1.25 (88° across at 16:9); the room's
///   lowest ceiling, the alcove's 2.5 m, keeps far more.
/// - **Nothing is lost by going lower.** With reversed-Z in `Depth32Float`
///   the precision is nearly the same at every distance, so a smaller near
///   plane costs no depth precision worth having; it only clips what comes
///   closer than it, and rung 2 draws nothing that close (no view model).
pub const NEAR_PLANE: Real = 0.1;

/// A first-person camera: an eye in the world, its heading and elevation,
/// and its lens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FirstPersonCamera {
    /// The eye, in absolute world coordinates. Finite.
    pub eye: WorldPos,
    /// The heading: 0 looks along +X, a quarter turn along +Y.
    pub yaw: BAngle,
    /// The elevation, read as a signed angle (`pitch.0 as i32`): positive
    /// looks up. Any value gives a proper rotation, a quarter turn looking
    /// straight up; mouse look keeps it within ±80° (plan decision 9).
    pub pitch: BAngle,
    /// The full vertical field of view, above 0 and below a half turn.
    pub fov_y: BAngle,
    /// The near plane's distance in meters, finite and above 0. Depth is 1
    /// there.
    pub near: Real,
}

/// The camera's axes in world coordinates: unit vectors, each at right
/// angles to the other two. `right` is level, a quarter turn clockwise of
/// the heading; `up` is `right x forward`, which is +Z when the pitch is 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewBasis {
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
}

/// A 4x4 f32 matrix, stored column by column: the memory layout of WGSL's
/// `mat4x4<f32>`, so a uniform can take [`Mat4::cols`] as they are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    pub cols: [[Real; 4]; 4],
}

/// A position in clip space. The GPU divides by `w`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clip {
    pub x: Real,
    pub y: Real,
    pub z: Real,
    pub w: Real,
}

/// A camera made ready for one image's aspect ratio: the eye, for the f64
/// subtraction, the matrix that takes camera-relative offsets to clip
/// space, and the rays back out through the image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewProj {
    eye: WorldPos,
    matrix: Mat4,
    /// What one unit of clip-space x, then y, adds to a ray of forward
    /// length 1: the right axis times `aspect / f`, the up axis over `f`.
    rays: [Vec3; 2],
}

impl Mat4 {
    /// The matrix whose rows are `rows`.
    pub const fn from_rows(rows: [[Real; 4]; 4]) -> Self {
        let mut cols = [[0.0; 4]; 4];
        let mut r = 0;
        while r < 4 {
            let mut c = 0;
            while c < 4 {
                cols[c][r] = rows[r][c];
                c += 1;
            }
            r += 1;
        }
        Self { cols }
    }

    /// Row `r` of the matrix.
    pub const fn row(&self, r: usize) -> [Real; 4] {
        [
            self.cols[0][r],
            self.cols[1][r],
            self.cols[2][r],
            self.cols[3][r],
        ]
    }

    /// `self * v`, each row summed left to right with no fused operations.
    pub fn mul_vec4(&self, v: [Real; 4]) -> [Real; 4] {
        let c = &self.cols;
        let mut out = [0.0; 4];
        for (r, value) in out.iter_mut().enumerate() {
            *value = c[0][r] * v[0] + c[1][r] * v[1] + c[2][r] * v[2] + c[3][r] * v[3];
        }
        out
    }
}

impl Clip {
    /// Normalized device coordinates `(x, y, depth)`, or `None` when the
    /// point is beside the eye or behind it (`w <= 0`).
    pub fn ndc(self) -> Option<Vec3> {
        if self.w > 0.0 {
            Some(Vec3::new(self.x / self.w, self.y / self.w, self.z / self.w))
        } else {
            None
        }
    }
}

/// The f64 basis, before rounding: forward, right, up.
type Basis64 = [[f64; 3]; 3];

impl FirstPersonCamera {
    /// Checks that the camera can make an image: a finite eye, a field of
    /// view strictly between 0 and a half turn, and a finite near plane
    /// above 0. Otherwise an `[E-SCENE]` error.
    pub fn check(&self) -> Result<(), RenderError> {
        let fail = |problem: String| RenderError::Scene {
            shape: None,
            problem,
        };
        if !self.eye.is_finite() {
            return Err(fail("the camera's eye must be finite".into()));
        }
        if self.fov_y.0 == 0 || self.fov_y.0 >= BAngle::HALF.0 {
            return Err(fail(format!(
                "the camera's vertical field of view is {:#010x} of a turn in 2^32; it must be \
                 above 0 and below a half turn",
                self.fov_y.0
            )));
        }
        if !(self.near.is_finite() && self.near > 0.0) {
            return Err(fail(format!(
                "the camera's near plane is at {} m; it must be finite and above 0",
                self.near
            )));
        }
        Ok(())
    }

    /// The camera's axes in world coordinates, computed in f64 and rounded
    /// to f32.
    pub fn basis(&self) -> ViewBasis {
        let [forward, right, up] = self.basis_f64().map(to_vec3);
        ViewBasis { forward, right, up }
    }

    /// The camera made ready for an image `aspect` times as wide as it is
    /// high (finite and above 0), or an `[E-SCENE]` error for a camera
    /// that [`FirstPersonCamera::check`] refuses or a bad aspect.
    ///
    /// The matrix's rows are the camera's right axis times `f / aspect`,
    /// its up axis times `f`, `(0, 0, 0, near)` and its forward axis, where
    /// `f` is the cotangent of half the field of view. Each is computed in
    /// f64 and rounded to f32 once; an entry too large for f32 (an aspect
    /// near 0, say 1e-39 at 90°) is an `[E-SCENE]` error too.
    pub fn view_proj(&self, aspect: Real) -> Result<ViewProj, RenderError> {
        self.check()?;
        if !(aspect.is_finite() && aspect > 0.0) {
            return Err(RenderError::Scene {
                shape: None,
                problem: format!("an aspect ratio of {aspect}; it must be finite and above 0"),
            });
        }
        let (sin, cos) = sin_cos_f64(self.fov_y.radians_f64() * 0.5);
        let focal = cos / sin;
        let across = focal / f64::from(aspect);
        let [forward, right, up] = self.basis_f64();
        let row = |axis: [f64; 3], scale: f64| {
            [
                (axis[0] * scale) as Real,
                (axis[1] * scale) as Real,
                (axis[2] * scale) as Real,
                0.0,
            ]
        };
        let matrix = Mat4::from_rows([
            row(right, across),
            row(up, focal),
            [0.0, 0.0, 0.0, self.near],
            row(forward, 1.0),
        ]);
        if !matrix.cols.iter().flatten().all(|v| v.is_finite()) {
            return Err(RenderError::Scene {
                shape: None,
                problem: format!(
                    "an aspect ratio of {aspect} with this field of view makes the projection \
                     too large for f32"
                ),
            });
        }
        let ray = |axis: [f64; 3], scale: f64| {
            Vec3::new(
                (axis[0] / scale) as Real,
                (axis[1] / scale) as Real,
                (axis[2] / scale) as Real,
            )
        };
        Ok(ViewProj {
            eye: self.eye,
            matrix,
            rays: [ray(right, across), ray(up, focal)],
        })
    }

    /// Forward, right and up in f64. Forward is yaw's heading raised by
    /// the pitch; right is the heading turned a quarter turn clockwise,
    /// level; up is `right x forward`.
    fn basis_f64(&self) -> Basis64 {
        let (sy, cy) = sin_cos_f64(self.yaw.radians_f64());
        let (sp, cp) = sin_cos_f64(self.pitch.radians_f64());
        [
            [cp * cy, cp * sy, sp],
            [sy, -cy, 0.0],
            [-sp * cy, -sp * sy, cp],
        ]
    }
}

impl ViewProj {
    /// The eye the offsets are taken from.
    pub fn eye(&self) -> WorldPos {
        self.eye
    }

    /// The matrix from a camera-relative offset (`position - eye`, in
    /// world axes, as `(x, y, z, 1)`) to clip space. A shader adds the
    /// offset of a chunk's origin, uploaded per frame, to each vertex and
    /// applies this.
    pub fn matrix(&self) -> Mat4 {
        self.matrix
    }

    /// Where `position` lands in clip space: subtracted from the eye in
    /// f64, rounded to f32, then multiplied by [`ViewProj::matrix`].
    pub fn project(&self, position: WorldPos) -> Clip {
        self.project_offset(position.relative_to(self.eye))
    }

    /// Where the point `offset` from the eye, in world axes, lands in clip
    /// space.
    pub fn project_offset(&self, offset: Vec3) -> Clip {
        let [x, y, z, w] = self.matrix.mul_vec4([offset.x, offset.y, offset.z, 1.0]);
        Clip { x, y, z, w }
    }

    /// The camera's forward axis in world coordinates: the matrix's last
    /// row.
    pub fn forward(&self) -> Vec3 {
        let [x, y, z, _] = self.matrix.row(3);
        Vec3::new(x, y, z)
    }

    /// The near plane's distance, as the camera gave it.
    pub fn near(&self) -> Real {
        self.matrix.row(2)[3]
    }

    /// The ray from the eye through normalized device coordinates (`x`,
    /// `y`), in world axes, scaled so that its part along the forward axis
    /// is 1: the point `t` meters along it lies `t` meters ahead, at depth
    /// `near / t`, and projects back to (`x`, `y`). The sky pass builds
    /// the same ray from the per-frame uniform.
    pub fn ray(&self, x: Real, y: Real) -> Vec3 {
        let [across, up] = self.rays;
        self.forward() + across * x + up * y
    }

    /// The two rays [`ViewProj::ray`] adds to the forward axis per unit of
    /// `x` and of `y`.
    pub(crate) fn ray_steps(&self) -> [Vec3; 2] {
        self.rays
    }
}

fn to_vec3(v: [f64; 3]) -> Vec3 {
    Vec3::new(v[0] as Real, v[1] as Real, v[2] as Real)
}
