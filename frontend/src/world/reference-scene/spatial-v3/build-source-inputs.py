#!/usr/bin/env python3
"""Project immutable artist metadata for Vite; no public JSON imports or source edits."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
BASE = HERE.parents[3] / 'public/world/reference-scene-v2'
pins = {}


def record(path, expected):
    raw = (BASE / path).read_bytes()
    assert 'sha256:' + hashlib.sha256(raw).hexdigest() == expected, path
    pins[path] = expected
    return json.loads(raw)


manifest = record('quality-v6/manifest.json',
                  'sha256:a78fbd045e9123e18ffa01efc669dfef9965bbeccb1cd378b886b9b975247aed')
source = record('spatial-v3/geometry-proposal.json', manifest['geometry']['sha256'])
record('spatial-v3/integer-projection.json', manifest['integer_projection']['sha256'])
scenes = {}
shared = None
for theme in manifest['themes']:
    docs = {}
    for file in theme['files']:
        path = 'quality-v6/' + file['path']
        raw = (BASE / path).read_bytes()
        assert len(raw) == file['bytes']
        assert 'sha256:' + hashlib.sha256(raw).hexdigest() == file['sha256']
        pins[path] = file['sha256']
        if path.endswith('.json'):
            docs[path.rsplit('/', 1)[-1]] = json.loads(raw)
    layout, meta = docs['layout.json'], docs['map-metadata.json']
    assert layout['geometry_hash'] == meta['geometry_hash'] == source['geometry_hash']
    assert meta['places'] == source['places']
    assert meta['map_labels'] == source['map_labels']
    assert meta['plaza_parcel'] == source['plaza_parcel']
    # Packed MAP placements are the supplied integer projection, not rerounded float proposals.
    assert meta['overview_representations'] == layout['overview_representations']
    if shared is not None:
        assert meta == shared
    shared = meta
    scenes[theme['theme_id']] = dict(scenery=docs['scenery.json'], frames=layout['frames'])
output = dict(source_revision='bc731ed868ec30b33ae5dd1e6f13e97e68ecabd9', inputs=pins,
              geometry_hash=source['geometry_hash'], geometry=source['geometry'], places=source['places'],
              map_labels=shared['map_labels'], overview_representations=shared['overview_representations'],
              plaza_parcel=shared['plaza_parcel'], world_camera=layout['world_camera'], scenes=scenes)
(HERE / 'source-inputs.json').write_text(json.dumps(output, separators=(',', ':')) + '\n')
binding = json.loads((BASE / 'runtime/spatial-v3/source-binding-v4.json').read_bytes())
tile = binding['tilemap']
raw_tile = (BASE.parents[1] / tile['path']).read_bytes()
assert len(raw_tile) == tile['bytes']
assert 'sha256:' + hashlib.sha256(raw_tile).hexdigest() == tile['sha256']
(HERE / 'tile-pin.json').write_text(json.dumps(tile, indent=2) + '\n')
print('Projected exact geometry, places, packed MAP placements and three theme-specific frame/scene tables')
