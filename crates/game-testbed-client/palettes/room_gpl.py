#!/usr/bin/env python3
"""Writes the testbed room's palette, room.gpl (rung 2 plan, decision 8).

    python3 room_gpl.py > room.gpl        # write it
    python3 room_gpl.py --check room.gpl  # exit 1 if the file differs

The palette is 256 colours in GIMP's plain-text .gpl format, which
eng-render reads (GplPalette) and bakes into the palette look's table.
Black first, then ramps of shades, each from near black through its base
colour (two thirds of the way along) to a pale tint: neutral greys, then
one ramp for each of the room's materials, things, sky and fog. The greys'
and the materials' shades sit 7 to 15 codes apart (median 8 to 11), so the
look's 4x4 dither (eng-render's DITHER_CODES, 8 codes) moves pixels between
neighbouring shades there; the 16-shade ramps of the things, the sky and
the fog are coarser (13 to 32 codes), and the dither shows less on them.

It is not Doom's palette, which is id Software's work: spec 11 asks for
public tropes, not protected works. The arithmetic is integers only, so
every Python 3 writes the same bytes. The provenance sidecar is
room.gpl.provenance.toml.
"""

import sys

# (name, shades, dark end, base, light end), in the order they are written.
RAMPS = [
    ("grey", 32, (7, 7, 7), (167, 167, 167), (255, 255, 255)),
    ("stone", 28, (9, 8, 7), (150, 141, 126), (236, 230, 218)),
    ("brick", 24, (12, 6, 4), (146, 82, 62), (232, 176, 156)),
    ("ceiling", 20, (6, 7, 9), (118, 122, 132), (206, 210, 222)),
    ("tread", 20, (6, 9, 7), (104, 116, 108), (196, 210, 200)),
    ("wood", 24, (11, 7, 3), (170, 124, 72), (242, 214, 166)),
    ("red", 16, (18, 3, 2), (214, 44, 40), (255, 176, 164)),
    ("green", 16, (3, 16, 5), (52, 186, 64), (194, 255, 196)),
    ("blue", 16, (3, 5, 20), (46, 76, 222), (176, 194, 255)),
    ("yellow", 16, (17, 13, 2), (236, 200, 44), (255, 246, 184)),
    ("sky", 16, (5, 9, 19), (98, 140, 206), (214, 230, 255)),
    ("fog", 16, (2, 9, 4), (56, 140, 80), (182, 232, 194)),
    ("rust", 11, (16, 7, 2), (200, 112, 42), (255, 204, 146)),
]

COUNT = 256


def round_div(numerator, denominator):
    """numerator / denominator, rounded half up; both whole, denominator > 0."""
    return (2 * numerator + denominator) // (2 * denominator)


def ramp(shades, dark, base, light):
    """The ramp's colours: dark at 0, base at two thirds, light at the end."""
    last = shades - 1
    colours = []
    for i in range(shades):
        # The position is i / last; two thirds of the way is the base.
        a, b = 3 * i, 2 * last
        if a <= b:
            colour = tuple(d + round_div((m - d) * a, b) for d, m in zip(dark, base))
        else:
            colour = tuple(m + round_div((l - m) * (a - b), last) for m, l in zip(base, light))
        colours.append(colour)
    return colours


def palette():
    """Every colour with its name, black first."""
    named = [((0, 0, 0), "black")]
    for name, shades, dark, base, light in RAMPS:
        named += [(c, f"{name} {i}") for i, c in enumerate(ramp(shades, dark, base, light))]
    colours = [c for c, _ in named]
    assert len(colours) == COUNT, len(colours)
    assert len(set(colours)) == COUNT, "a colour appears twice"
    assert all(0 <= v <= 255 for c in colours for v in c)
    return named


def text():
    lines = [
        "GIMP Palette",
        "Name: Testbed room",
        "Columns: 16",
        "# Made by crates/game-testbed-client/palettes/room_gpl.py; do not edit by hand.",
        "# Black, then ramps from near black through a base colour to a pale tint:",
        "# " + ", ".join(f"{name} {shades}" for name, shades, *_ in RAMPS) + ".",
    ]
    for (r, g, b), name in palette():
        lines.append(f"{r:3d} {g:3d} {b:3d}\t{name}")
    return "\n".join(lines) + "\n"


def main(args):
    out = text()
    if args[:1] == ["--check"] and len(args) == 2:
        with open(args[1], encoding="utf-8", newline="") as f:
            same = f.read() == out
        print("room.gpl is up to date" if same else "room.gpl differs from room_gpl.py's")
        return 0 if same else 1
    if args:
        print(__doc__, file=sys.stderr)
        return 64
    sys.stdout.write(out)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
