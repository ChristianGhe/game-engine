//! The first-person graph's passes (ADR-0020): their transients, their
//! nodes in eng-gpu's sorted graph, and what each records.

use eng_gpu::{
    Format, FullscreenPipeline, LinearRgba, MeshDraw, MeshPipeline, PRESENT, Pass, PassDecl,
    PassNode, PassRecorder, Resource, ScreenPipeline, Transient,
};

use super::world::ChunkDraw;

/// What `opaque` draws its surfaces' colours into: linear albedo.
pub(crate) const ALBEDO: Transient = Transient::new("albedo", Format::Rgba16Float);
/// Each pixel's [`crate::Info`], packed.
pub(crate) const INFO: Transient = Transient::new("info", Format::Rg32Uint);
/// Reversed-Z depth, `near / distance ahead`, 0 where nothing was drawn.
pub(crate) const DEPTH: Transient = Transient::new("depth", Format::Depth32Float);
/// Luminance in nits, scene-referred: what `light` and `sky` draw.
pub(crate) const LIT: Transient = Transient::new("lit", Format::Rgba16Float);
/// `lit` with fog: the format of `lit`, so that fog can be bypassed.
pub(crate) const FOGGED: Transient = Transient::new("fogged", Format::Rgba16Float);
/// The tone-mapped image, display-referred 8-bit sRGB: the format of the
/// output, so that the palette can be bypassed.
pub(crate) const TONED: Transient = Transient::new("toned", Format::Rgba8UnormSrgb);

const OPAQUE_TARGETS: &[Resource] = &[
    Resource::Transient(ALBEDO),
    Resource::Transient(INFO),
    Resource::Transient(DEPTH),
];
const LIT_TARGET: &[Resource] = &[Resource::Transient(LIT)];

const fn node(
    name: &'static str,
    reads: &'static [Resource],
    writes: &'static [Resource],
) -> PassNode {
    PassNode {
        name,
        reads,
        writes,
        continues: None,
        bypassable: false,
    }
}

pub(crate) const OPAQUE: PassNode = node("opaque", &[], OPAQUE_TARGETS);
/// The things' billboards, drawn on top of `opaque`'s three targets
/// (`first_person/sprites.rs`).
pub(crate) const SPRITES: PassNode = PassNode {
    continues: Some("opaque"),
    ..node("sprites", &[], OPAQUE_TARGETS)
};
pub(crate) const LIGHT: PassNode = node("light", OPAQUE_TARGETS, LIT_TARGET);
pub(crate) const SKY: PassNode = PassNode {
    continues: Some("light"),
    ..node("sky", &[Resource::Transient(DEPTH)], LIT_TARGET)
};
/// Its first read is its input: switched off, `fogged` takes `lit`'s
/// texture.
pub(crate) const FOG: PassNode = PassNode {
    bypassable: true,
    ..node(
        "fog",
        &[
            Resource::Transient(LIT),
            Resource::Transient(INFO),
            Resource::Transient(DEPTH),
        ],
        &[Resource::Transient(FOGGED)],
    )
};
pub(crate) const TONEMAP: PassNode = node(
    "tonemap",
    &[Resource::Transient(FOGGED)],
    &[Resource::Transient(TONED)],
);
/// Switched off, the tone map draws `toned` into the output.
pub(crate) const PALETTE: PassNode = PassNode {
    bypassable: true,
    ..node(
        "palette",
        &[Resource::Transient(TONED)],
        &[Resource::Output],
    )
};
/// The plain pipeline's tone map: straight from `lit` into the output.
pub(crate) const PLAIN_TONEMAP: PassNode = node("tonemap", LIT_TARGET, &[Resource::Output]);

/// The look's graph, as declared: [`FirstPersonRenderer`]'s frames give
/// their passes in this order.
///
/// [`FirstPersonRenderer`]: crate::FirstPersonRenderer
pub(crate) const LOOK_NODES: [PassNode; 7] = [OPAQUE, SPRITES, LIGHT, SKY, FOG, TONEMAP, PALETTE];
pub(crate) const LOOK_NODES_PRESENTED: [PassNode; 8] = [
    OPAQUE,
    SPRITES,
    LIGHT,
    SKY,
    FOG,
    TONEMAP,
    PALETTE,
    PRESENT.node(),
];
/// The plain pipeline's graph: no fog and no palette at all.
pub(crate) const PLAIN_NODES: [PassNode; 5] = [OPAQUE, SPRITES, LIGHT, SKY, PLAIN_TONEMAP];
pub(crate) const PLAIN_NODES_PRESENTED: [PassNode; 6] =
    [OPAQUE, SPRITES, LIGHT, SKY, PLAIN_TONEMAP, PRESENT.node()];

/// The opaque pass: everything cleared (albedo and `info` to 0, depth to
/// the far plane), then each chunk drawn with its own record, the frame's
/// records from `first_record` on.
pub(crate) struct Opaque<'a> {
    pub(crate) pipeline: &'a MeshPipeline,
    pub(crate) chunks: &'a [ChunkDraw],
    pub(crate) first_record: u32,
}

impl Pass for Opaque<'_> {
    fn decl(&self) -> PassDecl {
        OPAQUE.decl()
    }

    fn record(&mut self, rec: &mut PassRecorder) {
        rec.clear(LinearRgba::TRANSPARENT);
        for (record, chunk) in (self.first_record..).zip(self.chunks) {
            rec.mesh(
                self.pipeline,
                MeshDraw {
                    indices: chunk.indices.clone(),
                    base_vertex: chunk.base_vertex,
                    instances: record..record + 1,
                },
            );
        }
    }
}

/// A pass of one screen draw.
pub(crate) struct Screen<'a>(pub(crate) PassNode, pub(crate) &'a ScreenPipeline);

impl Pass for Screen<'_> {
    fn decl(&self) -> PassDecl {
        self.0.decl()
    }

    fn record(&mut self, rec: &mut PassRecorder) {
        rec.screen(self.1);
    }
}

/// The present blit, rung 1b's `present.wgsl`: the output drawn into the
/// window's image, texel for texel, or k×k for upscaled targets, whose
/// window may show a border under k pixels wide, cleared to opaque black.
pub(crate) struct Present<'a>(pub(crate) &'a FullscreenPipeline);

impl Pass for Present<'_> {
    fn decl(&self) -> PassDecl {
        PRESENT
    }

    fn record(&mut self, rec: &mut PassRecorder) {
        rec.clear(LinearRgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
        rec.fullscreen(self.0);
    }
}
