"""Writes test_glyphs.ttf and test_glyphs_alt.ttf, the fonts the UI font
tests load.

Made here rather than taken from a real font so that it is ours to ship, a
few hundred bytes rather than megabytes, and shaped for the assertions:

- '한' and '글' are Hangul, which none of egui's built-in fonts has. Without
  this font both draw as the same replacement box; with it they differ.
- 'A' is a glyph the built-in fonts also have, drawn here as a solid block,
  so a frame can tell which font won.
- Nothing else. A character this font lacks ('B') has to come from the
  built-in fonts behind it.

test_glyphs_alt.ttf has only an 'A', as two bars: of two project fonts that
both have 'A', the frame shows which one is first.

Run with fontTools (`python make_test_font.py`) and commit the results.
"""

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen


def rects(*boxes):
    pen = TTGlyphPen(None)
    for x0, y0, x1, y1 in boxes:
        # Clockwise, as TrueType wants outer contours.
        pen.moveTo((x0, y0))
        pen.lineTo((x0, y1))
        pen.lineTo((x1, y1))
        pen.lineTo((x1, y0))
        pen.closePath()
    return pen.glyph()


def save(path, family, glyphs, cmap):
    fb = FontBuilder(unitsPerEm=1000, isTTF=True)
    fb.setupGlyphOrder(list(glyphs))
    fb.setupCharacterMap({ord(c): name for c, name in cmap.items()})
    fb.setupGlyf({name: shape for name, (shape, _) in glyphs.items()})
    fb.setupHorizontalMetrics({name: (adv, 0) for name, (_, adv) in glyphs.items()})
    fb.setupHorizontalHeader(ascent=800, descent=-200)
    fb.setupNameTable({"familyName": family, "styleName": "Regular"})
    fb.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
    fb.setupPost()
    fb.save(path)


save(
    "test_glyphs.ttf",
    "BSEngine Test Glyphs",
    {
        ".notdef": (rects(), 500),
        # A solid block: nothing like the built-in 'A'.
        "A": (rects((50, 0, 550, 700)), 600),
        # Three horizontal bars.
        "uniD55C": (rects((50, 0, 850, 150), (50, 300, 850, 450), (50, 600, 850, 750)), 900),
        # Three vertical bars.
        "uniAE00": (rects((50, 0, 200, 750), (350, 0, 500, 750), (650, 0, 800, 750)), 900),
    },
    {"A": "A", "한": "uniD55C", "글": "uniAE00"},
)
save(
    "test_glyphs_alt.ttf",
    "BSEngine Test Glyphs Alt",
    {
        ".notdef": (rects(), 500),
        # Two vertical bars: neither the block above nor the built-in 'A'.
        "A": (rects((50, 0, 200, 700), (400, 0, 550, 700)), 600),
    },
    {"A": "A"},
)
