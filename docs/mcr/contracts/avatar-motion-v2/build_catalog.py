#!/usr/bin/env python3
"""Derive the immutable motion-v2 candidate catalog; never edits source artwork."""
import argparse
import copy
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
ART = 'frontend/public/world/avatars/derived-v1/motion-candidates-v1/'
EVIDENCE = 'docs/mcr/art/evidence/motion-candidates-v1/'
RELEASE = 'backend/releases/selected-character-motion-v2/'
FIXTURES = 'contracts/fixtures/assets/motion-v2/'
MANIFEST_ID = 'mcr_selected_motion_20261103'
DIRECTIONS = ['down', 'left', 'right', 'up']
SOURCE_DIRECTIONS = ['front', 'left', 'right', 'rear']
COUNTS = {'body': 8, 'eyes': 4, 'hair': 6, 'outfit': 6, 'accessory': 4}

def load(path):
    return json.loads((ROOT / path).read_text())

def encoded(obj):
    return (json.dumps(obj, indent=2, ensure_ascii=True) + '\n').encode()

def resource(path):
    data = (ROOT / path).read_bytes()
    value = {'path': path, 'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}
    if path.endswith('.png'):
        assert data[:8] == b'\x89PNG\r\n\x1a\n'
        value['width'], value['height'] = struct.unpack('>II', data[16:24])
    return value

def ids(slot):
    return [f'motion_v2_{slot}_{i}' for i in range(COUNTS[slot])]

def preset_ids():
    return [f'motion_v2_photo01_front_{i:02}' for i in range(1, 7)]

def make():
    handoff = load(EVIDENCE + 'consumer-handoff.json')
    for pin in handoff['pins']:
        assert resource(pin['path']) | pin == resource(pin['path']), pin['path']
    pack_path = 'frontend/src/entities/avatar/character-source-pack.json'
    pack = load(pack_path)
    originals = []
    for entry in pack['files']:
        res = resource('frontend/public' + pack['base_url'] + entry['file'])
        assert (res['sha256'], res['bytes']) == (entry['sha256'], entry['byte_length'])
        originals.append({'source_id': entry['source_id'], 'resource': res})
    assert len(originals) == 20
    descriptor_path = ART + 'combined-release-candidate.json'
    candidate = load(descriptor_path)
    assert candidate['productionEligible'] is False
    assert candidate['wardrobe']['counts'] == COUNTS
    assert candidate['presets']['frameSize'] == candidate['wardrobe']['frameSize'] == [64, 64]
    assert candidate['presets']['foot'] == candidate['wardrobe']['foot'] == [32, 60]
    assert candidate['presets']['directions'] == candidate['wardrobe']['directions'] == SOURCE_DIRECTIONS
    components = load(EVIDENCE + 'component-source-inputs.json')
    authored = load(EVIDENCE + 'authored-reconstruction.json')
    # Preserve the source evidence verbatim as pinned resources, with explicit per-item pointers.
    resources = [resource(pin['path']) for pin in handoff['pins']]
    resources.append(resource(pack_path))
    for name in [entry['file'] for entry in components.values()] + ['authored-turnaround-source-9a40c5d4098296b0.png']:
        resources.append(resource(ART + name))
    for entry in components.values():
        assert resource(ART + entry['file'])['sha256'] == entry['sha256']
    atlas = resource(ART + candidate['atlas']['file'])
    assert (atlas['sha256'], atlas['bytes'], atlas['width'], atlas['height']) == (candidate['atlas']['sha256'], candidate['atlas']['bytes'], 1280, 1856)
    presets = []
    for ordinal, canonical in enumerate(preset_ids()):
        local = f'photo01-front-{ordinal + 1:02}'
        frames = [f for f in candidate['presets']['frames'] if f['preset'] == local]
        assert len(frames) == 20
        presets.append({'preset_id': canonical, 'local_key': local,
            'source_pointer': f'/presets/frames/{ordinal * 20}',
            'directions': [{'direction': direction, 'frames': [f['atlasIndex'] for f in frames if f['direction'] == SOURCE_DIRECTIONS[index]]}
                           for index, direction in enumerate(DIRECTIONS)],
            'source_frames': frames})
    items = []
    for index, entry in enumerate(candidate['wardrobe']['items']):
        slot = entry['kind']
        number = int(entry['key'].split('-')[1])
        directions = []
        for source in entry['directions']:
            direction = {'direction': DIRECTIONS[source['direction']], 'frames': source['frames']}
            if 'handFrames' in source:
                direction['hand_frames'] = source['handFrames']
            if 'visibility' in source:
                direction['visibility'] = source['visibility']
            directions.append(direction)
        source_ids = {'body': ['photo_02'], 'eyes': ['photo_09'], 'hair': ['photo_01', 'photo_04', 'photo_05'],
                      'outfit': ['photo_01', 'photo_06'], 'accessory': ['photo_19']}[slot]
        item = {'item_id': f'motion_v2_{slot}_{number}', 'slot': slot, 'local_key': entry['key'],
                'source_pointer': f'/wardrobe/items/{index}', 'selected_source_ids': source_ids,
                'attribution': 'SUPPLIED_EYE_CUTOUTS' if slot == 'eyes' else components[slot]['method'],
                'phase_motion': entry['phaseMotion'], 'directions': directions}
        if slot == 'eyes':
            item['source_location'] = entry['sourceLocation']
        else:
            item['authored_resource'] = ART + components[slot]['file']
        if 'sourceRow' in entry:
            item['source_row'] = entry['sourceRow']
        if 'skinPalette' in entry:
            item['skin_palette'] = entry['skinPalette']
        if 'cuffPolicy' in entry:
            item['cuff_policy'] = entry['cuffPolicy']
        items.append(item)
    manifest = {
        'contract_version': '2.0.0', 'kind': 'avatar_manifest', 'manifest_id': MANIFEST_ID,
        'manifest_version': 1, 'representation': 'selected_source_motion',
        'production_eligible': False, 'qualification': 'CANDIDATE_ONLY', 'fallback': 'NONE',
        'source': {'pack_id': pack['source_pack_id'], 'revision': pack['revision'],
                   'descriptor': resource(descriptor_path), 'originals': originals,
                   'art_source_revision': 'bb7d21b3', 'art_receipt_revision': handoff['publishedArtReceipt'],
                   'resources': resources},
        'atlas': atlas,
        'geometry': {'cell': [64, 64], 'foot': [32, 60], 'directions': DIRECTIONS,
                     'source_direction_aliases': dict(zip(DIRECTIONS, SOURCE_DIRECTIONS)),
                     'phase_order': ['idle', 'walk-0', 'walk-1', 'walk-2', 'walk-3'],
                     'output_cells': 20, 'physical_frames': candidate['physicalFrames']},
        'composition': {'layer_order': ['body', 'hands', 'outfit', 'eyes', 'hair', 'accessory'],
                        'dressed_body_clip_y_exclusive': 42, 'hand_palette_color_index': 2,
                        'skin_policy': 'BODY_SELECTS_SEMANTIC_RGB_PALETTE_NO_BLANKET_TINT',
                        'rear_eyes': 'NON_VISIBLE_FROM_REAR', 'stationary_uv_sharing': True,
                        'combined_indices_already_offset': True},
        'presets': presets, 'items': items}
    header = {'contract_version': {'const': '2.0.0'}, 'kind': {'const': 'avatar_config'},
              'manifest_id': {'const': MANIFEST_ID}, 'manifest_version': {'const': 1}}
    def closed(properties):
        return {'type': 'object', 'additionalProperties': False, 'required': list(properties), 'properties': properties}
    slots = {slot: {'enum': ids(slot) + ([None] if slot in ('outfit', 'accessory') else [])} for slot in COUNTS}
    schema = {'$schema': 'https://json-schema.org/draft/2020-12/schema',
              '$id': 'https://mcr.example/contracts/assets/v2/avatar-config.schema.json',
              'title': 'MCR selected-source motion AvatarConfig 2.0.0', 'oneOf': [
                closed(header | {'representation': {'const': 'motion_preset'}, 'preset_id': {'enum': preset_ids()}}),
                closed(header | {'representation': {'const': 'motion_layered'}, 'item_ids': closed(slots)})]}
    # Manifest version 1 is an immutable release. Exact equality closes every nested mapping,
    # source pin, palette, nullable field and rendering semantic without a permissive extension.
    manifest_schema = {'$schema': schema['$schema'], '$id': 'https://mcr.example/contracts/assets/v2/avatar-manifest.schema.json',
        'title': 'Immutable selected-source motion manifest 2.0.0 release 1', 'type': 'object', 'const': manifest}
    header_value = {'contract_version': '2.0.0', 'kind': 'avatar_config', 'manifest_id': MANIFEST_ID, 'manifest_version': 1}
    fixtures = {f'preset-{i+1:02}': header_value | {'representation': 'motion_preset', 'preset_id': v} for i, v in enumerate(preset_ids())}
    for i in range(8):
        fixtures[f'layered-body-{i}'] = header_value | {'representation': 'motion_layered', 'item_ids': {
            'body': ids('body')[i], 'eyes': ids('eyes')[i % 4], 'hair': ids('hair')[i % 6],
            'outfit': ids('outfit')[i % 6] if i % 2 == 0 else None,
            'accessory': ids('accessory')[i % 4] if i % 2 == 0 else None}}
    fixtures['layered-max'] = header_value | {'representation': 'motion_layered', 'item_ids': {s: ids(s)[-1] for s in COUNTS}}
    bad = {}
    base = fixtures['layered-body-0']
    def invalid(name, change):
        v = copy.deepcopy(base); change(v); bad[name] = v
    invalid('wrong-version', lambda v: v.update(contract_version='1.1.0'))
    invalid('wrong-kind', lambda v: v.update(kind='avatar_manifest'))
    invalid('unknown-manifest', lambda v: v.update(manifest_id='other'))
    invalid('wrong-revision', lambda v: v.update(manifest_version=2))
    invalid('mixed-representation', lambda v: v.update(preset_id=preset_ids()[0]))
    invalid('url', lambda v: v.update(url='https://untrusted.invalid/sheet.png'))
    invalid('skin-tone', lambda v: v['item_ids'].update(skin_tone_id='tone_0'))
    invalid('body-overflow', lambda v: v['item_ids'].update(body='motion_v2_body_8'))
    invalid('wrong-slot', lambda v: v['item_ids'].update(eyes='motion_v2_hair_0'))
    invalid('null-body', lambda v: v['item_ids'].update(body=None))
    invalid('missing-outfit', lambda v: v['item_ids'].pop('outfit'))
    invalid('legacy-id', lambda v: v['item_ids'].update(body='body_0'))
    invalid('float-revision', lambda v: v.update(manifest_version=1.5))
    invalid('unknown-nested', lambda v: v['item_ids'].update(sheet_id='any'))
    bad['unknown-preset'] = fixtures['preset-01'] | {'preset_id': 'motion_v2_photo01_front_07'}
    bad['preset-mixed-items'] = fixtures['preset-01'] | {'item_ids': base['item_ids']}
    outputs = {RELEASE + 'manifest.json': manifest, 'contracts/assets/v2/avatar-config.schema.json': schema,
               'contracts/assets/v2/avatar-manifest.schema.json': manifest_schema}
    outputs |= {FIXTURES + 'valid/' + key + '.json': value for key, value in fixtures.items()}
    outputs |= {FIXTURES + 'invalid/' + key + '.json': value for key, value in bad.items()}
    return outputs

def main():
    parser = argparse.ArgumentParser(); parser.add_argument('--check', action='store_true'); args = parser.parse_args()
    outputs = make()
    for path, obj in outputs.items():
        data = encoded(obj); target = ROOT / path
        if args.check:
            assert target.read_bytes() == data, f'drift: {path}'
        else:
            target.parent.mkdir(parents=True, exist_ok=True); target.write_bytes(data)
    print(f'{"CHECK" if args.check else "WRITE"} PASS: {len(outputs)} deterministic catalog/schema/config files; 20 originals and 10 handoff pins verified')

if __name__ == '__main__':
    main()
