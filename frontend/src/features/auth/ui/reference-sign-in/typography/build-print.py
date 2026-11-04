"""Auth-scoped print face: reproducible OFL-1.1 Roboto Condensed derivative.
Use pre-existing FontTools; no package install. No reference pixels/outlines are sampled.
Add original uppercase I terminals, condense width, and roughen long ink edges.
"""
from pathlib import Path
import math
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont
from fontTools import subset
from fontTools.pens.recordingPen import RecordingPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.pens.transformPen import TransformPen

root = Path(__file__).parent
font = instantiateVariableFont(TTFont(root / 'RobotoCondensed-variable.ttf'), {'wght': 800}, inplace=True)
options = subset.Options()
options.recalc_timestamp = False
subsetter = subset.Subsetter(options=options)
subsetter.populate(unicodes=list(range(32, 127)) + [0x2019, 0x2026, 0x2013])
subsetter.subset(font)
# Original I terminals reproduce the target's compact vertical cap/foot treatment.
name_i = font.getBestCmap()[ord('I')]
i_pen = TTGlyphPen(font.getGlyphSet())
outline = [(40,1456),(510,1456),(510,1346),(349,1346),(349,110),(510,110),(510,0),(40,0),(40,110),(165,110),(165,1346),(40,1346)]
i_pen.moveTo(outline[0])
for point in outline[1:]: i_pen.lineTo(point)
i_pen.closePath()
font['glyf'][name_i] = i_pen.glyph()
font['hmtx'][name_i] = (550,40)
for ordinal, name in enumerate(font.getGlyphOrder()):
    recorded = RecordingPen()
    font['glyf'][name].draw(recorded, font['glyf'])
    pen = TTGlyphPen(font.getGlyphSet())
    target = TransformPen(pen, (.86, 0, 0, 1, 0, 0))
    previous = None
    for op, points in recorded.value:
        if op == 'lineTo' and previous is not None:
            end = points[0]
            distance = math.hypot(end[0] - previous[0], end[1] - previous[1])
            steps = max(1, round(distance / 95))
            for step in range(1, steps):
                t = step / steps
                # Deterministic slight ink-edge chips; no damaged counters or lost terminals.
                jitter = math.sin(ordinal * 19 + step * 23 + previous[0] * .07) * 10
                target.lineTo((previous[0] + (end[0]-previous[0])*t + jitter,
                               previous[1] + (end[1]-previous[1])*t + jitter*.5))
        getattr(target, op)(*points)
        if op in ('moveTo', 'lineTo', 'qCurveTo', 'curveTo'):
            previous = points[-1]
        elif op in ('closePath', 'endPath'):
            previous = None
    font['glyf'][name] = pen.glyph()
    advance, lsb = font['hmtx'][name]
    font['hmtx'][name] = (round(advance * .86), round(lsb * .86))
for record in font['name'].names:
    replacements = {1:'MCR Auth Print',2:'Bold',3:'MCRAuthPrint-1.0',4:'MCR Auth Print Bold',6:'MCRAuthPrint-Bold',16:'MCR Auth Print',17:'Bold'}
    if record.nameID in replacements:
        record.string = replacements[record.nameID].encode(record.getEncoding())
font['head'].created = 2082844800
font['head'].modified = 2082844800
font.recalcTimestamp = False
font.flavor = 'woff'
font.save(root / 'mcr-auth-print.woff')
print('Auth print WOFF written')
