#!/usr/bin/env python3
"""Close published architecture art and emit only supplemental consumer inputs."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
PUBLIC = ROOT / 'frontend/public'
BASE = PUBLIC / 'world/reference-scene-v2'
REVISION = '585baf9fae121ed68b26ec886221c0171e28b799'
PUBLISHED = '921cb3d850925a489302ebcc5483fba71ac835a5'
HANDOFF = 'frontend/public/world/reference-scene-v2/architecture-v2/atlas-handoff.json'
HANDOFF_SHA = 'sha256:98484ffd7c3301f7ba3b8cc5221be29d48386ec1dcc2eb67015c2dffc76a5fc0'
FROZEN = ['source-inputs.json', 'tile-pin.json', 'backend-allocation.json', 'map-labels-source.json',
          'map-labels-pins.json', 'map-landscape-source.json', 'map-landscape-pin.json']


def sha(raw):
    return 'sha256:' + hashlib.sha256(raw).hexdigest()


def pin(path):
    raw = (ROOT / path).read_bytes()
    return dict(path=path, bytes=len(raw), sha256=sha(raw))


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, separators=(',', ':')) + '\n').encode()


def build():
    handoff_pin = pin(HANDOFF)
    assert handoff_pin['bytes'] == 8302 and handoff_pin['sha256'] == HANDOFF_SHA
    handoff = json.loads((ROOT / HANDOFF).read_bytes())
    outputs = handoff['outputs']
    assert len(outputs) == len({p['path'] for p in outputs}) == 18
    assert sum(p['bytes'] for p in outputs) == handoff['derivative_bytes'] == 7921243
    for item in outputs:
        assert item == pin(item['path']), item['path']
    source = json.loads((HERE / 'source-inputs.json').read_bytes())
    assert handoff['geometry_hash'] == source['geometry_hash']
    scenes, resources = {}, {}
    for era in ['revenge', 'parade', 'danger']:
        folder = BASE / 'architecture-v2' / era
        old = source['scenes'][era.upper()]
        layout = json.loads((folder / 'layout.json').read_bytes())
        old_layout = json.loads((BASE / 'quality-v10' / era / 'layout.json').read_bytes())
        # Art changes only resource provenance, never the packing/placement contract.
        for field in handoff['themes'][['revenge', 'parade', 'danger'].index(era)]['frozen_layout_fields']:
            assert layout[field] == old_layout[field], (era, field)
        assert layout['frames'] == old['frames']
        scenery = json.loads((folder / 'scenery.json').read_bytes())
        entries = json.loads((folder / 'resource-inputs.json').read_bytes())
        atlas = scenery['atlases'][0]
        assert len(scenery['atlases']) == 1 and len(scenery['objects']) == 34
        assert atlas['id'] == era + '_architecture_v2_atlas'
        assert atlas['path'] == 'world/reference-scene-v2/architecture-v2/' + era + '/atlas.png'
        assert atlas['width_px'] == atlas['height_px'] == 1024
        assert pin('frontend/public/' + atlas['path']) == {k: ('frontend/public/' + atlas[k] if k == 'path' else atlas[k]) for k in ['path', 'bytes', 'sha256']}
        assert scenery['ground'] == old['scenery']['ground']
        normalized = json.loads(json.dumps(scenery))
        normalized['atlases'] = old['scenery']['atlases']
        for obj in normalized['objects']:
            assert obj['atlas_id'] == atlas['id']
            obj['atlas_id'] = old['scenery']['atlases'][0]['id']
        assert normalized == old['scenery'], era
        rows = [scenery['ground'], atlas]
        assert entries['resources'] == rows
        assert sum(r['bytes'] for r in rows) == entries['total_bytes'] <= 2097152
        assert sum(r['width_px'] * r['height_px'] for r in rows) == entries['total_pixels'] == 2043904
        for row in rows:
            raw = (PUBLIC / row['path']).read_bytes()
            assert len(raw) == row['bytes'] and sha(raw) == row['sha256']
        scenes[era.upper()] = dict(scenery=scenery)
        resources[era.upper()] = atlas
    frozen = [pin(str((HERE / p).relative_to(ROOT))) for p in FROZEN]
    frozen += [pin('frontend/public/world/reference-scene-v2/runtime/spatial-v5/' + p) for p in ['canonical-map.json', 'source-binding.json']]
    source_output = dict(format='MCR_SPATIAL5_ARCHITECTURE_V2_INPUTS', version=1, production_status='NOT_ADMITTED',
                         geometry_hash=source['geometry_hash'], base_source_revision=source['source_revision'], scenes=scenes)
    raw = encoded(source_output)
    source_pin = dict(path='frontend/src/world/reference-scene/spatial-v5/architecture-source.json', bytes=len(raw), sha256=sha(raw))
    pins = dict(format='MCR_SPATIAL5_ARCHITECTURE_V2_PINS', version=1, source_revision=REVISION, published_revision=PUBLISHED,
                production_status='NOT_ADMITTED', handoff=handoff_pin, outputs=outputs, frozen_inputs=frozen,
                document=source_pin, runtime_atlases=resources)
    binding = dict(format='MCR_SPATIAL5_ARCHITECTURE_V2_BINDING', version=1, status='NOT_RELEASE_BOUND', production_status='NOT_ADMITTED',
                   published_revision=PUBLISHED, geometry_hash=source['geometry_hash'], handoff=handoff_pin, document=source_pin,
                   frozen_inputs=frozen, scenes=[dict(theme_id=k, runtime_resources=[v['scenery']['ground'], *v['scenery']['atlases']]) for k, v in scenes.items()],
                   authority='Supplemental source identity only. No backend reservation, release admission, REVIEWED provenance, physical destination or live qualification.')
    target = BASE / 'runtime/architecture-v2/source-binding.json'
    target.parent.mkdir(parents=True, exist_ok=True)
    (HERE / 'architecture-source.json').write_bytes(raw)
    (HERE / 'architecture-pin.json').write_text(json.dumps(pins, indent=2) + '\n')
    target.write_text(json.dumps(binding, indent=2) + '\n')
    print(json.dumps(dict(status='PASS_SOURCE_ONLY', closed_outputs=18, frozen_inputs=len(frozen), themes=3)))


if __name__ == '__main__':
    import sys
    assert not sys.argv[1:]
    build()
