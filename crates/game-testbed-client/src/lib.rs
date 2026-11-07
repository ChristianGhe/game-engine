//! The testbed's presentation (spec 6.1, the games row; rung 1b plan,
//! difference 6): what the renderer is given each frame, extracted from the
//! simulation's state, which it reads and never writes. Nothing here is
//! simulation, so nothing here changes a hash stream.
//!
//! - [`top_down`]: the dot map's top-down view, moved here from
//!   `game-testbed` unchanged (rung 1b's four shots stay pixel for pixel).
//! - [`room`]: the room's first-person view ([`ROOM`]): chunks of quads
//!   from its grid, region looks (the alcove's green fog), the sky through
//!   the skylight, one sprite per thing, and player 0's camera.
//! - [`textures`] and [`sprites`]: the surfaces' textures and the things'
//!   sprite frames, made by code (plan, P4 option A): no image file in the
//!   repo. The sprites are rendered on the CPU from small 3D primitives.
//! - [`palette`]: the plain-text palette, `palettes/room.gpl`, made by its
//!   script `palettes/room_gpl.py`, with its provenance sidecar.
#![forbid(unsafe_code)]

pub mod palette;
pub mod room;
pub mod sprites;
pub mod textures;
pub mod top_down;

/// The room map's view, for its `GameMap` (rung 2, T12).
pub const ROOM: eng_cli::FirstPersonView = eng_cli::FirstPersonView {
    world: room::world,
    scene: room::scene,
};
