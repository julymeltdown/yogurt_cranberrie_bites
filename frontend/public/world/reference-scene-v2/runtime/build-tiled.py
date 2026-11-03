#!/usr/bin/env python3
"""Reproduce the sole public finite Tiled projection. This is not release admission."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parent / 'geometry-proposal.json'
EXPECTED = 'd2650b06842b577eb8a8a455446a10e8f69ef5f9c6cfa2dd2d6008fcc7b89542'
raw = SOURCE.read_bytes()
assert hashlib.sha256(raw).hexdigest() == EXPECTED, 'Frozen source changed'
source = json.loads(raw)
g = source['geometry']
assert (g['width_cells'], g['height_cells'], g['tile_width_px'], g['tile_height_px']) == (48, 81, 16, 16)
tiled = dict(type='map', version='1.10', tiledversion='1.10.2', orientation='orthogonal', renderorder='right-down', infinite=False,
             width=48, height=81, tilewidth=16, tileheight=16)
tiled['layers'] = [dict(id=i+1, name=name, type='tilelayer', width=48, height=81, x=0, y=0, opacity=1, visible=True,
                        data=[v for row in g[key] for v in row]) for i, (name, key) in enumerate([
                            ('terrain', 'terrain_layer'), ('collision', 'collision_grid'), ('overdraw', 'overdraw_layer')])]
tiled['tilesets'] = [dict(firstgid=1, name='theme', image='mcr-theme.png', imagewidth=768, imageheight=1296,
                          tilewidth=16, tileheight=16, tilecount=3888, columns=48, margin=0, spacing=0)]
encoded = (json.dumps(tiled, separators=(',', ':'))+'\n').encode()
(HERE/'canonical-map-v2.json').write_bytes(encoded)
descriptor = dict(format='MCR_REFERENCE_SCENE_PUBLIC_INPUTS', version=2, status='NOT_ADMITTED',
                  source_revision='8f89d1905a86143d34ff77d2951fc60651ab2497', source_sha256='sha256:'+EXPECTED,
                  candidate_map=dict(map_id='mcr_reference_scene_r3', map_revision=2, geometry_hash=source['geometry_hash']),
                  tilemap=dict(path='world/reference-scene-v2/runtime/canonical-map-v2.json',
                               sha256='sha256:'+hashlib.sha256(encoded).hexdigest(), bytes=len(encoded), format='TILED_JSON'),
                  compatibility_texture=dict(width_px=768, height_px=1296, image_marker='mcr-theme.png',
                                             marker_policy='Inert logical tileset binding. Never fetch this marker.'),
                  parade_scenery=dict(path='world/reference-scene-v2/runtime-v4/parade-scenery.json',
                                      sha256='sha256:53d4b4cf6e580de724053cd1d6ff11b87ef77b56fa4d5399f294a63c0a4b4594',
                                      source_revision='e3af146ae4757794ca153363fd1408300ffde29e'),
                  authority='Static inputs only. Backend/root must admit actual versioned map/theme/avatar/operator bundle. Virtual coordinates never establish physical arrival.',
                  limitations=['PARADE-only painted source', 'Place provenance remains virtual/unreviewed', 'No production activation or release manifest is asserted'])
(HERE/'source-binding-v2.json').write_text(json.dumps(descriptor,indent=2)+'\n')
print(json.dumps(descriptor['tilemap']))
