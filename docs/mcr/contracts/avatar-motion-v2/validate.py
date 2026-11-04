#!/usr/bin/env python3
"""Independent shape, source, inventory, raster and refusal checks (not art review)."""
import copy
import hashlib
import json
from pathlib import Path

from jsonschema import Draft202012Validator
from PIL import Image

ROOT = Path(__file__).resolve().parents[4]

def no_duplicates(pairs):
    result = {}
    for key, value in pairs:
        assert key not in result, f'duplicate JSON key: {key}'
        result[key] = value
    return result

def read(path):
    return json.loads((ROOT / path).read_text(), object_pairs_hook=no_duplicates)

def main():
    config_schema = read('contracts/assets/v2/avatar-config.schema.json')
    manifest_schema = read('contracts/assets/v2/avatar-manifest.schema.json')
    for schema in [config_schema, manifest_schema]:
        Draft202012Validator.check_schema(schema)
    config_validator, manifest_validator = map(Draft202012Validator, [config_schema, manifest_schema])
    path = 'backend/releases/selected-character-motion-v2/manifest.json'
    m = read(path); manifest_validator.validate(m)
    fixture_root = ROOT / 'contracts/fixtures/assets/motion-v2'
    valid, invalid = list((fixture_root/'valid').glob('*.json')), list((fixture_root/'invalid').glob('*.json'))
    assert (len(valid), len(invalid)) == (15,16)
    for p in valid:
        config_validator.validate(json.loads(p.read_text(), object_pairs_hook=no_duplicates))
    for p in invalid:
        assert not config_validator.is_valid(json.loads(p.read_text(), object_pairs_hook=no_duplicates)), p
    resources = [m['atlas'], m['source']['descriptor'], *m['source']['resources'], *[x['resource'] for x in m['source']['originals']]]
    for resource in resources:
        data = (ROOT/resource['path']).read_bytes()
        assert (len(data), hashlib.sha256(data).hexdigest()) == (resource['bytes'], resource['sha256'])
        if 'width' in resource:
            with Image.open(ROOT/resource['path']) as image:
                image.load(); assert image.size == (resource['width'],resource['height'])
    d = read(m['source']['descriptor']['path'])
    assert d['productionEligible'] is m['production_eligible'] is False
    assert m['qualification'] == 'CANDIDATE_ONLY' and m['fallback'] == 'NONE'
    assert m['geometry']['cell'] == d['presets']['frameSize'] == [64,64]
    assert m['geometry']['foot'] == d['presets']['foot'] == [32,60]
    assert m['geometry']['output_cells'] == 20 and m['geometry']['physical_frames'] == 576
    assert m['geometry']['source_direction_aliases'] == dict(zip(['down','left','right','up'],['front','left','right','rear']))
    direction_names = ['down','left','right','up']
    for ordinal, p in enumerate(m['presets']):
        assert p['preset_id'] == f'motion_v2_photo01_front_{ordinal+1:02}'
        assert p['local_key'] == f'photo01-front-{ordinal+1:02}'
        assert p['source_frames'] == d['presets']['frames'][ordinal*20:(ordinal+1)*20]
        assert len(p['directions']) == 4
        for index, direction in enumerate(p['directions']):
            assert direction['direction'] == direction_names[index]
            assert direction['frames'] == list(range(ordinal*20+index*5, ordinal*20+index*5+5))
    counts = {'body':8,'eyes':4,'hair':6,'outfit':6,'accessory':4}
    assert {slot:sum(x['slot']==slot for x in m['items']) for slot in counts} == counts
    for index, item in enumerate(m['items']):
        source = d['wardrobe']['items'][index]
        assert item['source_pointer'] == f'/wardrobe/items/{index}'
        assert item['local_key'] == source['key']
        assert item['item_id'] == 'motion_v2_'+source['key'].replace('-','_')
        assert item['slot'] == source['kind']
        assert len(item['directions']) == 4
        if item['slot']=='body':
            assert item['skin_palette'] == source['skinPalette']
        if item['slot']=='outfit':
            assert item['cuff_policy'] == 'SEMANTIC_CUFF_FILL_REMOVED_AND_BODY_SKIN_RENDERED_BEHIND'
        for i, direction in enumerate(item['directions']):
            original = source['directions'][i]
            assert direction['direction'] == direction_names[i]
            assert direction['frames'] == original['frames']
            assert len(direction['frames']) == 5 and all(0<=x<576 for x in direction['frames'])
            if 'handFrames' in original:
                assert direction['hand_frames'] == original['handFrames']
                assert len(direction['hand_frames']) == 5 and all(0<=x<576 for x in direction['hand_frames'])
            if item['phase_motion']=='ARTICULATED_LIMBS':
                assert len(set(direction['frames'])) == 5
            else:
                assert len(set(direction['frames'])) == 1
            if item['slot']=='eyes' and i==3:
                assert direction['visibility'] == 'NON_VISIBLE_FROM_REAR'
    with Image.open(ROOT/m['atlas']['path']) as image:
        image.load(); assert image.mode == 'RGBA' and image.size == (1280,1856)
        for item in m['items']:
            if item['slot'] == 'eyes':
                index = item['directions'][3]['frames'][0]
                x,y=(index%20)*64,(index//20)*64
                assert image.crop((x,y,x+64,y+64)).getchannel('A').getbbox() is None
    # Schema is an exact immutable release: even apparently benign extensions fail.
    mutations = [('/production_eligible', True),('/qualification','ACCEPTED'),('/atlas/path','https://foreign.invalid/a.png'),
                 ('/atlas/width',64),('/atlas/sha256','0'*64),('/geometry/output_cells',1),
                 ('/geometry/foot/1',59),('/items/0/slot','hair'),('/items/0/directions/0/frames/0',576)]
    for pointer, value in mutations:
        changed=copy.deepcopy(m); parts=pointer.strip('/').split('/'); node=changed
        for part in parts[:-1]: node=node[int(part)] if isinstance(node,list) else node[part]
        key=int(parts[-1]) if isinstance(node,list) else parts[-1];node[key]=value
        assert not manifest_validator.is_valid(changed),pointer
    for raw in ['{"a":1,"a":1}', '{"item_ids":{"body":"a","body":"a"}}']:
        try: json.loads(raw,object_pairs_hook=no_duplicates)
        except AssertionError: pass
        else: raise AssertionError('duplicate field accepted')
    print(f'PASS: schemas; {len(valid)} accepted/{len(invalid)} refused fixtures; {len(resources)} pinned resource occurrences; 6 presets/28 items; direction/frame/palette/cuff/source/raster checks; 9 manifest tamper refusals. CANDIDATE_ONLY, production NOT ADMITTED.')

if __name__ == '__main__': main()
