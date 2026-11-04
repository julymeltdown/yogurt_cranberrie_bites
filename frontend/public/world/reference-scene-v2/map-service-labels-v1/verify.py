#!/usr/bin/env python3
"""Read-only payload/authority boundary verification for the frozen annotation."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
read = lambda p: json.loads(p.read_text())
digest = lambda b: 'sha256:'+hashlib.sha256(b).hexdigest()

def check_pin(pin):
    path = (ROOT/pin['path']).resolve()
    assert path.is_relative_to(ROOT)
    blob = path.read_bytes()
    assert len(blob) == pin['bytes'] and digest(blob) == pin['sha256'], pin['path']

handoff = read(HERE/'handoff.json')
for pin in handoff['payloads']:
    check_pin(pin)
check_pin(handoff['labels'])
labels = read(HERE/'label-presentation.json')
for key in ['required_base_labels', 'required_landscape', 'required_architecture',
            'required_massing', 'supplied_reference', 'font', 'font_license']:
    check_pin(labels[key])
base = read(ROOT/labels['required_base_labels']['path'])
assert labels['style'] == base['style'] and labels['font'] == base['font']
expected = [('MAIN STAGE', None), ('PLAZA', None), ('WALL', 'wall'),
            ('FEED', 'feed'), ('MEET', 'meet'), ('GIFTS', 'gifts')]
assert [(l['label'], l['navigation_intent']) for l in labels['labels']] == expected
for old, new in zip(base['labels'], labels['labels']):
    assert new['source_label'] == old['label']
    assert [new[k] for k in ['rect', 'x', 'y']] == [old[k] for k in ['rect', 'x', 'y']]
    assert new['interaction_id'] is None and new['place_id'] is None
    assert new['authority'] == ('VIRTUAL_SCENERY_ONLY' if new['navigation_intent'] is None else 'VIRTUAL_FEATURE_NAVIGATION_ONLY')
    assert 'action' not in new and 'path' not in new and 'id' not in new
assert 'food' not in labels['icons'] and 'merch' not in labels['icons']
assert all(key in labels['icons'] for key in ['stage', 'people', 'gift', 'feed', 'wall'])
assert labels['production_status'] == 'NOT_ADMITTED' and labels['view'] == 'MAP_ONLY'
assert not any((HERE/name).exists() for name in ['landscape-presentation.json',
               'venue-massing-presentation.json', 'display-replacements.json'])
print(json.dumps({'closed_payloads': len(handoff['payloads']), 'labels': 6,
                  'closed_intents': 4, 'null_canonical_bindings': 6,
                  'status': 'PASS_SOURCE_ONLY', 'native_actions': 'NOT_RUN'}))
