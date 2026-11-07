//! A map's view (rung 2, T12): what `shot` and `video` draw of a map, named
//! per map on its [`crate::GameMap`] beside [`GameDef`], whose fields stay
//! as they are (`twin.rs` builds one by literal).
//!
//! - [`MapView::TopDown`] is 1b's view, [`GameDef::top_down`]: the first
//!   map's, always. A later map that names it is drawn by that same
//!   function, which must then handle the map (the testbed's reads the dot
//!   room's `BoxRoom`, so its room is first person).
//! - [`MapView::FirstPerson`] is eng-render's first-person view: the game
//!   builds the map's world once from its sim at tick 0, whose constant
//!   resources hold the geometry ([`FirstPersonView::world`]), and each
//!   frame's scene from the states, as the top-down view does
//!   ([`FirstPersonView::scene`]).
//! - [`MapView::None`]: `shot`, `video` and `play` refuse the map before a
//!   GPU or a window opens (exit 64, `[E-MAP]`).
//!
//! `play` shows the top-down view only, until rung 2's T15 opens the
//! first-person window, so it refuses a first-person map the same way. The
//! scene is the game's at the frame's alpha; what the probe's own flags
//! choose (the live view angles, the field of view, `--look`) it sets on
//! the returned scene's `camera` and `look`, so a [`FirstPersonFrame`]
//! carries only the states, as the top-down view's does.

use eng_gpu::Gpu;
use eng_render::{
    Chunk, ColourTag, FirstPersonScene, GpuWorld, Material, PaletteLut, RenderError, SceneTexture,
    WorldMesh,
};
use eng_sim::Sim;

use crate::{GameDef, TopDownFrame};

/// What a first-person view's scene is built from: the two states, the
/// frame's alpha and the image's size, as for the top-down view. `shot`
/// and `video` pass one tick's state as both, at alpha 1.
pub type FirstPersonFrame<'a> = TopDownFrame<'a>;

/// How `shot` and `video` draw a map.
#[derive(Debug, Clone, Copy)]
pub enum MapView {
    /// The game's top-down view, [`GameDef::top_down`].
    TopDown,
    /// A first-person view, through eng-render's `FirstPersonRenderer`.
    FirstPerson(FirstPersonView),
    /// No view: `shot`, `video` and `play` refuse the map.
    None,
}

/// A map's first-person view: its world, loaded once, and each frame's
/// scene. Both read the sim and never write to it.
#[derive(Debug, Clone, Copy)]
pub struct FirstPersonView {
    /// The world from the map's sim at tick 0: its chunks, materials,
    /// textures and palette. `shot` and `video` load it onto the GPU once,
    /// before the first frame; one that eng-render refuses is a bug in the
    /// game and panics (`[E-SCENE]`, exit 12).
    pub world: fn(&Sim) -> FirstPersonWorld,
    /// One frame's camera, sky, regions, look and sprites.
    pub scene: fn(&FirstPersonFrame<'_>) -> FirstPersonScene,
}

/// A first-person world as a game builds it, owning what eng-render's
/// [`WorldMesh`] borrows, plus the palette look's table.
#[derive(Debug, Clone, PartialEq)]
pub struct FirstPersonWorld {
    /// Names the world's buffers and textures in GPU debuggers.
    pub label: &'static str,
    pub chunks: Vec<Chunk>,
    pub materials: Vec<Material>,
    pub textures: Vec<WorldTexture>,
    /// The palette look's table; without one the palette must be off.
    pub palette: Option<PaletteLut>,
}

/// A square texture of a [`FirstPersonWorld`], owned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldTexture {
    /// 64, 128 or 256.
    pub side: u32,
    pub tag: ColourTag,
    /// `side * side * 4` bytes, rows top first.
    pub rgba8: Vec<u8>,
}

impl FirstPersonWorld {
    /// Its textures as eng-render borrows them.
    pub fn scene_textures(&self) -> Vec<SceneTexture<'_>> {
        self.textures
            .iter()
            .map(|t| SceneTexture {
                side: t.side,
                tag: t.tag,
                rgba8: &t.rgba8,
            })
            .collect()
    }

    /// The mesh eng-render uploads, over `textures` (from
    /// [`FirstPersonWorld::scene_textures`]).
    pub fn mesh<'a>(&'a self, textures: &'a [SceneTexture<'a>]) -> WorldMesh<'a> {
        WorldMesh {
            label: self.label,
            chunks: &self.chunks,
            materials: &self.materials,
            textures,
        }
    }

    /// Uploads it onto `gpu`, once: [`GpuWorld::load_with_palette`] with its
    /// palette, else [`GpuWorld::load`]. `shot` and `video` do this before
    /// the first frame.
    pub fn load(&self, gpu: &Gpu) -> Result<GpuWorld, RenderError> {
        let textures = self.scene_textures();
        let mesh = self.mesh(&textures);
        match &self.palette {
            Some(palette) => GpuWorld::load_with_palette(gpu, &mesh, palette),
            None => GpuWorld::load(gpu, &mesh),
        }
    }
}

impl MapView {
    /// Why `shot` and `video` cannot draw a map of this view, if they
    /// cannot: the `[E-USAGE] [E-MAP]` line.
    pub(crate) fn refusal(&self, game: &GameDef, map: &str) -> Option<String> {
        matches!(self, Self::None).then(|| {
            format!(
                "[E-USAGE] [E-MAP] {}'s map {map:?} has no view to draw",
                game.name
            )
        })
    }

    /// Why `play`'s window cannot show a map of this view yet, if it
    /// cannot: it draws [`GameDef::top_down`] only.
    pub(crate) fn window_refusal(&self, game: &GameDef, map: &str) -> Option<String> {
        match self {
            Self::TopDown => None,
            Self::FirstPerson(_) => Some(format!(
                "[E-USAGE] [E-MAP] {}'s map {map:?} has a first-person view, which play cannot \
                 show yet (its window draws GameDef::top_down only)",
                game.name
            )),
            Self::None => self.refusal(game, map),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use eng_render::{ColourTag, Material};
    use eng_sim::{ScheduleError, Sim, SimBuilder};

    use super::{FirstPersonFrame, FirstPersonView, FirstPersonWorld, MapView, WorldTexture};
    use crate::{GameDef, UNKNOWN};

    fn world(sim: &Sim) -> FirstPersonWorld {
        let _ = sim;
        unreachable!("never loaded here")
    }

    fn scene(frame: &FirstPersonFrame<'_>) -> eng_render::FirstPersonScene {
        let _ = frame;
        unreachable!("never drawn here")
    }

    /// A first-person view that is never drawn, for maps in tests.
    pub(crate) const FIRST_PERSON: MapView = MapView::FirstPerson(FirstPersonView { world, scene });

    fn build(config: eng_sim::SimConfig) -> Result<Sim, ScheduleError> {
        SimBuilder::new(config).build()
    }

    const GAME: GameDef = GameDef {
        name: "views",
        version: "0",
        build_sha: UNKNOWN,
        tick_rate: 60,
        build,
        top_down: None,
        view_yaw: 0,
    };

    #[test]
    fn each_view_says_where_it_cannot_be_drawn() {
        assert_eq!(MapView::TopDown.refusal(&GAME, "a"), None);
        assert_eq!(MapView::TopDown.window_refusal(&GAME, "a"), None);
        assert_eq!(FIRST_PERSON.refusal(&GAME, "b"), None);
        assert_eq!(
            FIRST_PERSON.window_refusal(&GAME, "b").as_deref(),
            Some(
                "[E-USAGE] [E-MAP] views's map \"b\" has a first-person view, which play cannot \
                 show yet (its window draws GameDef::top_down only)"
            )
        );
        let none = "[E-USAGE] [E-MAP] views's map \"c\" has no view to draw";
        assert_eq!(MapView::None.refusal(&GAME, "c").as_deref(), Some(none));
        assert_eq!(
            MapView::None.window_refusal(&GAME, "c").as_deref(),
            Some(none)
        );
    }

    #[test]
    fn a_world_lends_its_parts_as_eng_render_borrows_them() {
        let built = FirstPersonWorld {
            label: "one",
            chunks: Vec::new(),
            materials: vec![Material {
                texture: 0,
                alpha_test: true,
            }],
            textures: vec![WorldTexture {
                side: 64,
                tag: ColourTag::Linear,
                rgba8: vec![7; 64 * 64 * 4],
            }],
            palette: None,
        };
        let textures = built.scene_textures();
        assert_eq!((textures[0].side, textures[0].tag), (64, ColourTag::Linear));
        assert_eq!(textures[0].rgba8, &built.textures[0].rgba8[..]);
        let mesh = built.mesh(&textures);
        assert_eq!(
            (mesh.label, mesh.materials.len(), mesh.textures.len()),
            ("one", 1, 1)
        );
    }
}
