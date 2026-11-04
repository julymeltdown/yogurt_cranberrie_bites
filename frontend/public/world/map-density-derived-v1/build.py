"""A bounded new composition using immutable ART06 geometry and original crops."""
from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import io
import json
import math
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
BASE = ROOT / 'frontend/public/world/map-reference-composition-v1'
SOURCE = 'af298e3246096c4b7a3ae5c7e439f5f33988a6be'
CANONICAL_BASE = '609ae3932f22e3689637eb43074b6236ccb6c64f'
PRESENTATION = 'sha256:d8437dda63dd5f0cb554bcfcc1c267782db6630fab39c541291bb3c368d154b6'
VERSION = 'map-density-derived-v1'
REVISION = 'lawn-shoulders-and-junction-banks-01'
ERAS = ('REVENGE', 'PARADE', 'DANGER')
SIZE = (768, 1296)
PATH_HALF_WIDTH = 25  # Existing outer23px plus2px independent clearance.
GARDEN_HALF_WIDTH = 13  # Existing border11px plus2px independent clearance.
CANONICAL_CLEARANCE = 34
ALLOWED = ('tree_green', 'tree_dark', 'tree_yellow', 'shrub_bed')
FROZEN = {
    'README.md': 'dd5915ec730d780b20f9d164328760c39520dfdd57b5c2c0ee26c32bf1d9060c',
    'build.py': '6bf696b7c3edd5adf35e0b5995ca9cd33f915e87898de0e9266f2923cffc0433',
    'layout.json': '77da03bcddba15cb88a250a3cad4dbe9bdfadac519af1f1d4ac22623a8aa6141',
    'source-provenance.json': '0b9c5663e1fa182aae4012be465c448832ff798f926e930ee33c08cd456c084e',
    'revenge-scene.png': '0b6da66ac646ec28469c29d309f62e725ba24d48eee9b4b24e0c0383aedd3083',
    'parade-scene.png': '1d7cc73a1cdaf0931d446206a5424dd8f131c5c819e1f147a0a150583e98b493',
    'danger-scene.png': '8846875dabb8a48df8b8d8d5433eab5f682e215c02ac50a0fb6dcc2d7da54edf',
    'revenge-scene.svg': 'd3b6bb973996848be191579f3a8f018efda8cea4f8fe428c99f48f1721b1e0ed',
    'parade-scene.svg': '2d75bd6c760e140abc39da9bf6e0079edede28aeb0935ad8bc52bfffee92855c',
    'danger-scene.svg': '27ab6aa807bc6c63a6bff3c82d6d70189f20d3f93b7a09a536cf92c044e394a1',
}
# Authored root-space ellipses follow the actual bare shoulders, not a row grid.
# Existing artwork/clearances decide acceptance; candidate counts are not coverage.
SHOULDERS = [
    ('stage_west', 190, 235, 48, 48, 18),
    ('stage_north', 336, 120, 76, 22, 14),
    ('stage_east', 600, 235, 48, 58, 18),
    ('plaza_upper', 380, 425, 80, 18, 20),
    ('plaza_west', 223, 470, 38, 70, 22),
    ('plaza_lower', 329, 588, 68, 20, 20),
    ('plaza_east', 494, 513, 49, 71, 26),
    ('west_court', 115, 590, 54, 32, 18),
    ('west_junction', 248, 620, 58, 26, 20),
    ('east_court', 556, 603, 58, 38, 20),
    ('junction_north', 388, 637, 34, 41, 18),
    ('junction_south', 352, 748, 54, 31, 22),
    ('junction_east', 605, 781, 62, 32, 20),
    ('meet_west', 212, 823, 42, 45, 20),
    ('meet_north', 403, 850, 86, 24, 26),
    ('meet_east', 548, 872, 37, 52, 20),
    ('meet_lower', 396, 1045, 84, 29, 26),
    ('south_west_walk', 203, 1032, 47, 39, 20),
    ('south_west_lawn', 227, 1169, 57, 30, 20),
    ('south_central', 390, 1240, 69, 38, 22),
    ('south_east', 628, 1266, 64, 25, 20),
]


def sha(raw):
    return 'sha256:' + hashlib.sha256(raw).hexdigest()


def encode(value):
    return (json.dumps(value, indent=2, ensure_ascii=True) + '\n').encode()


def descriptor(path, raw):
    return {'path': path, 'bytes': len(raw), 'sha256': sha(raw)}


def overlap(a, b):
    return a[0] < b[0]+b[2] and b[0] < a[0]+a[2] and a[1] < b[1]+b[3] and b[1] < a[1]+a[3]


def segment_hits_rect(a, b, rect, padding):
    """Slab intersection against the complete rectangle, expanded by paint width."""
    low, high = 0.0, 1.0
    for index in (0, 1):
        start, delta = a[index], b[index]-a[index]
        minimum, maximum = rect[index]-padding, rect[index]+rect[index+2]+padding
        if delta == 0:
            if start < minimum or start > maximum:
                return False
        else:
            t0, t1 = (minimum-start)/delta, (maximum-start)/delta
            low, high = max(low, min(t0, t1)), min(high, max(t0, t1))
            if low > high:
                return False
    return True


def protected(layout):
    return [[r['exclusionRect'][k] for k in ('x', 'y', 'width', 'height')]
            for r in layout['serviceLabelAnchors']]


def safe(rect, base):
    x, y, w, h = rect
    if not (0 <= x < x+w <= SIZE[0] and 0 <= y < y+h <= SIZE[1]):
        return False
    if any(overlap(rect, other) for other in protected(base)):
        return False
    center, radius = (x+w/2, y+h/2), math.hypot(w/2, h/2)
    if any(math.dist(center, (p['x'], p['y'])) < radius+CANONICAL_CLEARANCE
           for p in base['canonicalDisplayAnchors']):
        return False
    if any(segment_hits_rect(a, b, rect, PATH_HALF_WIDTH)
           for path in base['paths'] for a, b in zip(path, path[1:])):
        return False
    if any(segment_hits_rect(a, b, rect, GARDEN_HALF_WIDTH)
           for garden in base['gardenParcels'] for a, b in zip(garden, garden[1:]+garden[:1])):
        return False
    for previous in base['decorations']:
        if not overlap(rect, previous['rect']):
            continue
        # Existing trees can interleave in global foot order. Architecture,
        # furniture and all other incumbent non-tree objects remain unobscured.
        if not previous['frame'].startswith('tree_'):
            return False
    return True


def additions(base):
    rows, roots = [], []
    for group, (name, cx, cy, rx, ry, count) in enumerate(SHOULDERS):
        samples = count*2
        for index in range(samples):
            seed = group*101+index*47
            angle = index*math.pi*(3-math.sqrt(5))+group*0.63
            radius = math.sqrt((index+0.5)/samples)
            root = (round(cx+rx*radius*math.cos(angle)), round(cy+ry*radius*math.sin(angle)))
            frame = 'shrub_bed' if seed % 4 == 0 else ('tree_yellow' if seed % 11 == 0 else 'tree_dark' if seed % 3 == 0 else 'tree_green')
            width = 29+seed % 17 if frame == 'shrub_bed' else 36+seed % 20
            height = math.ceil(width*(0.65 if frame == 'shrub_bed' else 0.84))
            rect = [root[0]-width//2, root[1]-height, width, height]
            if not safe(rect, base) or any(math.dist(root, old) < 17 for old in roots):
                continue
            roots.append(root)
            rows.append({'id': f'density_{name}_{index:02d}', 'frame': frame, 'rect': rect,
                         'depth_y': root[1], 'interactive': False, 'place_id': None,
                         'interaction_id': None, 'world_solid': False, 'crop_role': 'whole',
                         'density_group': name})
    return sorted(rows, key=lambda row: (row['depth_y'], row['id']))


def inputs():
    frozen, pins = {}, []
    for name, expected in FROZEN.items():
        raw = (BASE/name).read_bytes()
        assert sha(raw) == 'sha256:'+expected, name
        frozen[name] = raw
        pins.append(descriptor(str((BASE/name).relative_to(ROOT)), raw))
    provenance = json.loads(frozen['source-provenance.json'])
    for pin in provenance['sourceInputs']:
        raw = (ROOT/pin['path']).read_bytes()
        assert descriptor(pin['path'], raw) == pin, pin['path']
    return frozen, pins, provenance


def produce():
    frozen, baseline_pins, old_provenance = inputs()
    base = json.loads(frozen['layout.json'])
    new = additions(base)
    counts = {group[0]: sum(row['density_group'] == group[0] for row in new) for group in SHOULDERS}
    assert len(new) >= 12, f'Only {len(new)} additions survive: {counts}'
    assert len({row['density_group'] for row in new}) >= 6
    assert any('junction' in row['density_group'] for row in new), 'Junction bank must be visible in the actual output'
    original_crops = {(p['themeId'], p['frame']): p for p in old_provenance['originalCropPins']}
    # Reuse only the SHA-pinned pure ground/roof helpers, never produce()/main().
    # The old builder and its files are read-only. New and existing crops render
    # once in global foot order, allowing genuine layered foliage rather than
    # rejecting every useful position behind an existing tree.
    spec = importlib.util.spec_from_file_location('immutable_art06_helpers', BASE/'build.py')
    assert spec and spec.loader
    parent = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(parent)
    ordered = sorted(base['decorations']+new, key=lambda row: (row['depth_y'], row['id']))
    new_ids = {row['id'] for row in new}
    crops, deltas, rendered, outputs, assets = [], {}, {}, {}, []
    for era in ERAS:
        name = era.lower()
        with Image.open(io.BytesIO(frozen[name+'-scene.png'])) as old:
            assert old.size == SIZE and old.mode == 'RGB'
            before = old.copy()
        canvas, _, substrate, _ = parent.surface(era)
        assert substrate == 0
        atlas_path = f'frontend/public/world/reference-scene-v2/map-cozy-v1/{name}/atlas.png'
        with Image.open(ROOT/atlas_path) as atlas:
            assert atlas.size == (1024, 1024) and atlas.mode == 'RGBA'
            vectors, cache, boxes = [], {}, []
            for row in ordered:
                frame = row['frame']
                proof = original_crops[(era, frame)]
                f = proof['sourceRect']
                sx, sy, sw, sh = [f[k] for k in ('x', 'y', 'width', 'height')]
                if frame not in cache:
                    crop = atlas.crop((sx, sy, sx+sw, sy+sh))
                    assert sha(crop.tobytes()) == proof['rgbaSha256']
                    cache[frame] = crop
                    if frame in ALLOWED:
                        crops.append({'themeId': era, 'frame': frame, 'sourceRect': f,
                                      'rgbaSha256': proof['rgbaSha256'], 'atlas': proof['atlas'],
                                      'alphaPolicy': 'EXACT_WHOLE_SOURCE_RGBA_NO_RETOUCH'})
                x, y, w, h = row['rect']
                source_end = math.floor(sh*parent.CANOPY_SOURCE_END) if row['crop_role'] == 'canopy' else sh
                scale = min(w/sw, h/source_end)
                rw, rh = max(1, math.floor(sw*scale+0.5)), max(1, math.floor(source_end*scale+0.5))
                dx, dy = x+(w-rw)//2, y+h-rh
                trim_source = math.floor(sh*parent.FACADE_SOURCE_START) if frame in parent.VENUES else 0
                facade_height = max(1, math.floor(rh*(1-parent.FACADE_SOURCE_START)*parent.FACADE_HEIGHT_SCALE+0.5)) if frame in parent.VENUES else rh
                trim_dest = rh-facade_height
                selected = cache[frame].crop((0, trim_source, sw, source_end))
                canvas.alpha_composite(selected.resize((rw, rh-trim_dest), Image.Resampling.NEAREST), (dx, dy+trim_dest))
                vectors.append(f'<svg id="{row["id"]}" x="{dx}" y="{dy+trim_dest}" width="{rw}" height="{rh-trim_dest}" viewBox="{sx} {sy+trim_source} {sw} {source_end-trim_source}" preserveAspectRatio="none"><use xlink:href="#atlas"/></svg>')
                if frame in parent.VENUES:
                    parent.overhead_roof(canvas, vectors, row['id'], dx, dy, rw, rh, parent.PALETTES[era])
                if row['id'] in new_ids:
                    boxes.append({'id': row['id'], 'rect': [dx, dy, rw, rh]})
        final = canvas.convert('RGB')
        for x, y, w, h in protected(base):
            assert final.crop((x, y, x+w, y+h)).tobytes() == before.crop((x, y, x+w, y+h)).tobytes()
        # Compare channels explicitly: grayscale conversion cannot hide a small single-channel delta.
        channels = ImageChops.difference(final, before).split()
        changed_mask = ImageChops.lighter(ImageChops.lighter(channels[0], channels[1]), channels[2]).point(lambda value: 255 if value else 0)
        allowed = Image.new('L', SIZE)
        draw = ImageDraw.Draw(allowed)
        for row in new:
            x, y, w, h = row['rect']
            draw.rectangle((x, y, x+w-1, y+h-1), fill=255)
        assert ImageChops.subtract(changed_mask, allowed).getbbox() is None
        deltas[era] = changed_mask.histogram()[255]
        assert deltas[era] > 5000, 'Accepted geometry must produce an actual visible increment'
        rendered[era] = boxes
        buffer = io.BytesIO()
        final.save(buffer, format='PNG', compress_level=9)
        png = buffer.getvalue()
        assert len(png) <= 2097152
        outputs[name+'-scene.png'] = png
        # Reuse the exact immutable ground/atlas; interleave named crop layers.
        svg = frozen[name+'-scene.svg'].decode()
        start = svg.index('<g id="editable_noninteractive_scenery"')
        start = svg.index('>', start)+1
        outputs[name+'-scene.svg'] = (svg[:start]+''.join(vectors)+'</g></svg>\n').encode()
        assets.append({'themeId': era, **descriptor('world/'+VERSION+'/'+name+'-scene.png', png),
                       'width': SIZE[0], 'height': SIZE[1], 'mime': 'image/png'})
    layout = copy.deepcopy(base)
    layout.update(version=VERSION, recipeRevision=REVISION, assets=assets,
                  decorations=ordered)
    layout['derivation'] = {'sourceCommit': SOURCE, 'canonicalBase': CANONICAL_BASE,
                            'previousPresentationHash': PRESENTATION, 'baselinePins': baseline_pins,
                            'addedDecorationIds': [row['id'] for row in new],
                            'pathHalfWidthWithClearancePx': PATH_HALF_WIDTH,
                            'gardenHalfWidthWithClearancePx': GARDEN_HALF_WIDTH,
                            'canonicalClearancePx': CANONICAL_CLEARANCE,
                            'architecturePolicy': 'UNCHANGED_NO_OVERPAINT',
                            'foliagePolicy': 'WHOLE_ORIGINAL_RGBA_UNIFORM_NEAREST_GLOBAL_FOOT_ORDER'}
    outputs['layout.json'] = encode(layout)
    provenance = {'format': 'MCR_MAP_DENSITY_DERIVED_V1', 'version': VERSION,
                  'status': 'SOURCE_VALIDATED_NATIVE_NOT_RUN', 'recipeRevision': REVISION,
                  'sourceCommit': SOURCE, 'canonicalBase': CANONICAL_BASE,
                  'previousPresentationHash': PRESENTATION, 'baselinePins': baseline_pins,
                  'sourceInputs': old_provenance['sourceInputs'],
                  'originalCropPins': old_provenance['originalCropPins'], 'additionCropPins': crops,
                  'recipe': descriptor(str((HERE/'build.py').relative_to(ROOT)), (HERE/'build.py').read_bytes()),
                  'outputs': [descriptor(str((HERE/name).relative_to(ROOT)), raw) for name, raw in outputs.items()],
                  'shoulderDomains': [{**dict(zip(('id', 'x', 'y', 'radiusX', 'radiusY', 'candidates'), group)),
                                      'candidates': group[-1]*2} for group in SHOULDERS],
                  'acceptedByGroup': counts,
                  'addedDecorations': len(new), 'actualRenderedRectsByTheme': rendered,
                  'changedRasterPixelsByTheme': deltas, 'protectedLabelRectanglesExact': 18,
                  'newSubstratePaintedPixelsByTheme': {era: 0 for era in ERAS},
                  'policy': 'GLOBAL_FOOT_ORDER_ORIGINAL_FOLIAGE_NO_PATH_GARDEN_ARCHITECTURE_IDENTITY_OR_PALETTE_CHANGE',
                  'helperReuse': {'path': str((BASE/'build.py').relative_to(ROOT)), 'sha256': 'sha256:'+FROZEN['build.py'],
                                  'called': ['surface', 'overhead_roof'], 'notCalled': ['produce', 'main'],
                                  'scope': 'New derivative raster only; no old output or recipe write'},
                  'referenceUse': 'ACTUAL_ART06_NATIVE02_AND_FOURTH_TARGET_VISUAL_INSPECTION_NO_REFERENCE_SAMPLING',
                  'notRun': ['actual consumer registration/App uptake', 'runtime marker/framing changes',
                             'SVG engine parity', 'final full reference acceptance', 'device/live/physical venue']}
    outputs['source-provenance.json'] = encode(provenance)
    assert sum(map(len, outputs.values())) < 10*1024*1024
    return outputs, new, deltas


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--verify', action='store_true')
    args = parser.parse_args()
    outputs, rows, deltas = produce()
    expected = set(outputs) | {'build.py', 'README.md'}
    assert not ({p.name for p in HERE.iterdir()}-expected)
    if args.verify:
        assert {p.name for p in HERE.iterdir()} == expected
        for name, raw in outputs.items():
            assert (HERE/name).read_bytes() == raw, name
    else:
        for name, raw in outputs.items():
            (HERE/name).write_bytes(raw)
    print(json.dumps({'status': 'PASS', 'mode': 'verify' if args.verify else 'build',
                      'outputs': len(outputs), 'bytes': sum(map(len, outputs.values())),
                      'addedDecorations': len(rows), 'changedRasterPixelsByTheme': deltas,
                      'outputPins': [descriptor(name, raw) for name, raw in outputs.items()]}))


if __name__ == '__main__':
    main()
