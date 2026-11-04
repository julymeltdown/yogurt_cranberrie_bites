#!/usr/bin/env python3
"""ART-ARCH-ATLAS-02: deterministic building-only derivatives, no scene export."""
import copy
import hashlib
import json
import math
import os
import sys
from pathlib import Path
from PIL import Image, ImageChops

HERE = Path(__file__).resolve().parent
BASE = HERE.parent / 'quality-v10'
ROOT = HERE.parents[4]
ERAS = ['revenge', 'parade', 'danger']
BUILDINGS = ['meeting_pavilion', 'giveaway_pavilion', 'north_hall', 'west_hall']
read = lambda p: json.loads(p.read_text())
sha = lambda p: 'sha256:' + hashlib.sha256(p.read_bytes()).hexdigest()
write = lambda p, j: p.write_text(json.dumps(j, indent=2) + '\n')
pin = lambda p: {'path': str(p.relative_to(ROOT)), 'sha256': sha(p), 'bytes': p.stat().st_size}
box = lambda f: (f['x'], f['y'], f['x']+f['width'], f['y']+f['height'])
source_meta = read(HERE/'source-metadata.json')
assert sha(HERE/'venue-sheet.png') == 'sha256:' + source_meta['source']['sha256']
verify_only = '--verify' in sys.argv
if not verify_only:
    stat = os.statvfs(HERE)
    assert stat.f_bavail * stat.f_frsize >= 8*2**30 + 64*2**20

results = []
for era in ERAS:
    original_layout = read(BASE/era/'layout.json')
    original_scene = read(BASE/era/'scenery.json')
    atlas_resource = original_scene['atlases'][0]
    original_path = ROOT/'frontend/public'/atlas_resource['path']
    assert sha(original_path) == atlas_resource['sha256']
    folder = HERE/era
    if not verify_only:
        folder.mkdir(exist_ok=True)
    original = Image.open(original_path).convert('RGBA')
    atlas = original.copy()
    transforms = []
    source = Image.open(HERE/'venue-sheet.png').convert('RGBA')
    for record in [f for f in source_meta['frames'] if f['era'] == era]:
        name = record['frame']
        frame = original_layout['frames'][name]
        assert (frame['width'], frame['height']) == (244, 180)
        x, y, w, h = record['source_rect']
        scale = min(240/w, 176/h)
        dw, dh = math.floor(w*scale+.5), math.floor(h*scale+.5)
        dx, dy = frame['x']+(244-dw)//2, frame['y']+180-dh-2
        crop = source.crop((x, y, x+w, y+h)).resize((dw, dh), Image.Resampling.NEAREST)
        atlas.paste((0, 0, 0, 0), box(frame))
        atlas.paste(crop, (dx, dy))
        transforms.append({'frame': name, 'source_rect': record['source_rect'],
                           'frame_rect': frame, 'packed_rect': [dx, dy, dw, dh],
                           'scale': scale, 'resampling': 'nearest',
                           'rounding': 'nearest ties away from zero',
                           'source_rgba_policy': 'Exact crop RGBA, no thresholding or repaint; recorded rectangle omits exterior faint residue.',
                           'padding': 'Transparent, horizontally centered, bottom-aligned2px.'})
    source.close()
    outside = ImageChops.difference(atlas, original)
    for name in BUILDINGS:
        outside.paste((0, 0, 0, 0), box(original_layout['frames'][name]))
        assert atlas.crop(box(original_layout['frames'][name])).tobytes() != original.crop(box(original_layout['frames'][name])).tobytes()
    assert outside.getbbox(alpha_only=False) is None
    unchanged = []
    for name, frame in original_layout['frames'].items():
        if name in BUILDINGS:
            continue
        data = atlas.crop(box(frame)).tobytes()
        assert data == original.crop(box(frame)).tobytes()
        unchanged.append({'frame': name, 'rgba_sha256': hashlib.sha256(data).hexdigest()})
    assert len(unchanged) == 12
    del outside, original
    if not verify_only:
        atlas.save(folder/'atlas.png', optimize=True, compress_level=9)
    disk_atlas = Image.open(folder/'atlas.png').convert('RGBA')
    assert disk_atlas.size == (1024, 1024) and disk_atlas.tobytes() == atlas.tobytes()
    del disk_atlas
    native = Image.new('RGBA', (2048, 2048))
    native.paste(atlas, (0, 0))
    if not verify_only:
        native.save(folder/'native-atlas.png', optimize=True, compress_level=9)
    disk_native = Image.open(folder/'native-atlas.png').convert('RGBA')
    assert disk_native.size == (2048, 2048)
    assert disk_native.tobytes() == native.tobytes()
    del disk_native, native, atlas
    descriptor = {'id': era+'_architecture_v2_atlas',
                  'path': 'world/reference-scene-v2/architecture-v2/'+era+'/atlas.png',
                  'sha256': sha(folder/'atlas.png'), 'width_px': 1024, 'height_px': 1024,
                  'bytes': (folder/'atlas.png').stat().st_size}
    layout = copy.deepcopy(original_layout)
    layout.update(source_variant='architecture-v2', ground='../../quality-v10/ground.png',
                  raster_source_policy='Four existing building frames replaced by recorded original uniform-nearest source crops; all other atlas pixels exact.',
                  runtime_derivative='SOURCE_ONLY_NOT_ADOPTED. All frame rectangles, placements, footprints, depths, interactions, cameras and geometry unchanged.')
    scene = copy.deepcopy(original_scene)
    scene['atlases'] = [descriptor]
    for obj in scene['objects']:
        obj['atlas_id'] = descriptor['id']
    resources = copy.deepcopy(read(BASE/era/'resource-inputs.json'))
    resources.update(source_variant='architecture-v2', resources=[scene['ground'], descriptor],
                     total_bytes=scene['ground']['bytes']+descriptor['bytes'],
                     map_metadata='../../quality-v10/'+era+'/map-metadata.json')
    assert resources['total_bytes'] <= resources['limits']['bytes']
    assert resources['total_pixels'] <= resources['limits']['pixels']
    provenance = {'format': 'MCR_ARCHITECTURE_V2_ATLAS_DERIVATIVE',
                  'status': 'SOURCE_ONLY_NOT_RELEASE_BOUND', 'theme_id': era.upper(),
                  'geometry_hash': original_layout['geometry_hash'],
                  'base_source_commit': '1bcf52081f8c6ed7dfdc0dceba90e7d5155579fa',
                  'inputs': [pin(HERE/'venue-sheet.png'), pin(HERE/'source-metadata.json'),
                             pin(original_path), pin(BASE/era/'layout.json'),
                             pin(BASE/era/'scenery.json'), pin(BASE/'ground.png')],
                  'transforms': transforms, 'unchanged_frames': unchanged,
                  'outside_four_frames_exact_rgba': True,
                  'native_padding': {'source_rect': [0, 0, 1024, 1024], 'size': [2048, 2048],
                                     'policy': 'Exact atlas top-left, all other pixels transparent; raster derivative only, not a native scene export.'},
                  'limits': ['No native project, scene export, browser or consumer qualification.',
                             'No current MAP layout/planting source pins are silently rebound.',
                             'Geometry, server feet, approved characters and release admission unchanged.']}
    for name, value in [('layout.json', layout), ('scenery.json', scene),
                        ('resource-inputs.json', resources), ('provenance.json', provenance)]:
        if verify_only:
            assert read(folder/name) == value
        else:
            write(folder/name, value)
    results.append({'era': era, 'runtime_bytes': resources['total_bytes'],
                    'runtime_pixels': resources['total_pixels'], 'atlas': pin(folder/'atlas.png'),
                    'native_padding': pin(folder/'native-atlas.png'),
                    'other12frames_and_outside_pixels': 'EXACT_RGBA',
                    'frozen_layout_fields': [k for k in original_layout if k not in
                     ['ground', 'raster_source_policy', 'runtime_derivative']]})

outputs = [HERE/era/name for era in ERAS for name in
           ['atlas.png', 'native-atlas.png', 'layout.json', 'scenery.json', 'resource-inputs.json', 'provenance.json']]
total = sum(p.stat().st_size for p in outputs)
assert total <= 24*2**20
handoff = {'format': 'MCR_ARCHITECTURE_V2_ATLAS_HANDOFF',
           'status': 'CONTRACT_READY_SOURCE_DERIVATIVES_RUNTIME_NOT_RUN',
           'job': 'ART-ARCH-ATLAS-02', 'geometry_hash': source_meta['geometry_hash'],
           'outputs': [pin(p) for p in outputs], 'derivative_bytes': total, 'themes': results,
           'pairing': 'New atlas resource pins require explicit new consumer ownership and matching MAP label/planting pin closure. Never mix silently with old v10/layout-v2 atlas pins.',
           'unchanged': ['Original ground', 'all16 frame rectangles', 'other12 frames and pixels outside four building rectangles',
                         '34 WORLD placements/footprints/depths/interactions', 'MAP representation destinations',
                         'labels/gardens/cameras/geometry/collision/characters/server/release'],
           'not_run': ['Native projects and scene renders', 'mobile reference comparisons',
                       'occlusion/visual massing in scene', 'application/runtime adoption', 'full reference acceptance']}
if verify_only:
    assert read(HERE/'atlas-handoff.json') == handoff
else:
    write(HERE/'atlas-handoff.json', handoff)
print(json.dumps({'status': handoff['status'], 'outputs': len(outputs), 'derivative_bytes': total,
                  'themes': [{k: t[k] for k in ['era', 'runtime_bytes', 'runtime_pixels']} for t in results]}))
