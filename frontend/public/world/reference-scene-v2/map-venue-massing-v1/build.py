#!/usr/bin/env python3
"""Small MAP-only source assembly; no raster, scene export or runtime changes."""
import hashlib
import json
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parent
ROOT = SOURCE.parents[3]
read = lambda p: json.loads(p.read_text())
sha = lambda p: 'sha256:' + hashlib.sha256(p.read_bytes()).hexdigest()
pin = lambda p: {'path': str(p.relative_to(ROOT)), 'bytes': p.stat().st_size, 'sha256': sha(p)}
geometry = read(SOURCE/'spatial-v5/geometry-proposal.json')
labels = read(SOURCE/'map-layout-v2/label-presentation.json')
landscape = read(SOURCE/'map-layout-v2/landscape-presentation.json')
assert geometry['geometry_hash'] == labels['geometry_hash'] == landscape['geometry_hash'] == 'sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395'
assert sha(ROOT/'docs/mcr/design/reference-target-2026-11-04.png') == 'sha256:d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617'
assert landscape['required_label_presentation'] == pin(SOURCE/'map-layout-v2/label-presentation.json')

annexes = [
    {'id': 'map_stage_west_annex', 'frame': 'north_hall', 'x': 117, 'y': 244,
     'width': 169, 'height': 128, 'depth_y': 372},
    {'id': 'map_stage_east_annex', 'frame': 'north_hall', 'x': 436, 'y': 280,
     'width': 122, 'height': 90, 'depth_y': 370}
]
for obj in annexes:
    obj.update(view='MAP_ONLY', interaction_id=None, place_id=None,
               kind='unbound_map_architecture_illustration_not_a_world_solid')

def disjoint(a, b):
    return a['x']+a['width'] <= b['x'] or b['x']+b['width'] <= a['x'] or a['y']+a['height'] <= b['y'] or b['y']+b['height'] <= a['y']

points = [dict(p, kind='interaction') for p in geometry['geometry']['interactions']]
points += [dict(p, kind='spawn') for p in geometry['geometry']['spawns']]
clearances = []
for obj in annexes:
    assert 0 <= obj['x'] < obj['x']+obj['width'] <= 768
    assert 0 <= obj['y'] < obj['y']+obj['height'] <= 1296
    assert obj['depth_y'] == obj['y']+obj['height'] < 373
    assert all(disjoint(obj, label['rect']) for label in labels['labels'])
    assert all(disjoint(obj, garden) for garden in landscape['decorations'])
    for p in points:
        x, y = p['cell_x']*16+8, p['cell_y']*16+8
        dx = max(obj['x']-x, 0, x-(obj['x']+obj['width']))
        dy = max(obj['y']-y, 0, y-(obj['y']+obj['height']))
        distance = math.hypot(dx, dy)
        assert distance >= 8
        clearances.append({'annex': obj['id'], 'point': p.get('interaction_id', p.get('spawn_id')),
                           'kind': p['kind'], 'distance_px': distance})

themes = []
for era in ['revenge', 'parade', 'danger']:
    folder = SOURCE/'architecture-v2'/era
    layout = read(folder/'layout.json')
    old = read(SOURCE/'quality-v10'/era/'layout.json')
    assert layout['frames'] == old['frames']
    assert layout['placements'] == old['placements']
    assert layout['overview_representations'] == old['overview_representations']
    assert len(layout['placements']) == 34
    stage = next(o for o in layout['overview_representations'] if o['id'] == 'stage_venue_representation')
    assert (stage['x'], stage['y'], stage['width'], stage['height'], stage['depth_y']) == (185, 163, 300, 210, 373)
    scene = read(folder/'scenery.json')
    atlas = scene['atlases'][0]
    assert sha(ROOT/'frontend/public'/atlas['path']) == atlas['sha256']
    provenance = read(folder/'provenance.json')
    transform = next(t for t in provenance['transforms'] if t['frame'] == 'north_hall')
    resources = read(folder/'resource-inputs.json')
    themes.append({'theme_id': era.upper(), 'layout': pin(folder/'layout.json'),
                   'scenery': pin(folder/'scenery.json'), 'atlas': atlas,
                   'native_atlas': pin(folder/'native-atlas.png'),
                   'source_provenance': pin(folder/'provenance.json'),
                   'frame': layout['frames']['north_hall'], 'source_transform': transform,
                   'runtime_pngs': 2, 'runtime_bytes': resources['total_bytes'],
                   'runtime_pixels': resources['total_pixels']})

presentation = {
    'format': 'MCR_MAP_VENUE_MASSING_V1', 'version': 1,
    'status': 'CONTRACT_READY_METADATA_RENDER_VALIDATION_NOT_RUN',
    'view': 'MAP_ONLY', 'geometry_hash': geometry['geometry_hash'],
    'required_labels': pin(SOURCE/'map-layout-v2/label-presentation.json'),
    'required_landscape': pin(SOURCE/'map-layout-v2/landscape-presentation.json'),
    'required_architecture': pin(SOURCE/'architecture-v2/atlas-handoff.json'),
    'reference': pin(ROOT/'docs/mcr/design/reference-target-2026-11-04.png'),
    'evidence': {
        'capture_source_commit': '4cb162b71133446e73533ce3471b06fffea6327b',
        'retained_artifact_prefix': 'docs/mcr/evidence/reference-scene-v5/architecture-v2/native-04/',
        'publication_at_comparison': 'PENDING',
        'map': {'file': 'map-parade-390.png', 'bytes': 409928, 'sha256': 'sha256:19d05754866c6ab54004a89dbb3e6026cf3b5b2b7b2e33c5f5a749ff1c907e03'},
        'editable': {'file': 'editable-parade.svg', 'bytes': 2657147, 'sha256': 'sha256:cb34694ac25ffa5e488aa2c98094f23a0c4950a3eb5622ed590d21f606db4cb5'},
        'editable_raster': {'file': 'editable-parade.png', 'bytes': 1476218, 'sha256': 'sha256:9156109e82573868a1a9d24585f305b890353a96889392b34988359c590ba524'},
        'finding': 'Matching MAP source view shows a narrow standalone central block; original reference depicts a venue complex with flanking wings. Existing taller facade source survives packing/rendering, but does not complete the architectural mass.'
    },
    'annexes': annexes, 'themes': themes,
    'composition': 'Add two existing-atlas illustrations into MAP stable ascending depth_y ordering, behind unchanged stage_venue_representation at373. Preserve all current objects, labels, gardens and marker order. WORLD ignores this input.',
    'authority': 'Unbound virtual MAP art only. Never emit canonical objects, collisions, destinations, interaction targets, selected states or physical eligibility.',
    'unchanged': ['34 WORLD placements and source geometry', 'existing stage_venue_representation', 'all frame rectangles and PNG bytes', 'all six labels and ten gardens', 'canonical feet, spawns, interactions, camera and release bindings'],
    'bounds': {'original_stage_frame': [185, 163, 300, 210], 'proposed_combined_frame_envelope': [117, 163, 441, 210], 'metric_scope': 'Frame rectangles, not a pixel-perfect reference measurement or visible-alpha envelope.'},
    'validation': {'new_images': 0, 'new_native_exports': 0, 'new_browser_runs': 0,
                   'label_and_garden_rectangles_disjoint': True, 'canonical_point_checks': clearances,
                   'native_visibility_occlusion': 'NOT_RUN', 'runtime_adoption': 'NOT_RUN',
                   'mobile_reference_comparison': 'NOT_RUN', 'full_reference_fidelity': 'IN_PROGRESS_NOT_ACCEPTED'},
    'consumer': 'Requires a newly recorded exact consumer lease and explicit paired hash validation. Existing native04 renders do not qualify these new wings. No mandatory manager review.'
}
payload = json.dumps(presentation, indent=2) + '\n'
if '--verify' in sys.argv:
    assert (HERE/'presentation.json').read_text() == payload
else:
    (HERE/'presentation.json').write_text(payload)
print(json.dumps({'status': presentation['status'], 'annexes': len(annexes), 'eras': len(themes),
                  'canonical_clearance_checks': len(clearances), 'minimum_clearance_px': min(p['distance_px'] for p in clearances),
                  'new_runtime_textures': 0, 'presentation_bytes': len(payload.encode())}))
