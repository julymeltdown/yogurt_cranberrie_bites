#!/usr/bin/env python3
"""Reproduce the versioned spatial-v3 public inputs; never admit a release."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
REVISION = 'bc731ed868ec30b33ae5dd1e6f13e97e68ecabd9'
GEOMETRY = 'sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f'


def read(path, expected):
    raw = (ROOT / path).read_bytes()
    assert 'sha256:' + hashlib.sha256(raw).hexdigest() == expected, path
    return json.loads(raw), dict(path='world/reference-scene-v2/' + path,
                                 sha256=expected, bytes=len(raw))


source, source_pin = read('spatial-v3/geometry-proposal.json',
                         'sha256:373dfdb9ee119c5c2ae39b9b5843732e9fb6ca3d7030c2a26f026adc40a1e8dc')
manifest, manifest_pin = read('quality-v6/manifest.json',
                             'sha256:a78fbd045e9123e18ffa01efc669dfef9965bbeccb1cd378b886b9b975247aed')
_, projection_pin = read('spatial-v3/integer-projection.json',
                         manifest['integer_projection']['sha256'])
g = source['geometry']
assert source['geometry_hash'] == GEOMETRY
assert 'sha256:' + hashlib.sha256(json.dumps(g, sort_keys=True, separators=(',', ':'),
                                             ensure_ascii=True).encode()).hexdigest() == GEOMETRY
assert (g['width_cells'], g['height_cells'], g['tile_width_px'], g['tile_height_px']) == (48, 81, 16, 16)
tiled = dict(type='map', version='1.10', tiledversion='1.10.2', orientation='orthogonal',
             renderorder='right-down', infinite=False, width=48, height=81, tilewidth=16, tileheight=16)
tiled['layers'] = [dict(id=i+1, name=name, type='tilelayer', width=48, height=81, x=0, y=0,
                        opacity=1, visible=True, data=[v for row in g[key] for v in row])
                   for i, (name, key) in enumerate([('terrain', 'terrain_layer'),
                                                    ('collision', 'collision_grid'),
                                                    ('overdraw', 'overdraw_layer')])]
tiled['tilesets'] = [dict(firstgid=1, name='theme', image='mcr-theme.png', imagewidth=768,
                          imageheight=1296, tilewidth=16, tileheight=16, tilecount=3888,
                          columns=48, margin=0, spacing=0)]
encoded = (json.dumps(tiled, separators=(',', ':')) + '\n').encode()
(HERE / 'canonical-map-v4.json').write_bytes(encoded)
scenes = []
for theme in manifest['themes']:
    files = []
    for pin in theme['files']:
        raw = (ROOT / 'quality-v6' / pin['path']).read_bytes()
        assert len(raw) == pin['bytes']
        assert 'sha256:' + hashlib.sha256(raw).hexdigest() == pin['sha256']
        files.append(dict(pin, path='world/reference-scene-v2/quality-v6/' + pin['path']))
    scene = json.loads((ROOT / 'quality-v6' / theme['theme_id'].lower() / 'scenery.json').read_text())
    for resource in [scene['ground'], *scene['atlases']]:
        raw = (ROOT.parent.parent / resource['path']).read_bytes()
        assert len(raw) == resource['bytes']
        assert 'sha256:' + hashlib.sha256(raw).hexdigest() == resource['sha256']
    assert sum(r['bytes'] for r in [scene['ground'], *scene['atlases']]) <= 2097152
    assert sum(r['width_px'] * r['height_px'] for r in [scene['ground'], *scene['atlases']]) <= 2097152
    scenes.append(dict(theme_id=theme['theme_id'], sources=files,
                       runtime_resources=[scene['ground'], *scene['atlases']]))
descriptor = dict(format='MCR_REFERENCE_SCENE_SPATIAL3_PUBLIC_INPUTS', version=1,
                  status='NOT_RELEASE_BOUND', production_status='NOT_ADMITTED', source_revision=REVISION,
                  geometry_source=source_pin, integer_projection=projection_pin, artist_manifest=manifest_pin,
                  candidate_map=dict(map_id='mcr_reference_scene_r3', map_revision=4, geometry_hash=GEOMETRY),
                  tilemap=dict(path='world/reference-scene-v2/runtime/spatial-v3/canonical-map-v4.json',
                               sha256='sha256:' + hashlib.sha256(encoded).hexdigest(), bytes=len(encoded), format='TILED_JSON'),
                  compatibility_texture=dict(width_px=768, height_px=1296, image_marker='mcr-theme.png',
                                             marker_policy='Inert logical tileset binding. Never fetch this marker.'),
                  scenes=scenes,
                  authority='Static source inputs and proposed map identity only; no canonical backend manifest hash or admission asserted.',
                  limitations=['Required geometry/fidelity consumer increment, not final visual acceptance',
                               'Original virtual concept places remain unchanged; no physical destination authority',
                               'No product/bootstrap/operator/live qualification'])
(HERE / 'source-binding-v4.json').write_text(json.dumps(descriptor, indent=2) + '\n')
print(json.dumps(descriptor['tilemap']))
