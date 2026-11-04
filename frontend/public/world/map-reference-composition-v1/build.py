"""Bounded, deterministic Map-only scene composition. No subprocesses or caches."""
from __future__ import annotations

import argparse
import base64
import hashlib
import io
import json
import math
from pathlib import Path

from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
BASE = 'frontend/public/world/reference-scene-v2/'
VERSION = 'map4-reference-composition-v1'
RECIPE_REVISION = 'reference-correction-04'
# Keep the consumer's layout identity/schema; a future intake must pin new bytes.
PREVIOUS_HANDOFF = '94c47a8b3219efc76a4be9646014cfeaad2f1957'
FACADE_SOURCE_START = 0.54
FACADE_HEIGHT_SCALE = 0.80
ROOF_EAVE = 0.65
PATH_WIDTHS = (46, 40, 28)
GARDEN_BORDER = 22
PATH_PLANT_CLEARANCE = 60
GARDEN_PLANT_CLEARANCE = 30
CANOPY_SOURCE_END = 1.0
SIZE = (768, 1296)
PINS = {
    BASE + 'map-cozy-v1/parade/atlas.png': '9c77e42eb377fbab1df485d6c1e8e47221ac33aebbc03e9aecba3cc7bbcded9f',
    BASE + 'map-cozy-v1/revenge/atlas.png': '54b5a72a1e16062f84248602f8bebaee500ca086fb58285b1689d665618dfdbd',
    BASE + 'map-cozy-v1/danger/atlas.png': '3184e26c5b83cf95c31f692b3087112faa93a0ef270bd5416e36fc9952d9a17a',
    BASE + 'map-cozy-v1/era-derivatives.json': '4cb98c6ba28535427b4c3ed8e122e317a0f0cd1e278d2b9cb56aaf52836b5fa0',
    BASE + 'map-cozy-v1/source-registration.json': 'f38f19148b8d9ee80dc7340de2ae1fa4388583977aeff1c0602514b006792121',
    BASE + 'map-cozy-v1/presentation.json': 'b3c08ea713e7413539c976a49c5f1267bee3345f8cc2d1dafb99fede8e733a0a',
    'frontend/src/world/reference-scene/spatial-v3/source-inputs.json': '847d9c92a4ddfa22b8187bb6afe9a9fc2e6e09b1c7fe8e5f85c5a7f5c0d846a9',
    'frontend/src/app/world-entry/__tests__/reference-scene-root/source-inputs/source-binding.json': '0f15f5614d5502442a4e58cc995041f7ee1f2f906e21d4b630db7b13af65ee6b',
    'docs/mcr/design/reference-target-2026-11-04.png': 'd9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617',
}
THEMES = {
    'REVENGE': '2c456a8cb4b601a66c07577fa105136e11b1d4e4d9c401eb7e3181fed706e04f',
    'PARADE': '334011156771af9cfa199671f4977c517921bb877928245f501ecde3a55f163b',
    'DANGER': 'fdf92937e87445ca82826da062d7d21e673dd9e6d463a36e9d95b7acce55546a',
}
ANCHORS = [('main_stage', 410, 195, 312), ('plaza', 329, 502, 143),
           ('wall', 160, 709, 221), ('feed', 641, 692, 192),
           ('meet', 376, 933, 240), ('gifts', 515, 1160, 240)]
CANONICAL = [('stage_info', 410, 403, 30, 24), ('park_info', 646, 428, 38, 27),
             ('gifts_info', 523, 1230, 13, 48), ('wall_info', 143, 794, 14, 61)]
PATHS = [
    [(-60, 420), (305, 42), (540, -45)],
    [(-55, 1030), (325, 667), (820, 245)],
    [(-50, 425), (248, 697), (820, 1305)],
    [(240, -50), (514, 215), (810, 420)],
    [(-30, 1190), (241, 1006), (435, 796), (818, 867)],
    [(190, 267), (352, 355), (566, 353), (783, 150)],
    [(44, 1299), (194, 1139), (422, 1100), (780, 1100)],
]
GARDENS = [
    [(260, 390), (406, 405), (484, 494), (395, 608), (217, 574), (188, 462)],
    [(292, 778), (469, 806), (548, 929), (433, 1035), (274, 977), (235, 873)],
]
# Authored planting footprints, not measured reference pixels or World geometry.
# Broad interlocking masses replace the former equally spaced bed perimeter.
PLANTING = [
    [(0, 106), (94, 110), (146, 174), (128, 280), (35, 310), (0, 256)],
    [(173, 70), (248, 91), (309, 115), (282, 193), (184, 218), (143, 160)],
    [(455, 74), (573, 70), (650, 133), (651, 256), (555, 253), (521, 167)],
    [(625, 383), (768, 390), (768, 545), (685, 573), (586, 510), (570, 442)],
    [(0, 400), (114, 387), (177, 448), (162, 566), (50, 592), (0, 536)],
    [(253, 402), (369, 416), (440, 452), (451, 526), (382, 585), (239, 558), (204, 468)],
    [(92, 580), (183, 594), (243, 635), (212, 682), (125, 659), (62, 622)],
    [(441, 575), (541, 563), (621, 600), (613, 729), (522, 776), (426, 731), (392, 640)],
    [(658, 741), (768, 730), (768, 863), (657, 894), (586, 851), (605, 795)],
    [(0, 789), (100, 783), (198, 837), (213, 955), (147, 1002), (32, 947), (0, 887)],
    [(295, 795), (398, 817), (468, 859), (494, 949), (414, 1014), (287, 972), (252, 869)],
    [(541, 870), (628, 901), (703, 942), (714, 1035), (631, 1066), (548, 1019), (526, 958)],
    [(180, 1008), (274, 1036), (333, 1081), (292, 1150), (201, 1182), (146, 1110)],
    [(310, 1154), (411, 1139), (495, 1205), (502, 1296), (339, 1296), (278, 1248)],
    [(606, 1157), (713, 1167), (768, 1225), (768, 1296), (582, 1296), (563, 1224)],
]
PALETTES = {
    'REVENGE': {'lawn': '#b5c4a5', 'lawnLight': '#becbad', 'lawnDark': '#adbc9e',
                'path': '#f1d9d0', 'pathEdge': '#bda08f', 'pathMiddle': '#ead0c2',
                'garden': '#bccb9f', 'ring': '#e7c2af', 'seam': '#d7b6a5',
                'roof': '#cf999a', 'roofLight': '#efc7bc', 'roofDark': '#9f747e'},
    'PARADE': {'lawn': '#abc49c', 'lawnLight': '#b3cba3', 'lawnDark': '#a2bd95',
               'path': '#f4e8d6', 'pathEdge': '#ab9c83', 'pathMiddle': '#efe0c9',
               'garden': '#b4ca95', 'ring': '#ead8bc', 'seam': '#e4d1b7',
               'roof': '#b7a8c3', 'roofLight': '#dfcfde', 'roofDark': '#8d819d'},
    'DANGER': {'lawn': '#aec8b7', 'lawnLight': '#b7d1bd', 'lawnDark': '#a4bfad',
               'path': '#f3e6bb', 'pathEdge': '#ac9c76', 'pathMiddle': '#e9dbad',
               'garden': '#c3ce9d', 'ring': '#dfd29f', 'seam': '#d9c98e',
               'roof': '#9db5a8', 'roofLight': '#d5e0c9', 'roofDark': '#7a968b'},
}
VENUES = {'meeting_pavilion', 'giveaway_pavilion', 'north_hall', 'west_hall'}
TREES = {'tree_green', 'tree_dark', 'tree_yellow', 'tree_red'}


def digest(data):
    return 'sha256:' + hashlib.sha256(data).hexdigest()


def encode(value):
    return (json.dumps(value, indent=2, ensure_ascii=True) + '\n').encode()


def descriptor(path, data):
    return {'path': path, 'bytes': len(data), 'sha256': digest(data)}


def intersects(a, b):
    return a[0] < b[0] + b[2] and b[0] < a[0] + a[2] and a[1] < b[1] + b[3] and b[1] < a[1] + a[3]


def exclusions():
    return [[x - (w + 16) // 2, y - 56, w + 16, 112] for _, x, y, w in ANCHORS]


def inside(point, polygon):
    x, y = point
    hit = False
    for (ax, ay), (bx, by) in zip(polygon, polygon[1:] + polygon[:1]):
        if (ay > y) != (by > y) and x < (bx-ax)*(y-ay)/(by-ay)+ax:
            hit = not hit
    return hit


def segment_distance(point, a, b):
    vx, vy = b[0]-a[0], b[1]-a[1]
    t = max(0, min(1, ((point[0]-a[0])*vx+(point[1]-a[1])*vy)/(vx*vx+vy*vy)))
    return math.dist(point, (a[0]+t*vx, a[1]+t*vy))


def placements():
    """Every visible repeated crop has a deterministic, named editable placement."""
    rows = []

    def add(name, frame, x, y, w, h, essential=False, crop_role='whole'):
        rect = [x, y, w, h]
        if not essential and any(intersects(rect, e) for e in exclusions()):
            return
        rows.append({'id': 'map_only_' + name, 'frame': frame, 'rect': rect,
                     'depth_y': y + h, 'interactive': False, 'place_id': None,
                     'interaction_id': None, 'world_solid': False, 'crop_role': crop_role})

    # Compact overhead venue complex: center and two connected flanking wings.
    add('stage_west_wing', 'north_hall', 146, 273, 190, 121, True)
    add('stage_east_wing', 'north_hall', 456, 276, 161, 119, True)
    add('stage_central_hall', 'west_hall', 280, 255, 254, 147, True)
    for i, (x, y, frame, w, h) in enumerate([
        (-42, -33, 'north_hall', 212, 153), (260, -53, 'west_hall', 201, 150),
        (600, -19, 'giveaway_pavilion', 193, 143), (-68, 313, 'west_hall', 174, 147),
        (687, 314, 'north_hall', 182, 148), (-48, 558, 'north_hall', 195, 141),
        (673, 545, 'meeting_pavilion', 169, 125), (-61, 929, 'west_hall', 231, 192),
        (620, 1006, 'north_hall', 192, 145), (-21, 1154, 'north_hall', 195, 150),
        (451, 1012, 'giveaway_pavilion', 197, 91), (728, 1121, 'west_hall', 178, 148),
        (77, 748, 'meeting_pavilion', 130, 108), (271, 757, 'meeting_pavilion', 154, 107),
        (500, 567, 'food_counter', 145, 104), (20, 431, 'white_merch_booth', 96, 100),
        (666, 167, 'white_food_booth', 103, 108), (401, 1210, 'north_hall', 231, 121),
    ]):
        add('edge_venue_%02d' % i, frame, x, y, w, h)
    # Small tree clusters close the perimeter without one large repeated canopy.
    tree_frames = ['tree_green', 'tree_yellow', 'tree_dark', 'tree_red']
    for side, x0 in [('west', -12), ('east', 715)]:
        for i, y in enumerate(range(64, 1280, 61)):
            x = x0 + (i % 3) * (17 if side == 'west' else -17)
            w = 52 + (i % 3) * 7
            add('%s_edge_tree_%02d' % (side, i), tree_frames[(i + (side == 'east')) % 4], x, y, w, 44 + i % 3 * 6)
    for i, (x, y) in enumerate([
        (119, 65), (196, 101), (475, 57), (535, 112), (595, 95),
        (121, 354), (590, 363), (564, 443), (101, 479), (527, 508),
        (89, 868), (170, 842), (579, 907), (635, 868), (184, 1067),
        (241, 1149), (286, 1215), (654, 1198), (184, 1179), (583, 714),
        (105, 180), (188, 205), (558, 234), (620, 280), (124, 620),
        (175, 640), (477, 620), (505, 674), (407, 689), (87, 793),
        (575, 803), (659, 780), (170, 903), (585, 1033), (352, 1052),
        (633, 1138), (253, 1140), (369, 1186), (687, 1250),
    ]):
        add('interior_tree_%02d' % i, tree_frames[i % 4], x, y, 42 + i % 3 * 7, 36 + i % 3 * 6)
    for i, (x, y, w, h) in enumerate([
        (312, 1098, 80, 42), (347, 1222, 51, 32),
        (545, 1241, 62, 39), (133, 576, 69, 39), (593, 505, 62, 36),
        (520, 747, 73, 39), (134, 983, 61, 35),
    ]):
        add('garden_bed_%02d' % i, 'shrub_bed', x, y, w, h)
    add('plaza_north_seating', 'plaza_furniture', 310, 414, 88, 56)
    add('meeting_north_seating', 'plaza_furniture', 342, 829, 93, 57)
    # Unequal, overlapping groups sit within/along planting, not a regular ring.
    groves = [(68, 181), (211, 108), (568, 105), (654, 440), (88, 481),
              (269, 415), (393, 565), (467, 626), (536, 737), (667, 802),
              (112, 880), (314, 811), (414, 997), (598, 946), (220, 1090),
              (340, 1230), (676, 1234)]
    offsets = [(-28, -16, 43), (10, -29, 49), (31, 4, 37),
               (-11, 19, 55), (-37, 32, 35)]
    for group, (cx, cy) in enumerate(groves):
        for member, (ox, oy, width) in enumerate(offsets):
            # Green is the structural mass; small era trees remain accents.
            frame = ['tree_green', 'tree_dark', 'tree_green', 'tree_yellow', 'tree_green'][(group+member) % 5]
            add('grove_%02d_tree_%d' % (group, member), frame,
                cx+ox, cy+oy, width, round(width*0.84))
    # Complete original silhouettes avoid cut-off lower foliage. A deterministic
    # area spiral replaces rows; parcel polygons are invisible placement bounds.
    architecture = [r['rect'] for r in rows if r['frame'] in VENUES]
    for parcel, polygon in enumerate(PLANTING):
        left, right = min(p[0] for p in polygon), max(p[0] for p in polygon)
        top, bottom = min(p[1] for p in polygon), max(p[1] for p in polygon)
        samples = math.ceil((right-left)*(bottom-top)/780)
        for sample in range(samples):
            seed = parcel*97 + sample*53
            angle = sample*math.pi*(3-math.sqrt(5)) + parcel*0.73
            radius = math.sqrt((sample+0.5)/samples)
            cx = round((left+right)/2 + radius*(right-left)*0.55*math.cos(angle))
            cy = round((top+bottom)/2 + radius*(bottom-top)*0.55*math.sin(angle))
            width = 32+(seed % 3)*4
            height = round(width*0.84)
            center = (cx, cy)
            if not inside(center, polygon):
                continue
            clearance = math.hypot(width/2, height/2)
            if any(segment_distance(center, a, b) < PATH_PLANT_CLEARANCE/2+clearance
                   for path in PATHS for a, b in zip(path, path[1:])):
                continue
            if any(segment_distance(center, a, b) < GARDEN_PLANT_CLEARANCE/2+clearance
                   for garden in GARDENS for a, b in zip(garden, garden[1:] + garden[:1])):
                continue
            rect = [cx-width//2, cy-height//2, width, height]
            if any(intersects(rect, r) for r in architecture):
                continue
            add('parcel_%02d_canopy_%03d' % (parcel, sample),
                'tree_dark' if seed % 5 == 0 else 'tree_green', *rect, crop_role='canopy')
    assert len({r['id'] for r in rows}) == len(rows)
    assert all(not intersects(r['rect'], e) for r in rows for e in exclusions())
    return sorted(rows, key=lambda r: (r['depth_y'], r['id']))


def surface(era):
    palette = PALETTES[era]
    image = Image.new('RGBA', SIZE, palette['lawn'])
    draw = ImageDraw.Draw(image)
    svg = [f'<rect width="768" height="1296" fill="{palette["lawn"]}"/>']

    def rect(x, y, w, h, color):
        draw.rectangle((x, y, x+w-1, y+h-1), fill=color)
        svg.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{color}"/>')

    # Deterministic low-contrast pixel clusters, no raster/reference sampling.
    for y in range(0, SIZE[1], 12):
        for x in range(0, SIZE[0], 12):
            n = (x * 71 + y * 137 + x * y) % 97
            if n < 20:
                rect(x, y, 7, 4, palette['lawnLight'] if n % 2 else palette['lawnDark'])
    for i, points in enumerate(PATHS):
        encoded = ' '.join(f'{x},{y}' for x, y in points)
        for color, width in zip([palette['pathEdge'], palette['pathMiddle'], palette['path']], PATH_WIDTHS):
            draw.line(points, fill=color, width=width, joint='curve')
            svg.append(f'<polyline id="path_{i}_{width}" points="{encoded}" fill="none" stroke="{color}" stroke-width="{width}" stroke-linejoin="round"/>')
    for i, points in enumerate(GARDENS):
        draw.polygon(points, fill=palette['garden'], outline='#918e6e', width=3)
        draw.line(points + [points[0]], fill=palette['ring'], width=GARDEN_BORDER, joint='curve')
        encoded = ' '.join(f'{x},{y}' for x, y in points)
        svg.append(f'<polygon id="garden_{i}" points="{encoded}" fill="{palette["garden"]}" stroke="{palette["ring"]}" stroke-width="{GARDEN_BORDER}" stroke-linejoin="round"/>')
    # Planting parcels guide placement only; keep original textured ground.
    # Paving seams stay in the path raster; SVG keeps the same explicit segments.
    pixels = image.load()
    for y in range(8, SIZE[1]-8, 18):
        for x in range(6+(y//18 % 2)*9, SIZE[0]-8, 24):
            if pixels[x, y][:3] == tuple(bytes.fromhex(palette['path'][1:])):
                draw.line([(x, y), (x+6, y)], fill=palette['seam'], width=1)
                svg.append(f'<path d="M{x} {y}h6" stroke="{palette["seam"]}"/>')
    # Tiny original flowers are explicit scenery marks, never actor silhouettes.
    for i, (x, y) in enumerate([(123, 135), (221, 437), (452, 555), (285, 578),
                              (306, 823), (510, 951), (278, 993), (358, 1060),
                              (126, 1098), (604, 1238), (561, 488), (568, 742)]):
        color = ['#f1c8bf', '#f7e5b3', '#d6cae0'][i % 3]
        for dx, dy in [(0, -3), (-3, 0), (3, 0), (0, 3)]:
            rect(x+dx, y+dy, 3, 3, color)
        rect(x, y, 3, 3, '#f9edc1')
    return image, svg, 0


def overhead_roof(canvas, svg, name, x, y, w, h, palette):
    """Quantized hip roof with short staggered clusters, not continuous bands."""
    draw = ImageDraw.Draw(canvas)
    def point(u, v):
        return (x+min(w-1, max(0, 2*round(w*u/2))),
                y+min(h-1, max(0, 2*round(h*v/2))))
    a, b, c, d = [point(*p) for p in [(0.08, 0.08), (0.89, 0.08), (1, ROOF_EAVE), (0, ROOF_EAVE)]]
    r, s = point(0.22, 0.26), point(0.76, 0.26)
    svg.append(f'<g id="{name}_original_overhead_roof" shape-rendering="crispEdges">')
    planes = [([a, b, s, r], palette['roofLight']), ([r, s, c, d], palette['roof']),
              ([a, r, d], palette['roofDark']), ([b, c, s], palette['roofDark'])]
    for index, (polygon, color) in enumerate(planes):
        draw.polygon(polygon, fill=color, outline='#7b6b68', width=1)
        points = ' '.join(f'{px},{py}' for px, py in polygon)
        svg.append(f'<polygon points="{points}" fill="{color}" stroke="#7b6b68"/>')
        # Local mask keeps all clusters off silhouette/hip seams. Short tile
        # groups have 2px details and staggered offsets; no roof-wide stripes.
        mask = Image.new('1', (w+3, h+3))
        ImageDraw.Draw(mask).polygon([(px-x, py-y) for px, py in polygon], fill=1)
        pixels = mask.load()
        for ty in range(2, h-4, 6):
            for tx in range(2+((ty//6) % 2)*4, w-8, 8):
                n = (tx*17+ty*23+index*11+len(name)) % 19
                if n > 10 or not all(pixels[tx+dx, ty+dy] for dx in range(6) for dy in range(3)):
                    continue
                tint = mix(color, palette['roofLight'] if n < 6 else palette['roofDark'], 0.20)
                draw.rectangle((x+tx, y+ty, x+tx+5, y+ty+2), fill=tint)
                svg.append(f'<rect x="{x+tx}" y="{y+ty}" width="6" height="3" fill="{tint}"/>')
    for p, q in [(r, s), (r, d), (s, c)]:
        draw.line([p, q], fill=palette['roofDark'], width=2)
        svg.append(f'<path d="M{p[0]} {p[1]}L{q[0]} {q[1]}" stroke="{palette["roofDark"]}" stroke-width="2"/>')
    svg.append('</g>')


def mix(first, second, fraction):
    a, b = bytes.fromhex(first[1:]), bytes.fromhex(second[1:])
    return '#' + ''.join(f'{round(x*(1-fraction)+y*fraction):02x}' for x, y in zip(a, b))


def produce():
    data = {}
    for name, expected in PINS.items():
        raw = (ROOT / name).read_bytes()
        assert digest(raw) == 'sha256:' + expected, name
        data[name] = raw
    binding = json.loads(data['frontend/src/app/world-entry/__tests__/reference-scene-root/source-inputs/source-binding.json'])
    assert binding['map_revision'] == 4 and binding['map_id'] == 'mcr_reference_scene_r3'
    assert binding['geometry_hash'] == 'sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f'
    spatial = json.loads(data['frontend/src/world/reference-scene/spatial-v3/source-inputs.json'])
    cells = {r['interaction_id']: (r['cell_x'], r['cell_y']) for r in spatial['geometry']['interactions']}
    assert cells == {key: (cx, cy) for key, _, _, cx, cy in CANONICAL}
    assert len({digest((ROOT / (BASE + 'map-cozy-v1/' + era.lower() + '/atlas.png')).read_bytes()) for era in THEMES}) == 3
    derivatives = json.loads(data[BASE + 'map-cozy-v1/era-derivatives.json'])
    parade = json.loads(data[BASE + 'map-cozy-v1/presentation.json'])
    decorations = placements()
    canopy_frames = {row['frame'] for row in decorations if row['crop_role'] == 'canopy'}
    assert canopy_frames <= TREES
    outputs, assets, crops, planted_areas = {}, [], [], {}
    for era in THEMES:
        base, vectors, planted_areas[era] = surface(era)
        frames = derivatives['eras'][era]['frame_rects'] if era in derivatives['eras'] else parade['frame_rects']
        source_name = BASE + 'map-cozy-v1/' + era.lower() + '/atlas.png'
        atlas_bytes = data[source_name]
        with Image.open(io.BytesIO(atlas_bytes)) as original:
            assert original.size == (1024, 1024) and original.mode == 'RGBA'
            atlas = original.copy()
        canvas = base.copy()
        svg = ['<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="768" height="1296" viewBox="0 0 768 1296">',
               '<title>Original virtual Map-only scenery; no physical routing authority</title>',
               '<defs><image id="atlas" width="1024" height="1024" xlink:href="data:image/png;base64,' + base64.b64encode(atlas_bytes).decode() + '"/></defs>',
               '<g id="editable_ground">' + ''.join(vectors) + '</g>', '<g id="editable_noninteractive_scenery" image-rendering="pixelated">']
        crop_cache = {}
        for row in decorations:
            frame = row['frame']
            f = frames[frame]
            sx, sy, sw, sh = [f[k] for k in ['x', 'y', 'width', 'height']]
            assert 0 <= sx < sx+sw <= 1024 and 0 <= sy < sy+sh <= 1024
            if frame not in crop_cache:
                crop = atlas.crop((sx, sy, sx+sw, sy+sh))
                crop_cache[frame] = crop
                crops.append({'themeId': era, 'frame': frame, 'atlas': descriptor(source_name, atlas_bytes),
                              'sourceRect': f, 'rgbaSha256': digest(crop.tobytes()), 'alphaPolicy': 'EXACT_CROP_RGBA_NO_RETOUCH',
                              'usedFacadeSourceRect': {'x': sx, 'y': sy+math.floor(sh*FACADE_SOURCE_START), 'width': sw, 'height': sh-math.floor(sh*FACADE_SOURCE_START)} if frame in VENUES else None,
                              'facadeRgbaSha256': digest(crop.crop((0, math.floor(sh*FACADE_SOURCE_START), sw, sh)).tobytes()) if frame in VENUES else None,
                              'usedCanopySourceRect': {'x': sx, 'y': sy, 'width': sw, 'height': math.floor(sh*CANOPY_SOURCE_END)} if frame in canopy_frames else None,
                              'canopyRgbaSha256': digest(crop.crop((0, 0, sw, math.floor(sh*CANOPY_SOURCE_END))).tobytes()) if frame in canopy_frames else None})
            x, y, w, h = row['rect']
            source_end = math.floor(sh*CANOPY_SOURCE_END) if row['crop_role'] == 'canopy' else sh
            scale = min(w/sw, h/source_end)
            rw, rh = max(1, math.floor(sw*scale+0.5)), max(1, math.floor(source_end*scale+0.5))
            dx, dy = x+(w-rw)//2, y+h-rh
            trim_source = math.floor(sh*FACADE_SOURCE_START) if frame in VENUES else 0
            facade_height = max(1, math.floor(rh*(1-FACADE_SOURCE_START)*FACADE_HEIGHT_SCALE+0.5)) if frame in VENUES else rh
            trim_dest = rh-facade_height
            selected = crop_cache[frame].crop((0, trim_source, sw, source_end))
            resized = selected.resize((rw, rh-trim_dest), Image.Resampling.NEAREST)
            canvas.alpha_composite(resized, (dx, dy+trim_dest))
            svg.append(f'<svg id="{row["id"]}" x="{dx}" y="{dy+trim_dest}" width="{rw}" height="{rh-trim_dest}" viewBox="{sx} {sy+trim_source} {sw} {source_end-trim_source}" preserveAspectRatio="none"><use xlink:href="#atlas"/></svg>')
            if frame in VENUES:
                overhead_roof(canvas, svg, row['id'], dx, dy, rw, rh, PALETTES[era])
        svg.append('</g></svg>\n')
        png = io.BytesIO()
        canvas.convert('RGB').save(png, format='PNG', compress_level=9)
        raw = png.getvalue()
        assert 0 < len(raw) <= 2097152
        filename = era.lower() + '-scene.png'
        outputs[filename] = raw
        outputs[era.lower() + '-scene.svg'] = ''.join(svg).encode()
        assets.append({'themeId': era, **descriptor('world/map-reference-composition-v1/' + filename, raw),
                       'width': SIZE[0], 'height': SIZE[1], 'mime': 'image/png'})
    anchors = []
    for (key, x, y, width), e in zip(ANCHORS, exclusions()):
        assert 0 <= e[0] and e[0]+e[2] <= SIZE[0] and 0 <= e[1] and e[1]+e[3] <= SIZE[1]
        anchors.append({'key': key, 'x': x, 'y': y, 'width': width,
                        'exclusionRect': dict(zip(['x', 'y', 'width', 'height'], e))})
    canonical = [{'interactionId': key, 'x': x, 'y': y, 'worldCell': {'x': cx, 'y': cy},
                  'authority': 'EXISTING_CANONICAL_IDENTITY_ONLY'} for key, x, y, cx, cy in CANONICAL]
    original = {'mapId': binding['map_id'], 'mapRevision': binding['map_revision'],
                'mapContentHash': binding['map_content_hash'], 'geometryHash': binding['geometry_hash'],
                'sourceBinding': descriptor('frontend/src/app/world-entry/__tests__/reference-scene-root/source-inputs/source-binding.json',
                    data['frontend/src/app/world-entry/__tests__/reference-scene-root/source-inputs/source-binding.json']),
                'themes': [{'themeId': era, 'themeRevision': 1, 'themeContentHash': 'sha256:' + pin} for era, pin in THEMES.items()]}
    layout = {'schemaVersion': 1, 'version': VERSION, 'recipeRevision': RECIPE_REVISION,
              'coordinateSpace': 'MAP_ILLUSTRATION_ONLY',
              'renderMode': 'full-composition', 'dimensions': {'width': 768, 'height': 1296},
              'originalMap4': original, 'serviceLabelAnchors': anchors, 'canonicalDisplayAnchors': canonical,
              'assets': assets, 'sourceInputs': [descriptor(k, v) for k, v in data.items()],
              'decorations': decorations, 'paths': PATHS, 'gardenParcels': GARDENS,
              'plantingParcels': PLANTING,
              'authority': {'physicalArrival': False, 'physicalRoutes': False, 'retailAvailability': False,
                            'canonicalIdentityMutation': False, 'worldGeometryMutation': False,
                            'serviceAnchors': 'NONCANONICAL_VIRTUAL_SEMANTICS_RUNTIME_OWNER_BINDS_EXISTING_FEATURE_LINKS',
                            'displayMapping': 'EXPLICIT_OVERVIEW_ILLUSTRATION_ONLY_NOT_SERVER_COORDINATES'}}
    outputs['layout.json'] = encode(layout)
    provenance = {'format': 'MCR_MAP_REFERENCE_COMPOSITION_SOURCE_V1', 'status': 'SOURCE_VALIDATED_NATIVE_NOT_RUN',
                  'version': VERSION, 'sourceBaseCommit': '6bb850755f1b6540a8d013f84106c673c5567069',
                  'previousSourceCandidate': PREVIOUS_HANDOFF, 'recipeRevision': RECIPE_REVISION,
                  'sourceInputs': layout['sourceInputs'], 'originalCropPins': crops,
                  'outputs': [descriptor('frontend/public/world/map-reference-composition-v1/' + k, v) for k, v in outputs.items()],
                  'recipe': descriptor('frontend/public/world/map-reference-composition-v1/build.py', Path(__file__).read_bytes()),
                  'referenceUse': 'DIRECT_VISUAL_INSPECTION_ONLY_NO_REFERENCE_PIXELS_SAMPLED',
                  'authorship': 'New editable paths/gardens/placement geometry; retained original built-in generated warm atlas crops. No new generation, stock, character or font input.',
                  'transform': 'Whole frames including complete foliage silhouettes retain uniform nearest fit, centered X/bottom Y. Venue bottom46percent source crops retain width and receive explicit 0.80 vertical nearest projection, bottom-aligned; original RGBA unchanged before sampling.',
                  'venueProjection': {'sourceStartFraction': FACADE_SOURCE_START,
                                      'facadeHeightScale': FACADE_HEIGHT_SCALE,
                                      'roofEaveFraction': ROOF_EAVE,
                                      'roofCoordinateQuantumPx': 2,
                                      'roofTextureCellPx': [8, 6],
                                      'policy': 'ORIGINAL_MAP_ONLY_PIXEL_HIP_ROOF_NO_WORLD_ATLAS_MUTATION'},
                  'planting': {'parcels': PLANTING, 'paintedGroundPixelsByTheme': planted_areas,
                               'pathClearancePx': PATH_PLANT_CLEARANCE,
                               'gardenEdgeClearancePx': GARDEN_PLANT_CLEARANCE,
                               'pathWidthsPx': list(PATH_WIDTHS), 'gardenBorderPx': GARDEN_BORDER,
                               'canopySourceEndFraction': CANOPY_SOURCE_END,
                               'canopyPlacements': sum(row['crop_role'] == 'canopy' for row in decorations),
                               'excludes': ['six exact label rectangles', 'declared path corridors', 'declared garden edges'],
                               'substratePolicy': 'NONE_KEEP_EXISTING_TEXTURED_GROUND',
                               'placementSampler': 'DETERMINISTIC_AREA_SPIRAL_NO_ROW_GRID',
                               'coverageMeaning': 'Zero new substrate pixels: parcels are invisible placement domains, not vegetation coverage or reference/runtime similarity'},
                  'surfacePaletteByTheme': PALETTES,
                  'svgPolicy': 'Editable SVG embeds the exact era atlas; Pillow PNG is raster authority. SVG engine parity NOT_RUN.',
                  'checks': {'immutableInputHashes': True, 'closedThreeEras': True, 'uniqueDecorativeIds': True,
                             'canonicalWorldCellsExact': True, 'architectureOutsideLabelExclusions': True,
                             'labelExclusionHeightSourcePx': 112, 'eachPngMaxBytes': 2097152, 'outputDimensions': list(SIZE)},
                  'notRun': ['actual default App composition', 'three-width all-era native comparison',
                             'World RGBA invariance', 'feature link/focus/selection', 'human final acceptance',
                             'physical venue, devices, live multiplayer']}
    outputs['source-provenance.json'] = encode(provenance)
    assert sum(map(len, outputs.values())) < 10*1024*1024
    return outputs, len(decorations)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--verify', action='store_true')
    args = parser.parse_args()
    outputs, count = produce()
    expected = set(outputs) | {'build.py', 'README.md'}
    if args.verify:
        assert {p.name for p in HERE.iterdir()} == expected
        for name, raw in outputs.items():
            assert (HERE/name).read_bytes() == raw, name
    else:
        assert not ({p.name for p in HERE.iterdir()} - expected)
        for name, raw in outputs.items():
            (HERE/name).write_bytes(raw)
    print(json.dumps({'status': 'PASS', 'mode': 'verify' if args.verify else 'build',
                      'outputs': len(outputs), 'bytes': sum(map(len, outputs.values())),
                      'noninteractiveDecorations': count,
                      'pngs': [descriptor(k, v) for k, v in outputs.items() if k.endswith('.png')]}))


if __name__ == '__main__':
    main()
