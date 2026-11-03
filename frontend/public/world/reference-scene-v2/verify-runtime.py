#!/usr/bin/env python3
"""Validate the versioned compact resources and artist-owned C06 projection."""
import hashlib
import json
import math
from pathlib import Path
from PIL import Image, ImageChops
from jsonschema import Draft202012Validator

HERE=Path(__file__).resolve().parent
OUT=HERE/'runtime-v4'
read=lambda p:json.loads(p.read_text())
sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
nearest=lambda v:math.floor(v+.5) if v>=0 else math.ceil(v-.5)
source=read(HERE/'painted/layout.json');layout=read(OUT/'parade-layout.json')
manifest=read(OUT/'parade-resource-inputs.json');scenery=read(OUT/'parade-scenery.json')
projection=read(OUT/'parade-integer-projection.json');proposal=read(HERE/'geometry-proposal.json')
api=read(HERE.parents[3]/'backend/docs/api/mcr-core.openapi.json')
Draft202012Validator({'$ref':'#/components/schemas/Scenery','components':api['components']}).validate(scenery)
assert layout['geometry_hash']==source['geometry_hash']==proposal['geometry_hash']
assert ImageChops.difference(Image.open(HERE/'painted/parade-ground.png').convert('RGB'),Image.open(OUT/'parade-ground.png').convert('RGB')).getbbox() is None
for r in manifest['sources']:
    assert sha(OUT/r['path'])==r['sha256']
for r in manifest['resources']:
    p=OUT/r['path'];image=Image.open(p)
    assert sha(p)==r['sha256'] and p.stat().st_size==r['bytes']
    assert image.size==(r['width'],r['height']) and r['pixels']==image.width*image.height
assert manifest['total_pixels']==sum(r['pixels'] for r in manifest['resources'])<=2097152
assert manifest['total_bytes']==sum(r['bytes'] for r in manifest['resources'])<=2097152
assert manifest['budget_pass']
original_atlas=Image.open(HERE/'painted'/source['atlas']).convert('RGBA');atlas=Image.open(OUT/layout['atlas'])
for t in manifest['transforms']:
    a=t['source_rect'];b=t['packed_rect']
    expected=original_atlas.crop((a['x'],a['y'],a['x']+a['width'],a['y']+a['height'])).resize((b['width'],b['height']),Image.Resampling.NEAREST)
    actual=atlas.crop((b['x'],b['y'],b['x']+b['width'],b['y']+b['height']))
    assert ImageChops.difference(expected,actual).getbbox() is None
    assert b['width']==nearest(a['width']*.75) and b['height']==nearest(a['height']*.75)
    assert all(0<=b[k] for k in ['x','y']) and b['x']+b['width']<=1024 and b['y']+b['height']<=1024
    for other in manifest['transforms']:
        if other is t:continue
        c=other['packed_rect']
        assert b['x']+b['width']<=c['x'] or c['x']+c['width']<=b['x'] or b['y']+b['height']<=c['y'] or c['y']+c['height']<=b['y']
for p in projection['objects']:
    obj=layout[p['group']][p['index']];old=source[p['group']][p['index']]
    assert obj['id']==old['id']==p['id']
    for k,v in p['runtime'].items():assert type(v) is int and obj[k]==v
    assert abs(p['delta']['depth_y'])<=.5 and abs(p['foot_anchor_delta'])<=.5 and abs(p['delta']['y'])<=1
    assert obj['y']==obj['depth_y']-nearest(old['depth_y']-old['y'])
    for k in set(old)-set(p['runtime']):assert obj[k]==old[k],(obj['id'],k)
assert len(scenery['objects'])==22 and len(layout['overview_representations'])==6
assert [o['id'] for o in scenery['objects']]==[o['id'] for o in source['placements']]
for o,p in zip(scenery['objects'],layout['placements']):
    assert o['frame']==layout['frames'][p['frame']]
    assert all(o[k]==p[k] for k in ['x','y','width','height','depth_y'])
    assert p['footprint']==next(old for old in proposal['placements'] if old['id']==p['id'])['footprint']
for r in layout['overview_representations']:assert 'footprint' not in r and r['id'] not in {o['id'] for o in scenery['objects']}
# No C06 frame/depth projection may invert authored physical-object ordering.
assert [o['id'] for o in sorted(source['placements'],key=lambda o:o['depth_y'])]==[o['id'] for o in sorted(layout['placements'],key=lambda o:o['depth_y'])]
report={'status':'PASS_COMPACT_C06_ART_INPUTS_ONLY','geometry_hash':layout['geometry_hash'],
    'pixels':manifest['total_pixels'],'bytes':manifest['total_bytes'],'closed_C06_schema':'PASS',
    'physical_objects':22,'map_only_representations':6,'ground_pixels_identical':True,
    'max_depth_delta':max(abs(p['delta']['depth_y']) for p in projection['objects']),
    'max_y_delta':max(abs(p['delta']['y']) for p in projection['objects']),
    'max_local_foot_delta':max(abs(p['foot_anchor_delta']) for p in projection['objects']),
    'stable_depth_order':True,'original_sources_unchanged':True,
    'limitations':['PARADE proposal only; REVENGE/DANGER painted skins unavailable.','Not release admission, application/multiuser proof, moving-avatar occlusion or target acceptance.']}
(OUT/'verification.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
