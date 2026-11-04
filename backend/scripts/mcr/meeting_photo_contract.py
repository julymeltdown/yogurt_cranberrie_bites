#!/usr/bin/env python3
"""Offline producer/consumer photo contract checks, never live-media qualification."""
from pathlib import Path
import copy
import importlib.util
import json
ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location('mcr_photo_contract', ROOT/'scripts/mcr/validate-domain.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
contract = module.Contract()
fixture = json.loads((ROOT/'backend/services/concert-service/tests/meeting_photo_support/contract-fixtures.json').read_text())
count = 0
def check(value, schema, accepted=True):
    global count
    try:
        contract.validate(value, schema)
        actual=True
    except module.ContractError:
        actual=False
    assert actual == accepted, f'{schema}: expected {accepted}'
    count+=1
for name,key in [('CreateBeacon','create'),('UpdateBeacon','update'),('MeetBeacon','read')]:
    check(fixture[key], f'beacon.{name}')
check(fixture['media'], 'feed.Media')
check(dict(purpose='BEACON',content_type='image/png',byte_length=68,sha256='0'*64),'feed.InitUpload')
for name,key in [('CreateBeacon','create'),('UpdateBeacon','update')]:
    b=copy.deepcopy(fixture[key]);b.pop('photo_permission_acknowledged')
    check(b,f'beacon.{name}',False)
    b['photo_permission_acknowledged']=False
    check(b,f'beacon.{name}',False)
    b['meeting']['media_id']=None
    check(b,f'beacon.{name}')
    del b['meeting']['media_id'];del b['photo_permission_acknowledged']
    check(b,f'beacon.{name}')
for value in ['GIVEAWAY_AS_BEACON','beacon','https://example.invalid/image.png']:
    b=copy.deepcopy(fixture['media']);b['purpose']=value
    check(b,'feed.Media',False)
b=copy.deepcopy(fixture['create']);b['meeting']['photo_url']='https://example.invalid/image.png'
check(b,'beacon.CreateBeacon',False)
assert fixture['synthetic'] and fixture['production_rights']=='NOT_ESTABLISHED'
assert fixture['contract']=='meeting-photo/v1'
assert fixture['create']['meeting']==fixture['read']['meeting']
assert fixture['media']['media_id']==fixture['create']['meeting']['media_id']==fixture['media_id']
assert fixture['media']['read_path']==f"/api/concert/events/{fixture['event_id']}/media/{fixture['media_id']}/content"
print(json.dumps(dict(result='PASS',cases=count,scope='offline schema/fixture agreement; live/native NOT_RUN')))
