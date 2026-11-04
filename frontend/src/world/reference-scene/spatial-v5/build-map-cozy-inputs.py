#!/usr/bin/env python3
"""Close three committed era-specific environment tuples; write only two additive inputs."""
import argparse, hashlib, json, struct, subprocess
from pathlib import Path
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
PREFIX = 'frontend/public/world/reference-scene-v2/map-cozy-v1/'
def digest(raw): return 'sha256:' + hashlib.sha256(raw).hexdigest()
def pin(path):
    raw = (ROOT / path).read_bytes()
    return dict(path=path, bytes=len(raw), sha256=digest(raw))
def build(revision, handoff_sha):
    assert len(revision) == 40 and all(c in '0123456789abcdef' for c in revision)
    handoff_pin = pin(PREFIX + 'handoff.json')
    assert handoff_pin['sha256'] == handoff_sha
    handoff = json.loads((ROOT / handoff_pin['path']).read_bytes())
    assert handoff['format'] == 'MCR_COZY_SCENERY_HANDOFF_V1'
    assert handoff['status'] == 'CONTRACT_READY_SOURCE_VALIDATED_NATIVE_NOT_RUN'
    expected = {'README.md','build.py','prompt.txt','source-original.png','source-registration.json',
                'ground.svg','ground-provenance.json','presentation.json','parade/atlas.png','parade/ground.png',
                'revenge/atlas.png','danger/atlas.png','era-derivatives.json'}
    payloads = handoff['payloads']
    assert len(payloads) == len(expected) and {p['path'] for p in payloads} == {PREFIX+n for n in expected}
    for p in [handoff_pin, *payloads]:
        assert p == pin(p['path'])
        committed = subprocess.check_output(['git','show',revision+':'+p['path']], cwd=ROOT)
        assert digest(committed) == p['sha256'] and len(committed) == p['bytes']
    art = json.loads((ROOT / (PREFIX+'presentation.json')).read_bytes())
    base = json.loads((HERE/'source-inputs.json').read_bytes())
    arch = json.loads((HERE/'architecture-source.json').read_bytes())
    assert art['format']=='MCR_COZY_SCENERY_SOURCE_V1' and art['version']==1 and art['status']==handoff['status']
    assert art['geometry_hash']==base['geometry_hash']==arch['geometry_hash']
    eras=['PARADE','REVENGE','DANGER']
    assert art['scope']['eras']==eras and art['scope']['views']==['WORLD','MAP'] and art['scope']['other_eras']=='THREE_SCENERY_SOURCE_TUPLES_ONLY'
    assert set(art['eras'])==set(eras)
    assert art['retained_placements']==dict(world=34,map=51,labels=6,coordinates_depth_ids_unchanged=True)
    for p in art['immutable_inputs']: assert p==pin(p['path'])
    clean=lambda r,identity:dict({k:v for k,v in r.items() if k!='mime'},id=identity)
    scenes={}
    for era in eras:
        scene=art['eras'][era]
        assert scene['frame_rects']==base['scenes'][era]['frames']
        scenery=arch['scenes'][era]['scenery']
        ground,atlas=scene['ground'],scene['atlas']
        for resource,expected_path,old,dimensions in [(ground,'parade/ground.png',scenery['ground'],(768,1296)),(atlas,era.lower()+'/atlas.png',scenery['atlases'][0],(1024,1024))]:
            assert resource['path']=='world/reference-scene-v2/map-cozy-v1/'+expected_path and resource['mime']=='image/png'
            assert resource.get('id', old['id'])==old['id']
            raw=(ROOT/'frontend/public'/resource['path']).read_bytes()
            assert raw[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',raw[16:24])==dimensions
            assert (resource['width_px'],resource['height_px'])==dimensions
            assert resource['bytes']==len(raw) and resource['sha256']==digest(raw)
        assert ground['bytes']+atlas['bytes']<=2097152
        assert ground['width_px']*ground['height_px']+atlas['width_px']*atlas['height_px']==2043904
        scenes[era]=dict(ground=clean(ground,scenery['ground']['id']),atlas=clean(atlas,scenery['atlases'][0]['id']),frame_rects=scene['frame_rects'])
    assert art['ground']==art['eras']['PARADE']['ground'] and art['atlas']==art['eras']['PARADE']['atlas'] and art['frame_rects']==art['eras']['PARADE']['frame_rects']
    document=dict(format='MCR_SPATIAL5_MAP_COZY_V1_INPUTS',version=1,production_status='NOT_ADMITTED',
        geometry_hash=base['geometry_hash'],base_source_revision=base['source_revision'],eras=eras,scenes=scenes)
    encoded=json.dumps(document,separators=(',',':')).encode()
    frozen=['source-inputs.json','architecture-source.json','architecture-pin.json','map-service-source.json',
            'map-service-pins.json','map-labels-source.json','map-labels-pins.json','map-landscape-source.json','map-landscape-pin.json','tile-pin.json']
    source_path=str((HERE/'map-cozy-source.json').relative_to(ROOT))
    pins=dict(format='MCR_SPATIAL5_MAP_COZY_V1_PINS',source_revision=revision,production_status='NOT_ADMITTED',
        handoff=handoff_pin,outputs=payloads,frozen_inputs=[pin(str((HERE/n).relative_to(ROOT))) for n in frozen],
        document_sha256=digest(encoded),document=dict(path=source_path,bytes=len(encoded)+1,sha256=digest(encoded+b'\n')))
    (HERE/'map-cozy-source.json').write_bytes(encoded+b'\n')
    (HERE/'map-cozy-pins.json').write_text(json.dumps(pins,indent=2)+'\n')
    print(json.dumps(dict(result='PASS_SOURCE_ONLY_NOT_ADMITTED',eras=eras,world_objects=34,map_objects=51,png_bytes={era:s['ground']['bytes']+s['atlas']['bytes'] for era,s in scenes.items()})))
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--source-revision',required=True);parser.add_argument('--handoff-sha',required=True)
    args=parser.parse_args();build(args.source_revision,args.handoff_sha)
