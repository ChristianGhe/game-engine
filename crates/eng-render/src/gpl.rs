//! The plain-text palette reader (spec 6.7; rung 2 plan, decision 8; task
//! T10): GIMP's open `.gpl` format, read strictly, so that a palette file
//! means one thing and a mistake in it is an error naming its line.
//!
//! ```text
//! GIMP Palette
//! Name: Room
//! Columns: 16
//! # a comment
//!   0   0   0 Black
//! 255 255 255 White
//! ```
//!
//! - The first line is exactly `GIMP Palette`.
//! - Before the first colour, at most one `Name: <text>` and one
//!   `Columns: <0 to 256>` line, in either order.
//! - A colour is a line of three decimal codes, red, green and blue, each 0
//!   to 255, apart by spaces or tabs, then an optional name.
//! - Lines starting with `#` are comments, and blank lines are skipped,
//!   anywhere. Lines end in `\n` or `\r\n`.
//! - Anything else is refused, as is a palette of no colour or of more than
//!   [`MAX_PALETTE_COLOURS`], which the lookup table's one-byte indices
//!   cannot hold.

use crate::palette::{MAX_PALETTE_COLOURS, PaletteError, PaletteLut, Srgb8};

/// What a `.gpl` file says: its name and column count, if it gives them,
/// and its colours in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GplPalette {
    pub name: Option<String>,
    /// How many columns GIMP shows it in; 0 lets GIMP choose.
    pub columns: Option<u32>,
    /// 1 to [`MAX_PALETTE_COLOURS`] of them.
    pub colours: Vec<Srgb8>,
}

/// The most columns GIMP accepts.
const MAX_COLUMNS: u32 = 256;

impl GplPalette {
    /// Reads `text` by the rules at the top of `gpl.rs`. A line that breaks
    /// them is an [`PaletteError::Format`] naming it (`[E-PALETTE] line
    /// n: ...`); no colour is [`PaletteError::Empty`] and too many
    /// [`PaletteError::TooMany`].
    pub fn parse(text: &str) -> Result<Self, PaletteError> {
        let mut lines = text.split('\n').enumerate().map(|(index, line)| {
            let line = line.strip_suffix('\r').unwrap_or(line);
            (index + 1, line)
        });
        let fail = |line: usize, problem: String| PaletteError::Format { line, problem };
        match lines.next() {
            Some((_, "GIMP Palette")) => {}
            _ => {
                return Err(fail(
                    1,
                    "a GIMP palette starts with the line `GIMP Palette`".into(),
                ));
            }
        }
        let mut palette = Self {
            name: None,
            columns: None,
            colours: Vec::new(),
        };
        for (number, line) in lines {
            let trimmed = line.trim_matches([' ', '\t']);
            if trimmed.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix("Name:") {
                palette.header(number, "Name")?;
                palette.name = Some(name.trim_matches([' ', '\t']).to_string());
                continue;
            }
            if let Some(columns) = line.strip_prefix("Columns:") {
                palette.header(number, "Columns")?;
                let columns = columns.trim_matches([' ', '\t']);
                let value = decimal(columns)
                    .filter(|&n| n <= MAX_COLUMNS)
                    .ok_or_else(|| {
                        fail(
                            number,
                            format!("`Columns:` takes 0 to {MAX_COLUMNS}, not `{columns}`"),
                        )
                    })?;
                palette.columns = Some(value);
                continue;
            }
            palette.colours.push(colour(number, trimmed)?);
        }
        match palette.colours.len() {
            0 => Err(PaletteError::Empty),
            count if count > MAX_PALETTE_COLOURS => Err(PaletteError::TooMany { count }),
            _ => Ok(palette),
        }
    }

    /// Whether a header line of `key` may come at `line`: before the
    /// colours, and once.
    fn header(&self, line: usize, key: &str) -> Result<(), PaletteError> {
        let seen = match key {
            "Name" => self.name.is_some(),
            _ => self.columns.is_some(),
        };
        let problem = if !self.colours.is_empty() {
            format!("a `{key}:` line comes before the colours")
        } else if seen {
            format!("a second `{key}:` line")
        } else {
            return Ok(());
        };
        Err(PaletteError::Format { line, problem })
    }
}

impl PaletteLut {
    /// Reads a `.gpl` palette ([`GplPalette::parse`]) and bakes its table.
    pub fn from_gpl(text: &str) -> Result<Self, PaletteError> {
        Self::bake(&GplPalette::parse(text)?.colours)
    }
}

/// A run of ASCII digits as a number, if it fits a `u32`.
fn decimal(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// The colour on line `line`: three codes, then anything as its name.
fn colour(line: usize, text: &str) -> Result<Srgb8, PaletteError> {
    let mut words = text.split([' ', '\t']).filter(|w| !w.is_empty());
    let mut code = |channel: &str| {
        let word = words.next();
        word.and_then(decimal)
            .and_then(|n| u8::try_from(n).ok())
            .ok_or_else(|| PaletteError::Format {
                line,
                problem: match word {
                    Some(word) => format!(
                        "a colour's {channel} code is a whole number from 0 to 255, not `{word}`"
                    ),
                    None => format!("a colour gives red, green and blue codes, but no {channel}"),
                },
            })
    };
    Ok(Srgb8::new(code("red")?, code("green")?, code("blue")?))
}
