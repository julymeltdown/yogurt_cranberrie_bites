"""Reproducible, bounded non-character reference-to-vector authoring aid.

The unchanged user-supplied reference is an input only. Output consists of
transparent foreground path geometry, never embedded rasters, controls or characters.
Pillow/numpy/scipy are existing authoring tools; none are browser dependencies.
"""
import argparse
import hashlib
import json
from pathlib import Path

import numpy as np
from PIL import Image
from scipy import ndimage

HERE = Path(__file__).resolve().parent
SOURCE = HERE / 'evidence/target-full.png'
DESTINATION = HERE.parents[1] / 'ui/landing-reference/reference-contours.json'
SOURCE_SHA = 'd9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617'


def paths(mask):
    """Trace directed pixel-cell boundaries, retaining holes and print chips."""
    edges = {}
    for y, x in zip(*np.nonzero(mask)):
        x, y = int(x), int(y)
        for a, b in [((x, y), (x+1, y)), ((x+1, y), (x+1, y+1)),
                     ((x+1, y+1), (x, y+1)), ((x, y+1), (x, y))]:
            if (b, a) in edges:
                del edges[(b, a)]
            else:
                edges[(a, b)] = True
    outgoing = {}
    for a, b in edges:
        outgoing.setdefault(a, set()).add(b)
    contours = []
    while outgoing:
        start = min(outgoing)
        a = start
        points = [a]
        previous = (a[0]-1, a[1])
        while True:
            candidates = outgoing[a]
            # Clockwise boundary: prefer the right turn at diagonally touching cells.
            dx, dy = a[0]-previous[0], a[1]-previous[1]
            priority = [(a[0]-dy, a[1]+dx), (a[0]+dx, a[1]+dy),
                        (a[0]+dy, a[1]-dx), (a[0]-dx, a[1]-dy)]
            b = next((v for v in priority if v in candidates), min(candidates))
            candidates.remove(b)
            if not candidates:
                del outgoing[a]
            previous, a = a, b
            if a == start:
                break
            points.append(a)
        # Remove only exactly collinear vertices. Tiny chips remain source faithful.
        turns = []
        for i, p in enumerate(points):
            before, after = points[i-1], points[(i+1) % len(points)]
            if (p[0]-before[0])*(after[1]-p[1]) != (p[1]-before[1])*(after[0]-p[0]):
                turns.append(p)
        if len(turns) >= 3:
            contours.append('M'+' '.join(f'{x},{y}' for x, y in turns)+'Z')
    return ''.join(contours)


def components(mask, minimum, y_limit=None):
    labels, _ = ndimage.label(mask)
    result = np.zeros_like(mask)
    for i, region in enumerate(ndimage.find_objects(labels)):
        if region and (y_limit is None or region[0].start < y_limit):
            selected = labels[region] == i+1
            if int(selected.sum()) >= minimum:
                result[region] |= selected
    return result


def author():
    raw = SOURCE.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == SOURCE_SHA
    rgb = np.array(Image.open(SOURCE).convert('RGB')).astype(float)
    y, x = np.indices(rgb.shape[:2])
    light = rgb.mean(2)
    glyph_roi = (x >= 40) & (x < 285) & (y >= 53) & (y < 219)
    glyph = components(glyph_roi & (light > 95), 200, 180)
    exclusion = ndimage.binary_dilation(glyph, iterations=1)
    web_roi = (x >= 16) & (x < 139) & (y >= 35) & (y < 151)
    web = components(web_roi & ~exclusion & (light > 90), 2)
    scene_roi = (x >= 17) & (x < 292) & (y >= 162) & (y < 285)
    red = (rgb[:, :, 0] > 65) & (rgb[:, :, 0] > rgb[:, :, 1] * 1.65)
    olive = (rgb[:, :, 1] > 43) & (rgb[:, :, 1] > rgb[:, :, 0] * .84) & (rgb[:, :, 2] < rgb[:, :, 1] * .8)
    scene = scene_roi & ~exclusion & ((light > 70) | red | olive)
    frame_roi = (x >= 6) & (x < 302) & (y >= 33) & (y < 527) & ((x < 15) | (x >= 292) | (y < 38) | (y >= 516))
    frame = frame_roi & (light > 80)
    stars = (x >= 16) & (x < 69) & (y >= 13) & (y < 29) & ((light > 90) | red)
    copy = (x >= 65) & (x < 250) & (y >= 391) & (y < 458) & (light > 95)
    entry = (x >= 38) & (x < 274) & (y >= 467) & (y < 506) & (light > 130)
    assert not np.any((glyph | web | scene | frame | stars | copy | entry)[285:390, 15:292])
    # Explicitly forbid all target characters (first hair begins below this band),
    # text/copy/controls, frame chrome and every other screen.
    assert not np.any((glyph | web | scene)[285:, :])
    assert not np.any((glyph | web | scene)[:, 292:])
    layers = []
    for name, mask in [('wordmark', glyph), ('web', web), ('scene', scene), ('frame', frame), ('stars', stars), ('copy', copy), ('entry', entry)]:
        palette = []
        remainder = mask.copy()
        if name in ['scene', 'stars']:
            for color_mask in [red, olive]:
                for low, high in [(0, 65), (65, 95), (95, 256)]:
                    chosen = remainder & color_mask & (light >= low) & (light < high)
                    if np.any(chosen):
                        color = np.median(rgb[chosen], axis=0).astype(int)
                        palette.append({'fill': '#'+''.join(f'{n:02x}' for n in color), 'd': paths(chosen)})
                        remainder &= ~chosen
        for low, high in [(0, 95), (95, 135), (135, 175), (175, 210), (210, 256)]:
            chosen = remainder & (light >= low) & (light < high)
            if np.any(chosen):
                color = np.median(rgb[chosen], axis=0).astype(int)
                palette.append({'fill': '#'+''.join(f'{n:02x}' for n in color), 'd': paths(chosen)})
        yy, xx = np.nonzero(mask)
        layers.append({'name': name, 'bounds': [int(xx.min()), int(yy.min()), int(xx.max()+1), int(yy.max()+1)], 'pixels': int(mask.sum()), 'paths': palette})
    return {'revision': 1, 'sourceSha256': SOURCE_SHA, 'authoring': 'Isolated foreground contour transcription; no raster/image embedding or character extraction.', 'sourcePanel': [10, 35, 286, 485], 'forbiddenCharacterRegion': [15, 285, 292, 390], 'layers': layers}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    text = json.dumps(author(), separators=(',', ':'))+'\n'
    if args.check:
        assert DESTINATION.read_text() == text, 'Reference contour drift; regenerate the owned foreground paths.'
        print('PASS: deterministic foreground contours, source pin and character exclusion')
    else:
        DESTINATION.write_text(text)
        print(f'Wrote {DESTINATION.name}: {len(text)} bytes')
