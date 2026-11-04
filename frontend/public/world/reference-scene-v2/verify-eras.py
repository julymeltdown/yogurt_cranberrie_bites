#!/usr/bin/env python3
"""Validate three original era derivatives against immutable geometry and C06."""
import hashlib
import json
import math
import zipfile
from pathlib import Path
from PIL import Image, ImageChops
from jsonschema import Draft202012Validator
HERE=Path(__file__).resolve().parent
OUT=HERE/'eras-v5'
read=lambda p:json.loads(p.read_text())
sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
nearest=lambda v:math.floor(v+.5) if v>=0 else math.ceil(v-.5)
base=read(HERE/'runtime-v4/parade-layout.json');base_scenery=read(HERE/'runtime-v4/parade-scenery.json');base_map=read(HERE/'runtime-v4/parade-map-metadata.json')
api=read(HERE.parents[3]/'backend/docs/api/mcr-core.openapi.json')
validator=Draft202012Validator({'$ref':'#/components/schemas/Scenery','components':api['components']})
index=read(OUT/'manifest.json')
assert index['geometry']['sha256']==sha(HERE/'geometry-proposal.json')
assert index['integer_projection']['sha256']==sha(HERE/'runtime-v4/parade-integer-projection.json')
for theme in index['themes']:
    for pin in theme['files']:
        assert sha(OUT/pin['path'])==pin['sha256'] and (OUT/pin['path']).stat().st_size==pin['bytes']
reports=[];source_hashes=set();atlas_hashes=set()
for era in ['revenge','parade','danger']:
    folder=OUT/era;layout=read(folder/'layout.json');scenery=read(folder/'scenery.json');meta=read(folder/'map-metadata.json');provenance=read(folder/'provenance.json');inputs=read(folder/'resource-inputs.json')
    validator.validate(scenery)
    assert layout['geometry_hash']==base['geometry_hash']==meta['geometry_hash']==provenance['geometry_hash']
    assert layout['placements']==base['placements'] and layout['overview_representations']==base['overview_representations']
    assert meta['overview_representations']==base_map['overview_representations'] and meta['plaza_parcel']==base_map['plaza_parcel'] and meta['map_labels']==base_map['map_labels'] and meta['places']==base_map['places']
    assert scenery['ground']==base_scenery['ground']
    for descriptor in [scenery['ground'],*scenery['atlases']]:
        path=HERE.parents[1]/descriptor['path'];assert path.is_file()
        assert sha(path)==descriptor['sha256'] and path.stat().st_size==descriptor['bytes']
        assert Image.open(path).size==(descriptor['width_px'],descriptor['height_px'])
    assert sum(d['bytes'] for d in inputs['resources'])==inputs['combined_bytes']<=2097152
    assert sum(d['width_px']*d['height_px'] for d in inputs['resources'])==inputs['combined_pixels']<=2097152
    for pin in inputs['metadata_pins']:
        assert sha(folder/pin['path'])==pin['sha256'] and (folder/pin['path']).stat().st_size==pin['bytes']
    for pin in [provenance['input_source'],provenance['generated_source']]:assert sha(folder/pin['path'])==pin['sha256']
    source=Image.open(folder/'source-atlas.png');atlas=Image.open(folder/'atlas.png');assert source.mode==atlas.mode=='RGBA'
    assert source.getchannel('A').getextrema()==atlas.getchannel('A').getextrema()==(0,255)
    for t in provenance['transforms']:
        f=t['source_rect'];r=t['packed_rect']
        assert layout['frames'][t['frame']]==r
        expected=source.crop((f['x'],f['y'],f['x']+f['width'],f['y']+f['height'])).resize((r['width'],r['height']),Image.Resampling.NEAREST)
        actual=atlas.crop((r['x'],r['y'],r['x']+r['width'],r['y']+r['height']))
        assert not any(band.getbbox() for band in ImageChops.difference(expected,actual).split())
        assert r['width']==nearest(f['width']*t['scale']) and r['height']==nearest(f['height']*t['scale'])
        assert r['x']>=0 and r['y']>=0 and r['x']+r['width']<=1024 and r['y']+r['height']<=1024
        for u in provenance['transforms']:
            if u is t:continue
            b=u['packed_rect'];assert r['x']+r['width']<=b['x'] or b['x']+b['width']<=r['x'] or r['y']+r['height']<=b['y'] or b['y']+b['height']<=r['y']
    for a,b,c in zip(scenery['objects'],base_scenery['objects'],layout['placements']):
        assert {k:v for k,v in a.items() if k not in ['frame','atlas_id']}=={k:v for k,v in b.items() if k not in ['frame','atlas_id']}
        assert a['frame']==layout['frames'][c['frame']]
    padded=Image.open(folder/'native-atlas.png')
    assert padded.size==(2048,2048) and sha(folder/'native-atlas.png')==provenance['native_packaging']['sha256']
    assert not any(band.getbbox() for band in ImageChops.difference(padded.crop((0,0,1024,1024)),atlas).split())
    assert padded.getchannel('A').crop((1024,0,2048,2048)).getbbox() is None and padded.getchannel('A').crop((0,1024,2048,2048)).getbbox() is None
    for view in ['world','map']:
        doc=read(folder/f'{era}-{view}-editable.json');ids=[];assets=set()
        for layer in doc['composition']['layers']:
            ids.append(layer['id'])
            if layer['type']=='Image':
                assert layer['masks'] and layer['source']['fit']['custom'];assets.add(layer['source']['assetId']);ids.extend(m['id'] for m in layer['masks'])
        assert len(ids)==len(set(ids)) and len(assets)==2
        native=Image.open(folder/f'{era}-{view}-native.png').convert('RGB');assert native.size==(768,1296)
        ground=Image.open(HERE/'runtime-v4/parade-ground.png').convert('RGB')
        # Catches the real CLI0.4 mask-surface clip below y=1024. This parcel
        # has no MAP semantic fill or overlapping neighboring object.
        diff=ImageChops.difference(native.crop((440,1060,676,1266)),ground.crop((440,1060,676,1266)))
        channels=diff.split();maximum=ImageChops.lighter(ImageChops.lighter(channels[0],channels[1]),channels[2]);changed=sum(maximum.histogram()[25:])
        assert changed>236*206*.35,(era,view,'native lower-map giveaway missing',changed)
        with zipfile.ZipFile(folder/f'{era}-{view}.tsrct') as archive:
            assets=[n for n in archive.namelist() if n.startswith('assets/') and n.endswith('.png')]
            assert len(assets)==2
            assert {'sha256:'+hashlib.sha256(archive.read(n)).hexdigest() for n in assets}=={sha(folder/'native-atlas.png'),scenery['ground']['sha256']}
    assert all(c['exit_code']==0 for c in read(folder/'native-evidence.json')['commands'])
    source_hashes.add(provenance['generated_source']['sha256']);atlas_hashes.add(scenery['atlases'][0]['sha256'])
    reports.append({'era':era.upper(),'closed_C06_schema':'PASS','bytes':inputs['combined_bytes'],'pixels':inputs['combined_pixels'],'physical_objects':len(scenery['objects']),'map_only_representations':len(meta['overview_representations']),'frames':len(layout['frames']),'native_projects':2,'source_sha256':provenance['generated_source']['sha256'],'atlas_sha256':scenery['atlases'][0]['sha256']})
assert len(source_hashes)==len(atlas_hashes)==3
report={'status':'PASS_THREE_ORIGINAL_ERA_ART_INPUTS_ONLY','geometry_hash':base['geometry_hash'],'eras':reports,'identical_integer_placements_and_footprints':True,'distinct_original_sources_and_atlases':True,'shared_ground':'Intentional common quiet ground, not atlas/skin aliases.','limitations':['Art contract inputs only; no current app/server release, physical venue/operator qualification or multiplayer proof.','Alpha silhouettes vary by art skin; collision and rooted destination rectangles do not.','No fourth critic score or reference-target acceptance claimed.']}
(OUT/'verification.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
