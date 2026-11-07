//! The plain-text palette reader (rung 2, task T10; plan decision 8):
//! GIMP's `.gpl` format, read strictly, every refusal naming its line. No
//! GPU is needed.

use eng_render::{GplPalette, PaletteError, PaletteLut, Srgb8};

const ROOM: &str = "GIMP Palette\n\
Name: Room\n\
Columns: 4\n\
# Made by hand for the test.\n\
\n\
  0   0   0\tBlack\n\
255 255 255\tWhite\n\
 12  34 200 a blue with spaces in its name\n\
7 8 9\n";

fn refusal(text: &str) -> String {
    GplPalette::parse(text)
        .expect_err("the palette should be refused")
        .to_string()
}

#[test]
fn a_palette_reads_its_name_columns_and_colours_in_order() {
    let palette = GplPalette::parse(ROOM).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(palette.name.as_deref(), Some("Room"));
    assert_eq!(palette.columns, Some(4));
    assert_eq!(
        palette.colours,
        [
            Srgb8::new(0, 0, 0),
            Srgb8::new(255, 255, 255),
            Srgb8::new(12, 34, 200),
            Srgb8::new(7, 8, 9),
        ]
    );
    // Windows line ends, no header, no final newline: the same colours.
    let bare = "GIMP Palette\r\n0 0 0\r\n255 255 255\r\n12\t34\t200\r\n7 8 9";
    let bare = GplPalette::parse(bare).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((bare.name, bare.columns), (None, None));
    assert_eq!(bare.colours, palette.colours);
    // Columns before the name, and comments between colours.
    let swapped = "GIMP Palette\nColumns: 0\nName:\n1 2 3\n# no colour\n4 5 6\n";
    let swapped = GplPalette::parse(swapped).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        (swapped.name.as_deref(), swapped.columns),
        (Some(""), Some(0))
    );
    assert_eq!(swapped.colours.len(), 2);
}

#[test]
fn a_table_bakes_from_a_palette_file() {
    let lut = PaletteLut::from_gpl(ROOM).unwrap_or_else(|e| panic!("{e}"));
    let colours = GplPalette::parse(ROOM)
        .unwrap_or_else(|e| panic!("{e}"))
        .colours;
    assert_eq!(
        lut,
        PaletteLut::bake(&colours).unwrap_or_else(|e| panic!("{e}"))
    );
    assert_eq!(lut.lookup(Srgb8::new(250, 250, 250)), 1);
    assert_eq!(
        PaletteLut::from_gpl("GIMP Palette\n# nothing\n"),
        Err(PaletteError::Empty)
    );
}

#[test]
fn anything_off_the_format_is_refused_with_its_line() {
    assert_eq!(
        refusal("GIMP palette\n0 0 0\n"),
        "[E-PALETTE] line 1: a GIMP palette starts with the line `GIMP Palette`"
    );
    assert_eq!(
        refusal("\u{feff}GIMP Palette\n0 0 0\n"),
        "[E-PALETTE] line 1: a GIMP palette starts with the line `GIMP Palette`"
    );
    assert_eq!(
        refusal(""),
        "[E-PALETTE] line 1: a GIMP palette starts with the line `GIMP Palette`"
    );
    assert_eq!(
        refusal("GIMP Palette\n0 0 256\n"),
        "[E-PALETTE] line 2: a colour's blue code is a whole number from 0 to 255, not `256`"
    );
    assert_eq!(
        refusal("GIMP Palette\n0 -1 2\n"),
        "[E-PALETTE] line 2: a colour's green code is a whole number from 0 to 255, not `-1`"
    );
    assert_eq!(
        refusal("GIMP Palette\n\n0x10 0 0\n"),
        "[E-PALETTE] line 3: a colour's red code is a whole number from 0 to 255, not `0x10`"
    );
    assert_eq!(
        refusal("GIMP Palette\n1 2\n"),
        "[E-PALETTE] line 2: a colour gives red, green and blue codes, but no blue"
    );
    assert_eq!(
        refusal("GIMP Palette\n1.5 2 3\n"),
        "[E-PALETTE] line 2: a colour's red code is a whole number from 0 to 255, not `1.5`"
    );
    assert_eq!(
        refusal("GIMP Palette\n1 2 3\nName: late\n"),
        "[E-PALETTE] line 3: a `Name:` line comes before the colours"
    );
    assert_eq!(
        refusal("GIMP Palette\nColumns: 2\nColumns: 3\n1 2 3\n"),
        "[E-PALETTE] line 3: a second `Columns:` line"
    );
    assert_eq!(
        refusal("GIMP Palette\nColumns: 257\n1 2 3\n"),
        "[E-PALETTE] line 2: `Columns:` takes 0 to 256, not `257`"
    );
    assert_eq!(
        refusal("GIMP Palette\nColumns: four\n1 2 3\n"),
        "[E-PALETTE] line 2: `Columns:` takes 0 to 256, not `four`"
    );
    assert_eq!(
        refusal("GIMP Palette\nAuthor: me\n1 2 3\n"),
        "[E-PALETTE] line 2: a colour's red code is a whole number from 0 to 255, not `Author:`"
    );
    assert_eq!(
        refusal("GIMP Palette\n  # an indented comment is no comment\n"),
        "[E-PALETTE] line 2: a colour's red code is a whole number from 0 to 255, not `#`"
    );
    assert_eq!(
        refusal("GIMP Palette\nName: none\n"),
        "[E-PALETTE] the palette lists no colour"
    );
    let many: String = std::iter::once("GIMP Palette\n".to_string())
        .chain((0..257).map(|i| format!("{} 0 0\n", i % 256)))
        .collect();
    assert_eq!(
        GplPalette::parse(&many),
        Err(PaletteError::TooMany { count: 257 })
    );
    let most: String = std::iter::once("GIMP Palette\n".to_string())
        .chain((0..256).map(|i| format!("{i} 0 0\n")))
        .collect();
    assert_eq!(GplPalette::parse(&most).map(|p| p.colours.len()), Ok(256));
}
