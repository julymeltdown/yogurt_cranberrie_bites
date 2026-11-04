#!/usr/bin/env python3
"""Inspect the immutable generated sheet; never repaint, pack or export it."""
import hashlib
import json
import math
import sys
from collections import deque
from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
THRESHOLD = 32
ERAS = ['revenge', 'parade', 'danger']
FRAMES = ['meeting_pavilion', 'giveaway_pavilion', 'north_hall', 'west_hall']
EXPECTED = 'ef8f0a5c05702940bae39b0e4661f6a5223f8afd1e351a96f9d28186e0d4769a'
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
sheet = HERE / 'venue-sheet.png'
assert sha(sheet) == EXPECTED
im = Image.open(sheet)
assert im.mode == 'RGBA' and im.size == (1254, 1254)
alpha = im.getchannel('A')
assert alpha.getextrema() == (0, 255)
w, h = im.size
mask = bytearray(value >= THRESHOLD for value in alpha.get_flattened_data())
components = []
for start in range(w * h):
    if not mask[start]:
        continue
    mask[start] = 0
    queue = deque([start])
    count, x0, y0, x1, y1 = 0, w, h, 0, 0
    while queue:
        index = queue.popleft()
        x, y = index % w, index // w
        count += 1
        x0, y0, x1, y1 = min(x0, x), min(y0, y), max(x1, x), max(y1, y)
        neighbors = ([index-1] if x else []) + ([index+1] if x < w-1 else [])
        neighbors += ([index-w] if y else []) + ([index+w] if y < h-1 else [])
        for neighbor in neighbors:
            if mask[neighbor]:
                mask[neighbor] = 0
                queue.append(neighbor)
    if count > 20:
        components.append({'pixels': count, 'bbox': [x0, y0, x1+1, y1+1]})
assert len(components) == 12
assert all(0 < c['bbox'][0] < c['bbox'][2] < w and
           0 < c['bbox'][1] < c['bbox'][3] < h for c in components)

def gutter(target, extent, score):
    return min(range(max(1, round(target)-35), min(extent-1, round(target)+35)+1),
               key=lambda n: (score(n), abs(n-target), n))

rows = [0] + [gutter(i*h/4, h, lambda y: sum(alpha.getpixel((x, y)) >= THRESHOLD
             for x in range(w))) for i in [1, 2, 3]] + [h]
frames = []
for row, name in enumerate(FRAMES):
    top, bottom = rows[row:row+2]
    cols = [0] + [gutter(i*w/3, w, lambda x: sum(alpha.getpixel((x, y)) >= THRESHOLD
                 for y in range(top, bottom))) for i in [1, 2]] + [w]
    for col, era in enumerate(ERAS):
        left, right = cols[col:col+2]
        cell = alpha.crop((left, top, right, bottom))
        core = cell.point(lambda a: 255 if a >= THRESHOLD else 0).getbbox()
        assert core
        x0, y0 = max(left, left+core[0]-2), max(top, top+core[1]-2)
        x1, y1 = min(right, left+core[2]+2), min(bottom, top+core[3]+2)
        members = [c for c in components if left <= c['bbox'][0] and c['bbox'][2] <= right
                   and top <= c['bbox'][1] and c['bbox'][3] <= bottom]
        assert len(members) == 1
        scale = min(240/(x1-x0), 176/(y1-y0))
        packed = [math.floor((x1-x0)*scale+.5), math.floor((y1-y0)*scale+.5)]
        frames.append({'era': era, 'frame': name, 'source_cell': [left, top, right, bottom],
                       'source_rect': [x0, y0, x1-x0, y1-y0],
                       'core_component': members[0], 'planned_uniform_scale': scale,
                       'planned_packed_size': packed})

target = ROOT / 'docs/mcr/design/reference-target-2026-11-04.png'
old = HERE.parent / 'quality-v7/sources/venue-sheet.png'
assert sha(target) == 'd9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617'
assert sha(old) == '051a063e048f50e1abd3ec143e8c74f0cfb61545f927aeb08e6eb640f5aed122'
report = {
    'format': 'MCR_ARCHITECTURE_SOURCE_V2',
    'status': 'SOURCE_CANDIDATE_CORE_CUTOUTS_VERIFIED_PACKING_NOT_RUN',
    'source': {'path': 'venue-sheet.png', 'sha256': sha(sheet), 'bytes': sheet.stat().st_size,
               'width': w, 'height': h, 'mode': im.mode, 'raw_alpha_bbox': alpha.getbbox()},
    'prompt': {'path': 'prompt.md', 'sha256': sha(HERE/'prompt.md')},
    'generation': {'tool': 'built-in image_gen', 'intent': 'generate with references',
                   'transparent_background': True, 'job': 'ART-ARCH-SOURCE-01',
                   'provider_basename': 'exec-2cb8ded0-0fc9-414f-ad15-44e1758336c9.png',
                   'owned_copy': 'byte-exact; provider original preserved'},
    'reference_pins': [{'path': 'docs/mcr/design/reference-target-2026-11-04.png',
                        'sha256': sha(target), 'role': 'MAP architecture/style reference only'},
                       {'path': 'quality-v7/sources/venue-sheet.png', 'sha256': sha(old),
                        'role': 'old era/grid reference; unchanged'}],
    'geometry_hash': 'sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395',
    'core_alpha_threshold': THRESHOLD, 'core_components': 12, 'row_cuts': rows,
    'frames': frames,
    'source_findings': [
        'Two visible floor levels in rows1-3 and three in row4 replace single-storey forms.',
        'Raw sub32-alpha residuals touch left/bottom sheet edges. Original pixels are retained.',
        'Core silhouettes do not touch outer edges. Planned crops enclose each core with2px margin.',
        'Rows1,2,4 are taller/narrower than requested1.35-1.5 aspect; uniform letterboxing must preserve them.',
        'No generated character, text, extra place identity or scene geometry is introduced.'
    ],
    'packing_policy': 'NOT_RUN. Future allocated job may crop recorded rectangles and uniformly nearest-scale RGBA into existing244x180 frames, centered/bottom-aligned2px. No alpha thresholding/repaint/nonuniform scaling; clipping of faint exterior residue is recorded by these source crops.',
    'acceptance': {'atlas': 'NOT_RUN', 'native': 'NOT_RUN', 'runtime': 'NOT_RUN',
                   'reference_fidelity': 'IN_PROGRESS_NOT_ACCEPTED', 'release': 'NOT_ADMITTED'}
}
payload = json.dumps(report, indent=2) + '\n'
if '--write' in sys.argv:
    (HERE/'source-metadata.json').write_text(payload)
else:
    assert (HERE/'source-metadata.json').read_text() == payload
print(json.dumps({'status': report['status'], 'frames': len(frames),
                  'source_bytes': sheet.stat().st_size, 'source_sha256': sha(sheet),
                  'row_cuts': rows, 'packed_sizes_not_outputs': [f['planned_packed_size'] for f in frames]}))
