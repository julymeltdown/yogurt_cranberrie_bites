#!/usr/bin/env python3
"""Verify and copy a separate MAP-only source sidecar without changing the World tuple."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
BASE = ROOT / 'frontend/public/world/reference-scene-v2/map-landscape-v1'
sha = lambda raw: 'sha256:' + hashlib.sha256(raw).hexdigest()


def build():
    raw = (BASE / 'handoff.json').read_bytes()
    assert len(raw) == 11496 and sha(raw) == 'sha256:a753907e1e93b9565de487f8164c832eec566c2aa47cfe529b12b785be787f0b'
    handoff = json.loads(raw)
    for pin in handoff['files']:
        value = (ROOT / pin['path']).read_bytes()
        assert len(value) == pin['bytes'] and sha(value) == pin['sha256'], pin['path']
    raw = (BASE / 'presentation.json').read_bytes()
    assert len(raw) == 12497 and sha(raw) == 'sha256:90402525366769febb1f8d5b779d9b293e4680858f105c88af4bf808965cc35e'
    presentation = json.loads(raw)
    source = json.loads((HERE / 'source-inputs.json').read_bytes())
    assert presentation['geometry_hash'] == source['geometry_hash'] and presentation['view'] == 'MAP_ONLY'
    assert presentation['dimensions'] == dict(width=768, height=1296)
    assert len(presentation['decorations']) == 10
    ids = set()
    for item in presentation['decorations']:
        assert item['id'].startswith('map_garden_') and item['id'] not in ids
        ids.add(item['id'])
        assert item['interaction_id'] is None and item['view'] == 'MAP_ONLY' and item['frame'] == 'shrub_bed'
    for pin in presentation['frozen_inputs']:
        value = (ROOT / pin['path']).read_bytes()
        assert len(value) == pin['bytes'] and sha(value) == pin['sha256'], pin['path']
    for theme in presentation['themes']:
        scenery = source['scenes'][theme['theme_id']]['scenery']
        assert theme['geometry_hash'] == source['geometry_hash']
        assert theme['atlas'] == scenery['atlases'][0]
        assert theme['frame'] == source['scenes'][theme['theme_id']]['frames']['shrub_bed']
    (HERE / 'map-landscape-source.json').write_bytes(raw)
    pin = dict(source_revision='f19027fa96abd90eb11ffda825deda95c3091647', path='world/reference-scene-v2/map-landscape-v1/presentation.json',
               sha256=sha(raw), bytes=len(raw), geometry_hash=source['geometry_hash'],
               authority='MAP-only decoration, no additional PNG/font/World solids/place/interaction/admission.')
    (HERE / 'map-landscape-pin.json').write_text(json.dumps(pin, indent=2) + '\n')
    print(json.dumps(dict(status='PASS_MAP_ONLY_INPUTS', closed_pins=len(handoff['files']), decorations=10)))


if __name__ == '__main__':
    build()
