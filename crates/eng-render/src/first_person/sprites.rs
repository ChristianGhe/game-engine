//! The things' sprites (rung 2, T11; plan decision 4; ADR-0020): the
//! `sprites` pass, which continues `opaque` and draws each thing as a
//! billboard into its three targets (`sprites.wgsl`).
//!
//! - **A billboard turns about Z only.** Its quad stands upright on the
//!   thing's feet along the camera's right axis, which is level, so a
//!   camera pitched up or down does not tilt it: [`Sprite::width`] across,
//!   centred on the feet, and [`Sprite::height`] up +Z. The texture's left
//!   edge is on the image's left, its top row at the top.
//! - **The rotation** a thing shows comes from its facing and the eye, on
//!   the CPU each frame, by Q4's rule ([`crate::sprite_rotation_toward`]);
//!   its [`RotationSheet`] gives the frame to draw and whether to mirror it
//!   left to right ([`Sprite::frame_toward`]), and the frame names the
//!   material.
//! - **Alpha test and depth.** A material with an alpha test (bit 0 of its
//!   flags, [`crate::Material::alpha_test`]) discards texels whose alpha is
//!   below one half. What is left is tested against the depth `opaque`
//!   drew and writes albedo, `info` and depth, so a thing behind a pillar
//!   is hidden and a thing in front of a wall hides it.
//! - **`info`** takes [`SurfaceClass::Sprite`], the thing's region and its
//!   id: `light` and `fog` then light and fog it as a surface of its
//!   region. Fake contrast is for walls only, so none applies.
//!
//! **Records.** A sprite needs more than an [`InstanceRecord`]'s offset and
//! material, and all of it may change from frame to frame, so each takes
//! two of the frame's records, sprite `i` at `2i` and `2i + 1`, before the
//! chunks' records:
//! - the first, as a chunk's: its feet minus the eye, taken in f64, and the
//!   material of the frame shown;
//! - the second: its width (negative when mirrored, which runs the texture
//!   right to left), its height and its region as the `offset`, and its id
//!   as the `material`.
//!
//! One draw takes every sprite: the world's six billboard indices, whose
//! corners the shader makes from the vertex index, once per sprite. A
//! frame without sprites has no draw in the pass.

use std::ops::Range;

use eng_gpu::{
    Gpu, GpuError, InstanceRecord, MeshDesc, MeshDraw, MeshPipeline, Pass, PassDecl, PassRecorder,
};
use eng_math::{BAngle, Real, WorldPos};

use super::passes::SPRITES;
use crate::{RenderError, RotationSheet, SheetFrame, SurfaceClass, sprite_rotation_toward};

const SPRITES_WGSL: &str = include_str!("../sprites.wgsl");

/// One thing as a frame of the first-person view draws it: a billboard of
/// one frame of its rotation sheet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprite {
    /// The thing's id, which `info` carries at its pixels with
    /// [`SurfaceClass::Sprite`]. Ids are keyed by class and id together, so
    /// a thing may share its number with a world surface.
    pub id: u32,
    /// The region whose light and fog it takes, as [`crate::Info::region`].
    pub region: u16,
    /// The point it stands on, the middle of its billboard's bottom edge,
    /// in absolute world coordinates. Finite.
    pub feet: WorldPos,
    /// Which way it faces. With the eye, it picks the rotation shown.
    pub facing: BAngle,
    /// The billboard's width in meters, along the camera's right axis.
    /// Finite and above 0.
    pub width: Real,
    /// The billboard's height in meters, up +Z from the feet. Finite and
    /// above 0.
    pub height: Real,
    pub sheet: RotationSheet,
    /// The material of each frame of the sheet, frame `k` at `k`, as
    /// indices into the world's material table; a five-rotation sheet reads
    /// the first five. A sprite's materials normally alpha test, so that its
    /// shape is cut out of its square texture.
    pub materials: [u32; 8],
}

impl Sprite {
    /// The frame of its sheet that an eye at `eye` sees, and whether it is
    /// drawn mirrored: [`RotationSheet::frame`] of the rotation toward the
    /// eye.
    pub fn frame_toward(&self, eye: WorldPos) -> SheetFrame {
        self.sheet
            .frame(sprite_rotation_toward(self.feet, self.facing, eye))
    }

    /// The material drawn for an eye at `eye`: that of
    /// [`Sprite::frame_toward`]'s frame.
    pub fn material_toward(&self, eye: WorldPos) -> u32 {
        self.materials[usize::from(self.frame_toward(eye).frame)]
    }
}

/// Whether `sprites` can be drawn from a world of `materials` materials: an
/// `[E-SCENE]` error naming the first that cannot, unless each has finite
/// feet, a finite size above 0, and materials the world has for every
/// frame of its sheet.
pub(crate) fn check(sprites: &[Sprite], materials: usize) -> Result<(), RenderError> {
    let fail = |at: usize, problem: String| RenderError::Scene {
        shape: None,
        problem: format!("sprite {at}: {problem}"),
    };
    if u32::try_from(sprites.len().saturating_mul(2)).is_err() {
        return Err(RenderError::Scene {
            shape: None,
            problem: format!("{} sprites are more than a frame can count", sprites.len()),
        });
    }
    for (at, sprite) in sprites.iter().enumerate() {
        if !sprite.feet.is_finite() {
            return Err(fail(at, "its feet must be finite".into()));
        }
        let size = [sprite.width, sprite.height];
        if !size.iter().all(|s| s.is_finite() && *s > 0.0) {
            return Err(fail(
                at,
                format!(
                    "its size is {} x {} m; both must be finite and above 0",
                    sprite.width, sprite.height
                ),
            ));
        }
        let frames = usize::from(sprite.sheet.frames());
        if let Some(material) = sprite.materials[..frames]
            .iter()
            .find(|&&m| m as usize >= materials)
        {
            return Err(fail(
                at,
                format!("it names material {material}, but the world has {materials}"),
            ));
        }
    }
    Ok(())
}

/// The frame's two records for each of `sprites`, seen from `eye` (see the
/// top of this file).
pub(crate) fn records(sprites: &[Sprite], eye: WorldPos) -> Vec<InstanceRecord> {
    let mut records = Vec::with_capacity(sprites.len() * 2);
    for sprite in sprites {
        let shown = sprite.frame_toward(eye);
        let feet = sprite.feet;
        records.push(InstanceRecord::relative(
            [feet.x, feet.y, feet.z],
            [eye.x, eye.y, eye.z],
            sprite.materials[usize::from(shown.frame)],
        ));
        let width = if shown.mirrored {
            -sprite.width
        } else {
            sprite.width
        };
        records.push(InstanceRecord {
            offset: [width, sprite.height, f32::from(sprite.region)],
            material: sprite.id,
        });
    }
    records
}

/// The sprites' pipeline: no vertex attributes (the corners come from the
/// vertex index), the pass's three targets, depth written, both sides
/// drawn (a mirrored billboard runs clockwise).
pub(crate) fn pipeline(gpu: &Gpu) -> Result<MeshPipeline, GpuError> {
    debug_assert_eq!(SurfaceClass::Sprite.code(), 6, "sprites.wgsl's SPRITE");
    gpu.mesh_pipeline(&MeshDesc {
        label: "sprites",
        wgsl: SPRITES_WGSL,
        vertex_entry: "sprite_vertex",
        fragment_entry: "sprite_fragment",
        vertex: &[],
        reads: SPRITES.reads,
        writes: SPRITES.writes,
        depth_write: true,
        cull_back: false,
    })
}

/// The sprites pass: one draw of the shared billboard, once for each of
/// `count` sprites, on top of `opaque`'s targets.
pub(crate) struct Sprites<'a> {
    pub(crate) pipeline: &'a MeshPipeline,
    pub(crate) billboard: Range<u32>,
    pub(crate) count: u32,
}

impl Pass for Sprites<'_> {
    fn decl(&self) -> PassDecl {
        SPRITES.decl()
    }

    fn record(&mut self, rec: &mut PassRecorder) {
        rec.mesh(
            self.pipeline,
            MeshDraw {
                indices: self.billboard.clone(),
                base_vertex: 0,
                instances: 0..self.count,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use eng_math::{BAngle, WorldPos};

    use super::{Sprite, check, records};
    use crate::{RenderError, RotationSheet};

    fn sprite(sheet: RotationSheet) -> Sprite {
        Sprite {
            id: 0xDEAD_BEEF,
            region: 3,
            feet: WorldPos::new(1.0e9 + 0.5, -2.0, 0.25),
            facing: BAngle::QUARTER,
            width: 1.25,
            height: 2.0,
            sheet,
            materials: [10, 11, 12, 13, 14, 15, 16, 17],
        }
    }

    /// An eye `eighths` eighths of a turn counter-clockwise of the sprite's
    /// facing, 4 m out on an axis or a diagonal: it sees rotation `eighths`.
    fn eye(sprite: &Sprite, eighths: u32) -> WorldPos {
        let turn = (sprite.facing.0 >> 29).wrapping_add(eighths) % 8;
        let (dx, dy) = [
            (1.0, 0.0),
            (1.0, 1.0),
            (0.0, 1.0),
            (-1.0, 1.0),
            (-1.0, 0.0),
            (-1.0, -1.0),
            (0.0, -1.0),
            (1.0, -1.0),
        ][turn as usize];
        WorldPos::new(sprite.feet.x + 4.0 * dx, sprite.feet.y + 4.0 * dy, 1.6)
    }

    #[test]
    fn each_sprite_takes_two_records_its_place_and_its_shape() {
        let eight = sprite(RotationSheet::Eight);
        let five = Sprite {
            id: 7,
            region: 0,
            ..sprite(RotationSheet::Five)
        };
        for k in 0..8 {
            let eye = eye(&eight, k);
            let r = records(&[eight, five], eye);
            assert_eq!(r.len(), 4);
            // Feet minus the eye in f64: the half meter survives 1e9 m out.
            let (dx, dy) = (eight.feet.x - eye.x, eight.feet.y - eye.y);
            assert_eq!(r[0].offset, [dx as f32, dy as f32, 0.25 - 1.6]);
            assert_eq!(r[0].material, 10 + k);
            assert_eq!(
                (r[1].offset, r[1].material),
                ([1.25, 2.0, 3.0], 0xDEAD_BEEF)
            );
            // Five frames: 5, 6 and 7 are 3, 2 and 1, mirrored, which the
            // width's sign says.
            let (frame, mirrored) = if k > 4 { (8 - k, true) } else { (k, false) };
            assert_eq!(r[2].material, 10 + frame, "rotation {k}");
            let width = if mirrored { -1.25 } else { 1.25 };
            assert_eq!((r[3].offset, r[3].material), ([width, 2.0, 0.0], 7));
            assert_eq!(five.material_toward(eye), 10 + frame);
        }
        assert!(records(&[], WorldPos::new(0.0, 0.0, 0.0)).is_empty());
    }

    #[test]
    fn a_sprite_the_frame_cannot_draw_is_refused() {
        let problem = |bad: Sprite| match check(&[sprite(RotationSheet::Eight), bad], 18) {
            Err(RenderError::Scene { problem, .. }) => problem,
            other => panic!("{other:?}"),
        };
        assert!(check(&[sprite(RotationSheet::Eight)], 18).is_ok());
        let good = sprite(RotationSheet::Eight);
        assert_eq!(
            problem(Sprite {
                feet: WorldPos::new(f64::NAN, 0.0, 0.0),
                ..good
            }),
            "sprite 1: its feet must be finite"
        );
        assert_eq!(
            problem(Sprite { width: 0.0, ..good }),
            "sprite 1: its size is 0 x 2 m; both must be finite and above 0"
        );
        assert_eq!(
            problem(Sprite {
                height: f32::INFINITY,
                ..good
            }),
            "sprite 1: its size is 1.25 x inf m; both must be finite and above 0"
        );
        assert_eq!(
            problem(Sprite {
                materials: [0, 1, 2, 3, 4, 5, 6, 18],
                ..good
            }),
            "sprite 1: it names material 18, but the world has 18"
        );
        // A five-rotation sheet reads only its first five.
        let five = Sprite {
            sheet: RotationSheet::Five,
            materials: [0, 1, 2, 3, 4, 99, 99, 99],
            ..good
        };
        assert!(check(&[five], 18).is_ok());
    }
}
