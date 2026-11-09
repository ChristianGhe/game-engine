//! `--look look.toml` (rung 2, T15, onto T10's switches): which look
//! features a first-person view draws. Every key is optional; a key that
//! is given sets that field of eng-render's [`Look`] over the scene's own
//! (every feature on for the testbed's room), and the rest stay as the
//! scene has them. A key the file does not know is an error, as in every
//! authored file.
//!
//! ```toml
//! version = 1
//! falloff = true        # light falls off with distance
//! banding = true        # light in 32 whole steps
//! contrast = true       # Doom's fake contrast on walls
//! sky_gradient = true   # the sky from horizon to zenith colour
//! fog = true            # the fog pass
//! palette = true        # the palette pass
//! dither = true         # the palette's ordered dither
//! low_resolution = 4    # each texel as 4x4 window pixels; 1 is off
//! ```

use eng_render::Look;
use eng_schema_json::{Authored, LoadError, load_toml};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LookFile {
    /// Checked by `load_toml`.
    #[serde(rename = "version")]
    _version: u32,
    falloff: Option<bool>,
    banding: Option<bool>,
    contrast: Option<bool>,
    sky_gradient: Option<bool>,
    fog: Option<bool>,
    palette: Option<bool>,
    dither: Option<bool>,
    low_resolution: Option<u32>,
}

impl Authored for LookFile {
    const KIND: &'static str = "look file";
    const VERSION: u32 = 1;
}

/// The look switches a `--look` file gives: each `Some` sets that field of
/// a scene's [`Look`], each `None` leaves it as the scene has it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LookSwitches {
    pub falloff: Option<bool>,
    pub banding: Option<bool>,
    pub contrast: Option<bool>,
    pub sky_gradient: Option<bool>,
    pub fog: Option<bool>,
    pub palette: Option<bool>,
    pub dither: Option<bool>,
    /// At least 1.
    pub low_resolution: Option<u32>,
}

impl LookSwitches {
    /// Reads a look file; `origin` names it in errors (`[E-LOAD]`).
    pub(crate) fn parse(text: &str, origin: &str) -> Result<Self, LoadError> {
        let file: LookFile = load_toml(text, origin)?;
        if file.low_resolution == Some(0) {
            return Err(LoadError {
                origin: origin.to_string(),
                message: "low_resolution must be at least 1 (1 is off)".into(),
            });
        }
        Ok(Self {
            falloff: file.falloff,
            banding: file.banding,
            contrast: file.contrast,
            sky_gradient: file.sky_gradient,
            fog: file.fog,
            palette: file.palette,
            dither: file.dither,
            low_resolution: file.low_resolution,
        })
    }

    /// `look` with the switches given set.
    pub fn apply(&self, look: Look) -> Look {
        Look {
            falloff: self.falloff.unwrap_or(look.falloff),
            banding: self.banding.unwrap_or(look.banding),
            contrast: self.contrast.unwrap_or(look.contrast),
            sky_gradient: self.sky_gradient.unwrap_or(look.sky_gradient),
            fog: self.fog.unwrap_or(look.fog),
            palette: self.palette.unwrap_or(look.palette),
            dither: self.dither.unwrap_or(look.dither),
            low_resolution: self.low_resolution.unwrap_or(look.low_resolution),
        }
    }
}

#[cfg(test)]
mod tests {
    use eng_render::Look;

    use super::LookSwitches;

    #[test]
    fn no_switch_keeps_the_scenes_look() {
        let none = LookSwitches::parse("version = 1\n", "l.toml").unwrap();
        assert_eq!(none, LookSwitches::default());
        assert_eq!(none.apply(Look::ON), Look::ON);
        assert_eq!(none.apply(Look::OFF), Look::OFF);
    }

    #[test]
    fn each_switch_sets_its_field_alone() {
        let text = "version = 1\nfog = false\nlow_resolution = 1\n";
        let look = LookSwitches::parse(text, "l.toml").unwrap().apply(Look::ON);
        assert_eq!(
            look,
            Look {
                fog: false,
                low_resolution: 1,
                ..Look::ON
            }
        );
        let every = "version = 1\nfalloff = false\nbanding = false\ncontrast = false\n\
                     sky_gradient = false\nfog = false\npalette = false\ndither = false\n\
                     low_resolution = 1\n";
        let off = LookSwitches::parse(every, "l.toml").unwrap();
        assert_eq!(off.apply(Look::ON), Look::OFF);
        let on = every.replace("false", "true").replace("= 1\n", "= 4\n");
        let on = on.replacen("version = 4", "version = 1", 1);
        assert_eq!(
            LookSwitches::parse(&on, "l.toml").unwrap().apply(Look::OFF),
            Look::ON
        );
    }

    #[test]
    fn a_bad_file_is_refused() {
        for (text, wanted) in [
            ("fog = false\n", "version"),
            ("version = 2\n", "version"),
            ("version = 1\nfogg = false\n", "fogg"),
            ("version = 1\nfog = 1\n", "fog"),
            ("version = 1\nlow_resolution = 0\n", "at least 1"),
            ("version = 1\nlow_resolution = -2\n", "low_resolution"),
        ] {
            let err = LookSwitches::parse(text, "l.toml").unwrap_err().to_string();
            assert!(
                err.starts_with("[E-LOAD] l.toml: ") && err.contains(wanted),
                "{text:?}: {err}"
            );
        }
    }
}
