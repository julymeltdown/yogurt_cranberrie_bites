#!/usr/bin/env python3
"""Close the additive semantic source and immutable scene lineage; emit only two new inputs."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
ART = ROOT / 'frontend/public/world/reference-scene-v2/map-service-labels-v1'
# Sealed producer handoff is set from its committed publication, never a mutable directory.
HANDOFF_SHA = 'sha256:33ea05993ff7f68bf71b653048d50de6b0ce806fe735c892576cbe4e9301ae86'
SOURCE_REVISION = 'b50177e745af331dcbc9d5755dcb2c04585b5cff'
PUBLISHED_REVISION = '58719fa05c51adda725c2a8260dbebfd4d09dab8'


def sha(raw):
    return 'sha256:' + hashlib.sha256(raw).hexdigest()


def raw_pin(path):
    raw = (ROOT / path).read_bytes()
    return dict(path=path, bytes=len(raw), sha256=sha(raw))


def build():
    handoff_path = str((ART / 'handoff.json').relative_to(ROOT))
    handoff = json.loads((ART / 'handoff.json').read_bytes())
    assert raw_pin(handoff_path)['sha256'] == HANDOFF_SHA
    for pin in handoff['payloads']:
        assert pin == raw_pin(pin['path']), pin['path']
    presentation = json.loads((ART / 'label-presentation.json').read_bytes())
    original = json.loads((HERE / 'source-inputs.json').read_bytes())
    labels = json.loads((HERE / 'map-labels-source.json').read_bytes())
    plants = json.loads((HERE / 'map-landscape-source.json').read_bytes())
    architecture = json.loads((HERE / 'architecture-pin.json').read_bytes())
    massing_path = 'frontend/public/world/reference-scene-v2/map-venue-massing-v1/presentation.json'
    massing = json.loads((ROOT / massing_path).read_bytes())
    assert presentation['geometry_hash'] == original['geometry_hash'] == massing['geometry_hash']
    assert presentation['production_status'] == 'NOT_ADMITTED' and presentation['view'] == 'MAP_ONLY'
    assert presentation['style'] == labels['style'] and presentation['font'] == labels['font']
    assert presentation['dimensions'] == labels['dimensions'] == dict(width=768, height=1296)
    assert len(presentation['labels']) == len(labels['labels']) == 6
    expected = [('MAIN STAGE', None, 'stage'), ('PLAZA', None, None), ('WALL', 'wall', 'wall'),
                ('FEED', 'feed', 'feed'), ('MEET', 'meet', 'people'), ('GIFTS', 'gifts', 'gift')]
    for new, old, (text, intent, icon) in zip(presentation['labels'], labels['labels'], expected):
        assert new['label'] == text and new['lines'] == [text] and new['navigation_intent'] == intent and new['icon'] == icon
        assert new['interaction_id'] is None and new['place_id'] is None and new['source_label'] == old['label']
        assert new['authority'] == ('VIRTUAL_FEATURE_NAVIGATION_ONLY' if intent else 'VIRTUAL_SCENERY_ONLY')
        assert all(new[k] == old[k] for k in ['x', 'y', 'rect'])
    required = ['required_base_labels', 'required_landscape', 'required_architecture', 'required_massing', 'supplied_reference']
    for key in required:
        assert presentation[key] == raw_pin(presentation[key]['path']), key
    assert presentation['required_massing'] == raw_pin(massing_path)
    assert presentation['required_architecture'] == architecture['handoff']
    assert len(plants['decorations']) == 10 and len(massing['annexes']) == 2
    for pin in architecture['outputs']:
        assert pin == raw_pin(pin['path'])
    # Producer baseline fields remain lineage. The effective consumer tuple is separately closed.
    assert massing['required_labels'] == presentation['required_base_labels']
    assert massing['required_landscape'] == presentation['required_landscape']
    frozen_names = ['source-inputs.json', 'tile-pin.json', 'backend-allocation.json', 'map-labels-source.json',
                    'map-labels-pins.json', 'map-landscape-source.json', 'map-landscape-pin.json',
                    'architecture-source.json', 'architecture-pin.json']
    frozen = [raw_pin(str((HERE / name).relative_to(ROOT))) for name in frozen_names]
    replacements = {}
    for era, scene in original['scenes'].items():
        old = next(o for o in scene['scenery']['objects'] if o['id'] == 'food_counters')
        placement = {k: old[k] for k in ['x', 'y', 'width', 'height', 'depth_y']}
        assert list(placement.values()) == [568, 611, 190, 164, 775]
        replacements[era] = dict(id='map_feed_seating_representation', replaces_prop_id=old['id'],
                                 frame=scene['frames']['plaza_furniture'], **placement,
                                 interaction_id=None, place_id=None, view='MAP_ONLY',
                                 authority='Unbound virtual illustration from existing pixels; not a World solid or physical service.')
    document = dict(format='MCR_SPATIAL5_MAP_SERVICE_V1_INPUTS', production_status='NOT_ADMITTED',
                    geometry_hash=original['geometry_hash'], base_source_revision=original['source_revision'],
                    presentation=presentation, annexes=massing['annexes'], display_replacements=replacements)
    encoded = json.dumps(document, ensure_ascii=False, separators=(',', ':')).encode()
    source_path = str((HERE / 'map-service-source.json').relative_to(ROOT))
    active = dict(labels=raw_pin(str((ART / 'label-presentation.json').relative_to(ROOT))),
                  landscape=presentation['required_landscape'], massing=presentation['required_massing'],
                  architecture=presentation['required_architecture'],
                  display_policy='Recorded consumer grant af588a30/9d1bec78: exact original placement, existing plaza_furniture frame; World unchanged.')
    pins = dict(format='MCR_SPATIAL5_MAP_SERVICE_V1_PINS', source_revision=SOURCE_REVISION,
                published_revision=PUBLISHED_REVISION, production_status='NOT_ADMITTED',
                handoff=raw_pin(handoff_path), outputs=handoff['payloads'], frozen_inputs=frozen,
                effective_consumer_tuple=active, document_sha256=sha(encoded),
                document=dict(path=source_path, bytes=len(encoded) + 1, sha256=sha(encoded + b'\n')))
    (HERE / 'map-service-source.json').write_bytes(encoded + b'\n')
    (HERE / 'map-service-pins.json').write_text(json.dumps(pins, indent=2) + '\n')
    print(json.dumps(dict(status='PASS_SOURCE_ONLY_NOT_ADMITTED', labels=6, intents=4, objects=51, changed_world_objects=0)))


if __name__ == '__main__':
    import sys
    assert not sys.argv[1:]
    build()
