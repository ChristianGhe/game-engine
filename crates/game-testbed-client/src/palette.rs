//! The room's palette (rung 2 plan, decision 8): GIMP's plain-text `.gpl`
//! format, game data in `palettes/room.gpl`, read by eng-render's strict
//! reader and baked into the palette look's table.
//!
//! The file is made by a script, `palettes/room_gpl.py`, and not by hand:
//! ramps of shades for the room's materials, its sky, its fog and its
//! things, in integer arithmetic so any Python 3 writes the same bytes. It
//! is not Doom's palette (id Software's work); spec 11 asks for its tropes
//! only. `palettes/room.gpl.provenance.toml` is its provenance sidecar
//! (spec 6.11): who made it, with what, under which licence, and that it is
//! a placeholder until the owner looks at it (gate G7).

use eng_render::{GplPalette, PaletteLut};

/// The palette file, as committed.
pub const ROOM_GPL: &str = include_str!("../palettes/room.gpl");

/// Its provenance sidecar.
pub const ROOM_GPL_PROVENANCE: &str = include_str!("../palettes/room.gpl.provenance.toml");

/// The room's palette colours, as the file gives them.
///
/// # Panics
/// If the committed file does not read: its tests catch that first.
pub fn colours() -> GplPalette {
    GplPalette::parse(ROOM_GPL).unwrap_or_else(|e| panic!("palettes/room.gpl: {e}"))
}

/// The palette look's table, baked from the file.
///
/// # Panics
/// As [`colours`].
pub fn room() -> PaletteLut {
    PaletteLut::from_gpl(ROOM_GPL).unwrap_or_else(|e| panic!("palettes/room.gpl: {e}"))
}

#[cfg(test)]
mod tests {
    use eng_render::{MAX_PALETTE_COLOURS, Srgb8};

    use super::{ROOM_GPL, ROOM_GPL_PROVENANCE, colours, room};

    #[test]
    fn the_palette_reads_and_bakes() {
        let palette = colours();
        assert_eq!(palette.name.as_deref(), Some("Testbed room"));
        assert_eq!(palette.colours.len(), MAX_PALETTE_COLOURS);
        assert_eq!(palette.colours[0], Srgb8::new(0, 0, 0));
        assert!(palette.colours.contains(&Srgb8::new(255, 255, 255)));
        let mut sorted = palette.colours.clone();
        sorted.sort_by_key(|c| (c.r, c.g, c.b));
        sorted.dedup();
        assert_eq!(sorted.len(), palette.colours.len(), "no colour twice");
        assert_eq!(room().colours(), &palette.colours[..]);
        assert!(ROOM_GPL.contains("room_gpl.py"), "it names its script");
    }

    /// The sidecar names the asset and its maker, and says what spec 6.11
    /// asks of it.
    #[test]
    fn the_provenance_sidecar_says_how_the_palette_was_made() {
        let sidecar: toml::Table = ROOM_GPL_PROVENANCE.parse().unwrap();
        for key in [
            "version",
            "asset",
            "source",
            "made_by",
            "tool",
            "licence",
            "licence_date",
            "human_edits",
            "reviewer",
            "status",
        ] {
            assert!(sidecar.contains_key(key), "{key}");
        }
        assert_eq!(sidecar["asset"].as_str(), Some("room.gpl"));
        assert_eq!(sidecar["source"].as_str(), Some("generated"));
        assert_eq!(sidecar["status"].as_str(), Some("placeholder"));
        assert!(
            sidecar["tool"]
                .as_str()
                .is_some_and(|t| t.contains("room_gpl.py"))
        );
    }
}
