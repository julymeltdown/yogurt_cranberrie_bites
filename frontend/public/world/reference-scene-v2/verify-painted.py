#!/usr/bin/env python3
"""Check immutable source bytes, layer masks and the painted adoption boundary."""
import hashlib
import json
from pathlib import Path
from PIL import Image

HERE=Path(__file__).resolve().parent
OUT=HERE/'painted'
read=lambda name:json.loads((OUT/name).read_text())
sha=lambda file:'sha256:'+hashlib.sha256(file.read_bytes()).hexdigest()
p=json.loads((HERE/'geometry-proposal.json').read_text());layout=read('layout.json');provenance=read('provenance.json')
assert layout['geometry_hash']==p['geometry_hash'] and layout['status']=='PROPOSAL_NOT_RELEASE_BOUND'
for source in [provenance['source_atlas'],provenance['source_surfaces']]+provenance['previous_sources_preserved']:
    assert sha(OUT/source['path'])==source['sha256']
for entry in read('resource-inputs.json')['resources']:
    file=OUT/entry['path'];assert sha(file)==entry['sha256'] and file.stat().st_size==entry['bytes'],entry['path']
atlas=Image.open(OUT/layout['atlas']);assert atlas.mode=='RGBA' and atlas.size==tuple(layout['source_atlas_dimensions'])
assert atlas.getchannel('A').getextrema()==(0,255)
for f in layout['frames'].values():
    assert 0<=f['x'] and 0<=f['y'] and f['x']+f['width']<=atlas.width and f['y']+f['height']<=atlas.height
for o in layout['placements']:
    assert 0<=o['x'] and 0<=o['y'] and o['x']+o['width']<=768 and o['y']+o['height']<=1296,o['id']
    original=next(n for n in p['placements'] if n['id']==o['id'])
    assert original['footprint']==o['footprint'] and original['depth_y']==o['depth_y'],o['id']
for view in ['world','map']:
    doc=read(f'parade-{view}-editable.json');layers=doc['composition']['layers'];ids=[];assets=set()
    for layer in layers:
        ids.append(layer['id'])
        if layer['type']=='Image':
            assert layer['masks'] and layer['source']['fit']['custom']
            assets.add(layer['source']['assetId']);ids.extend(m['id'] for m in layer['masks'])
    assert len(ids)==len(set(ids)) and len(assets)==2
    assert Image.open(OUT/f'parade-{view}-native.png').size==(768,1296)
    assert (OUT/f'parade-{view}.tsrct').stat().st_size>0
native=read('native-evidence.json');assert native['commands'] and all(c['exit_code']==0 for c in native['commands'])
report={'status':'PASS_PAINTED_PROPOSAL_ONLY','geometry_hash':p['geometry_hash'],'source_atlas':provenance['source_atlas'],'source_surfaces':provenance['source_surfaces'],'original_source_sheets_unchanged':True,'physical_placements':len(layout['placements']),'overview_illustrations':len(p['overview_representations']),'frames':len(layout['frames']),'native_editable_projects':2,'runtime_textures':2,'map_authority':'Non-collidable illustration/semantic zone; physical collision only from canonical geometry.','limit':'Hash/dimension/layer/placement validation is not reference acceptance or production integration.'}
(OUT/'verification.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
