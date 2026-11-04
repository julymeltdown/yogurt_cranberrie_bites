#!/usr/bin/env python3
"""Current C06, immutable world, painted-edge pixels and actual northern facades."""
import hashlib,json,zipfile
from pathlib import Path
from PIL import Image,ImageChops,ImageDraw,ImageFilter
from jsonschema import Draft202012Validator
HERE=Path(__file__).resolve().parent;OUT=HERE/'quality-v9';BASE=HERE/'quality-v8'
read=lambda p:json.loads(p.read_text());sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
p=read(HERE/'spatial-v4/geometry-proposal.json');projection=read(HERE/'spatial-v4/integer-projection.json');m=read(OUT/'manifest.json')
api=read(HERE.parents[3]/'backend/docs/api/mcr-core.openapi.json');validator=Draft202012Validator({'$ref':'#/components/schemas/Scenery','components':api['components']})
for pin in [m['geometry'],m['integer_projection'],m['base_manifest'],m['map_label_sidecar'],*m['ground_sources'],*[f for t in m['themes'] for f in t['files']]]:
 f=OUT/pin['path'];assert sha(f)==pin['sha256'];assert 'bytes' not in pin or f.stat().st_size==pin['bytes']
native=Image.open(OUT/'ground-native.png');ground=Image.open(OUT/'ground.png');oldground=Image.open(HERE/'quality-v6/ground.png').convert('RGB')
assert native.size==ground.size==(768,1296) and native.mode=='RGBA' and ground.mode=='RGB' and native.getchannel('A').getextrema()==(255,255)
assert not any(c.getbbox() for c in ImageChops.difference(native.convert('RGB'),ground).split())
# Independent raster union / morphology checks paint is near real route edges,
# not hidden seams between overlapping road polygons or changes to the lawn.
union=Image.new('L',(768,1296));d=ImageDraw.Draw(union)
for poly in p['paving_polygons']:d.polygon([tuple(q) for q in poly],fill=255)
edge=ImageChops.subtract(union.filter(ImageFilter.MaxFilter(15)),union.filter(ImageFilter.MinFilter(15)))
diff=ImageChops.difference(ground,oldground);changed=Image.new('L',ground.size)
for c in diff.split():changed=ImageChops.lighter(changed,c)
assert ImageChops.subtract(changed,edge).getbbox() is None,'Ground changed away from actual union boundary'
changed_count=sum(changed.point(lambda n:255 if n else 0).histogram()[1:]);assert 10000<changed_count<100000
with zipfile.ZipFile(OUT/'ground.tsrct') as z:
 assert json.loads(z.read('project.json'))['composition']==read(OUT/'ground-editable.json')['composition']
 assert len([n for n in z.namelist() if n.endswith('.png')])==1
summary=read(OUT/'resource-summary.json');reports=[];facades=[]
for era in ['revenge','parade','danger']:
 f=OUT/era;old=BASE/era;layout=read(f/'layout.json');before=read(old/'layout.json');scene=read(f/'scenery.json');oldscene=read(old/'scenery.json');meta=read(f/'map-metadata.json');resources=read(f/'resource-inputs.json')
 validator.validate(scene);assert layout['placements']==before['placements']==projection['placements'];assert len(scene['objects'])==34
 assert {k:v for k,v in scene.items() if k!='ground'}=={k:v for k,v in oldscene.items() if k!='ground'}
 assert meta['places']==read(old/'map-metadata.json')['places'] and meta['map_labels']==p['map_labels']
 assert layout['world_camera']==before['world_camera'] and layout['overview_camera']==before['overview_camera']
 assert layout['overview_representations'][:6]==before['overview_representations'] and len(layout['overview_representations'])==8
 assert layout['overview_representations']==meta['overview_representations']
 new=layout['overview_representations'][6:];assert {o['replaces_prop_id'] for o in new}=={'northern_hall','north_eastern_hall'} and all(o['interaction_id'] is None for o in new)
 assert sha(f/'atlas.png')==sha(old/'atlas.png') and sha(f/'native-atlas.png')==sha(old/'native-atlas.png')
 assert resources['resources']==[scene['ground'],*scene['atlases']]
 for r in resources['resources']:
  path=HERE.parents[1]/r['path'];assert sha(path)==r['sha256'] and path.stat().st_size==r['bytes']
 assert resources['total_bytes']==sum(r['bytes'] for r in resources['resources'])<=2097152
 assert resources['total_pixels']==sum(r['width_px']*r['height_px'] for r in resources['resources'])==2043904
 atlas=Image.open(f/'atlas.png').convert('RGBA')
 for view in ['world','map']:
  image=Image.open(f/f'{era}-{view}-native.png').convert('RGB');assert image.size==(768,1296)
  doc=read(f/f'{era}-{view}-editable.json');layers=doc['composition']['layers'];ids=[l['id'] for l in layers];assert len(ids)==len(set(ids))
  props=layout['placements'];texts=[]
  if view=='map':
   replaced={o.get('replaces_prop_id') for o in layout['overview_representations']};props=[o for o in props if o['id'] not in replaced]+layout['overview_representations']
   texts=[l for l in layers if l['type']=='Text'];assert len(texts)==8
  assert {l['name'] for l in layers if l['type']=='Image'}=={'Original material ground',*[o['id'] for o in props]}
  with zipfile.ZipFile(f/f'{era}-{view}.tsrct') as z:
   assert json.loads(z.read('project.json'))['composition']==doc['composition']
   pngs=[n for n in z.namelist() if n.endswith('.png')];assert len(pngs)==2
   assert {'sha256:'+hashlib.sha256(z.read(n)).hexdigest() for n in pngs}=={scene['ground']['sha256'],sha(f/'native-atlas.png')}
   fonts=[n for n in z.namelist() if n.endswith('.ttf')];assert len(fonts)==(1 if view=='map' else 0)
   if fonts:assert 'sha256:'+hashlib.sha256(z.read(fonts[0])).hexdigest()==sha(HERE/'map-labels-v1/map-labels-bold.ttf')
  if view=='map':
   previous=Image.open(old/f'{era}-map-native.png').convert('RGB')
   for o in new:
    frame=layout['frames'][o['frame']];samples=[]
    for dy in range(6,o['height']-6,3):
     for dx in range(6,o['width']-6,3):
      px,py=o['x']+dx,o['y']+dy
      if not (0<=px<768 and 0<=py<1296):continue
      sx=int(frame['x']+(dx+.5)*frame['width']/o['width']);sy=int(frame['y']+(dy+.5)*frame['height']/o['height']);patch=atlas.crop((sx-1,sy-1,sx+2,sy+2))
      if patch.getchannel('A').getextrema()[0]<248 or any(b-a>28 for a,b in patch.convert('RGB').getextrema()):continue
      expected=atlas.getpixel((sx,sy))[:3]
      if sum(abs(a-b) for a,b in zip(expected,ground.getpixel((px,py))))<80:continue
      samples.append((max(abs(a-b) for a,b in zip(expected,image.getpixel((px,py))))<=40,max(abs(a-b) for a,b in zip(expected,previous.getpixel((px,py))))<=40))
    count=sum(a for a,b in samples);oldcount=sum(b for a,b in samples)
    assert len(samples)>=50 and count/len(samples)>=.4 and count>=oldcount+30,(era,o['id'],len(samples),count,oldcount)
    facades.append({'era':era,'id':o['id'],'near_opaque_source_samples':len(samples),'v9_visible_matches':count,'v8_same_region_matches':oldcount,'threshold':'At least40percent source-color matches and30more than old rendering; direct static color samples, not a critic or fidelity score.'})
 reports.append({'era':era,'schema':'PASS','world_props':34,'map_representations':8,'runtime_pngs':2,'bytes':resources['total_bytes'],'pixels':resources['total_pixels'],'native_projects':2})
report={'status':'PASS_V9_SOURCE_COMPOSITION_ONLY','geometry_hash':p['geometry_hash'],'ground_changed_pixels':changed_count,'ground_change_scope':'Only within7px independent raster-union edge neighborhood; lossless native-RGB parity.','canonical_geometry_and_original_places_unchanged':True,'themes':reports,'north_facade_probes':facades,'limits':['Painted route edges do not certify geometric route-hierarchy fidelity.','No fourth critic score, application, source qualification, backend admission, physical or multiuser acceptance.']}
(OUT/'verification.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
