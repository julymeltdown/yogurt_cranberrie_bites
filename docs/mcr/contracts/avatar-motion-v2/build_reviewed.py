#!/usr/bin/env python3
"""Build a distinct reviewed successor. Never edits the frozen candidate/art."""
import copy
import hashlib
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parents[4]
CANDIDATE='backend/releases/selected-character-motion-v2/manifest.json'
REVIEW='docs/mcr/contracts/avatar-motion-v2/review/visual-review.json'
OUTPUT='backend/releases/selected-character-motion-v2/reviewed/manifest.json'
ID='mcr_selected_motion_reviewed_20261104'
VERSION=2

def canonical(value):
    def check(x):
        assert type(x) in (dict,list,str,int,bool,type(None)),'floats/unknown values forbidden'
        if isinstance(x,str):x.encode('utf-8',errors='strict')
        elif isinstance(x,dict):
            assert all(isinstance(k,str) for k in x)
            for k,v in x.items():check(k);check(v)
        elif isinstance(x,list):
            for v in x:check(v)
    check(value)
    return json.dumps(value,ensure_ascii=True,sort_keys=True,separators=(',',':'),allow_nan=False).encode('ascii')

def read(p):return json.loads((ROOT/p).read_text())
def pin(p):
    b=(ROOT/p).read_bytes();return {'path':p,'sha256':hashlib.sha256(b).hexdigest(),'bytes':len(b)}
def write(p,value):
    p=ROOT/p;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(value,indent=2)+'\n')
def config(c):
    c=copy.deepcopy(c);c['manifest_id']=ID;c['manifest_version']=VERSION;return c

def main():
    c=read(CANDIDATE)
    assert pin(CANDIDATE)['sha256']=='9a555f726a9e421f14ed876117be87bb1bb8fa128ac401ab38f8927760776fd6'
    r=read(REVIEW);assert r['decision']=='VISUAL_ACCEPTED'
    assert r['candidate_manifest']==pin(CANDIDATE) and r['atlas_sha256']==c['atlas']['sha256']
    for p in r['inspected_resources']:
        assert pin(p['path'])==p
    assert read(r['additional_seam_report']['path'])['result']=='PASS'
    assert pin(r['additional_seam_report']['path'])==r['additional_seam_report']
    m=copy.deepcopy(c);m.update(manifest_id=ID,manifest_version=VERSION,
      representation='reviewed_selected_source_motion',qualification='VISUALLY_REVIEWED',production_eligible=True,
      candidate_manifest=pin(CANDIDATE),review=pin(REVIEW),default_preset_id=c['presets'][0]['preset_id'])
    for p in m['presets']:
        p['config']={'contract_version':'2.0.0','kind':'avatar_config','manifest_id':ID,'manifest_version':VERSION,
          'representation':'motion_preset','preset_id':p['preset_id']}
    # Runtime qualification is separate execution evidence. This staged manifest
    # is not a deployment/admission decision until actual typed/runtime gates pass.
    m['content_hash']='sha256:'+hashlib.sha256(canonical(m)).hexdigest()
    write(OUTPUT,m)
    schema=read('contracts/assets/v2/avatar-config.schema.json')
    defs=copy.deepcopy(schema['oneOf'])
    for s in defs:
        s['properties']['manifest_id']={'const':ID};s['properties']['manifest_version']={'const':VERSION}
    base='https://contracts.mcr.invalid/assets/v2/reviewed/'
    write('contracts/assets/v2/reviewed/avatar-config.schema.json',{'$schema':schema['$schema'],
      '$id':base+'avatar-config.schema.json','$ref':'#/$defs/ReviewedMotionAvatarConfig',
      '$defs':{'ReviewedMotionAvatarConfig':{'oneOf':defs}}})
    write('contracts/assets/v2/reviewed/avatar-manifest.schema.json',{'$schema':schema['$schema'],
      '$id':base+'avatar-manifest.schema.json','$ref':'#/$defs/ReviewedMotionAvatarManifest',
      '$defs':{'ReviewedMotionAvatarManifest':{'type':'object','const':m}}})
    source=ROOT/'contracts/fixtures/assets/motion-v2'
    for kind in ['valid','invalid']:
        for p in sorted((source/kind).glob('*.json')):
            original=json.loads(p.read_text());value=config(original)
            # Preserve the actual negative: convert only the known candidate
            # header to successor values, not wrong manifest/version fixtures.
            for key,known in [('manifest_id',c['manifest_id']),('manifest_version',c['manifest_version'])]:
                if original.get(key)!=known:value[key]=original.get(key)
            if p.stem=='wrong-revision':value['manifest_version']=3
            write('contracts/fixtures/assets/motion-v2/reviewed/'+kind+'/'+p.name,value)
    print(json.dumps({'result':'BUILT_PENDING_RUNTIME_QUALIFICATION','manifest_id':ID,'manifest_version':VERSION,
      'manifest':pin(OUTPUT),'content_hash':m['content_hash'],'review':pin(REVIEW)},indent=2))
if __name__=='__main__':main()
