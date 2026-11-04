#!/usr/bin/env python3
"""Explicit terrain candidate, native material/curb pixels and closed source composition."""
import hashlib,json,zipfile
from pathlib import Path
from PIL import Image,ImageChops,ImageDraw,ImageFilter
from jsonschema import Draft202012Validator
HERE=Path(__file__).resolve().parent;OUT=HERE/'quality-v10';BASE=HERE/'quality-v8'
read=lambda p:json.loads(p.read_text());sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
p=read(HERE/'spatial-v5/geometry-proposal.json');projection=read(HERE/'spatial-v5/integer-projection.json');m=read(OUT/'manifest.json')
canonical='sha256:'+hashlib.sha256(json.dumps(p['geometry'],sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()).hexdigest()
assert canonical==p['geometry_hash']==projection['geometry_hash']==m['geometry_hash']
api=read(HERE.parents[3]/'backend/docs/api/mcr-core.openapi.json');validator=Draft202012Validator({'$ref':'#/components/schemas/Scenery','components':api['components']})
for pin in [m['geometry'],m['integer_projection'],m['base_manifest'],m['map_label_sidecar'],m['revision_evidence'],*m['ground_sources'],*[f for t in m['themes'] for f in t['files']]]:
 f=OUT/pin['path'];assert sha(f)==pin['sha256'];assert 'bytes' not in pin or f.stat().st_size==pin['bytes']

# Label drawing values are unchanged, but the new geometry binding is explicit.
label=read(OUT/'map-label-presentation.json');base_label=read(HERE/'map-labels-v1/presentation.json')
assert label['geometry_hash']==p['geometry_hash']!=base_label['geometry_hash']
for k in ['dimensions','view','style','icons','icon_cutouts','labels','font','font_license','composition','authority']:
 assert label[k]==base_label[k],k
for pin in [label['base_presentation'],*label['frozen_inputs'],label['font'],label['font_license']]:
 f=HERE.parents[3]/pin['path'];assert sha(f)==pin['sha256'] and f.stat().st_size==pin['bytes']
# Exact original material pixels, native-only transparent padding and editable masks.
material_native=Image.open(OUT/'material-ground-native.png');material=Image.open(OUT/'material-ground.png')
assert material_native.mode=='RGBA' and material_native.getchannel('A').getextrema()==(255,255)
assert not any(c.getbbox() for c in ImageChops.difference(material_native.convert('RGB'),material).split())
source=Image.open(HERE/'quality-v6/sources/surface-sheet.png').convert('RGBA');pad=Image.open(OUT/'surface-native.png')
assert pad.size==(2048,2048) and not any(c.getbbox() for c in ImageChops.difference(source,pad.crop((0,0,1254,1254))).split())
assert pad.getchannel('A').crop((1254,0,2048,2048)).getbbox() is None and pad.getchannel('A').crop((0,1254,2048,2048)).getbbox() is None
material_doc=read(OUT/'material-ground-editable.json');assert len(material_doc['composition']['layers'])==121
assert all(l['type']=='Image' and l['masks'] for l in material_doc['composition']['layers'])
with zipfile.ZipFile(OUT/'material-ground.tsrct') as z:
 assert json.loads(z.read('project.json'))['composition']==material_doc['composition']
 pngs=[n for n in z.namelist() if n.endswith('.png')];assert len(pngs)==1
 assert 'sha256:'+hashlib.sha256(z.read(pngs[0])).hexdigest()==sha(OUT/'surface-native.png')
# Sample actual exported material pixels against the original quadrant. Boundary,
# tile seam and high-frequency samples are excluded, never recolored for a pass.
union=Image.new('L',(768,1296));draw=ImageDraw.Draw(union)
for poly in p['paving_polygons']:draw.polygon([tuple(q) for q in poly],fill=255)
edge=ImageChops.subtract(union.filter(ImageFilter.MaxFilter(15)),union.filter(ImageFilter.MinFilter(15)))
material_samples={1:[],2:[]}
for y in range(8,1296-8,7):
 for x in range(8,768-8,7):
  if edge.getpixel((x,y)) or not(4<x%128<124 and 4<y%128<124):continue
  terrain=2 if union.getpixel((x,y)) else 1
  sx=int((x%128+.5)*627/128)+(627 if terrain==1 else 0);sy=int((y%128+.5)*627/128)
  patch=source.crop((sx-2,sy-2,sx+3,sy+3)).convert('RGB')
  if any(b-a>28 for a,b in patch.getextrema()):continue
  expected=source.getpixel((sx,sy))[:3];error=max(abs(a-b) for a,b in zip(expected,material.getpixel((x,y))))
  material_samples[terrain].append(error)
for terrain,errors in material_samples.items():
 assert len(errors)>500 and sum(e<=25 for e in errors)/len(errors)>=.98,(terrain,len(errors),sum(e<=25 for e in errors))

native=Image.open(OUT/'ground-native.png');ground=Image.open(OUT/'ground.png');oldground=Image.open(OUT/'material-ground.png').convert('RGB')
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
 pngs=[n for n in z.namelist() if n.endswith('.png')];assert len(pngs)==1
 assert 'sha256:'+hashlib.sha256(z.read(pngs[0])).hexdigest()==sha(OUT/'material-ground.png'),'Packed ground source differs from selected material ground'
summary=read(OUT/'resource-summary.json');reports=[];facades=[];native_probes=[]
assert summary['geometry_hash']==canonical
for era in ['revenge','parade','danger']:
 f=OUT/era;old=BASE/era;layout=read(f/'layout.json');before=read(old/'layout.json');scene=read(f/'scenery.json');oldscene=read(old/'scenery.json');meta=read(f/'map-metadata.json');resources=read(f/'resource-inputs.json')
 assert all(v['geometry_hash']==canonical for v in [layout,meta,resources,read(f/'provenance.json')]),'Theme geometry binding differs from canonical spatial-v5'
 assert layout['geometry_revision']==5 and resources['projection']=='../../spatial-v5/integer-projection.json'
 validator.validate(scene);assert layout['placements']==before['placements']==projection['placements'];assert len(scene['objects'])==34
 assert {k:v for k,v in scene.items() if k!='ground'}=={k:v for k,v in oldscene.items() if k!='ground'}
 assert meta['map_label_presentation']['path']=='../map-label-presentation.json' and meta['map_label_presentation']['sha256']==sha(OUT/'map-label-presentation.json')
 assert all(c['exit_code']==0 for c in read(f/'native-evidence.json')['commands'])
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
  o=next(p for p in props if p['id']=='giveaway_pavilion');frame=layout['frames'][o['frame']];samples=[]
  for dy in range(8,o['height']-8,4):
   for dx in range(8,o['width']-8,4):
    sx=int(frame['x']+(dx+.5)*frame['width']/o['width']);sy=int(frame['y']+(dy+.5)*frame['height']/o['height']);patch=atlas.crop((sx-1,sy-1,sx+2,sy+2))
    if patch.getchannel('A').getextrema()[0]<248 or any(b-a>28 for a,b in patch.convert('RGB').getextrema()):continue
    expected=atlas.getpixel((sx,sy))[:3];px,py=o['x']+dx,o['y']+dy
    if view=='map' and any(l['rect']['x']<=px<l['rect']['x']+l['rect']['width'] and l['rect']['y']<=py<l['rect']['y']+l['rect']['height'] for l in label['labels']):continue
    if sum(abs(a-b) for a,b in zip(expected,ground.getpixel((px,py))))<80:continue
    error=max(abs(a-b) for a,b in zip(expected,image.getpixel((px,py))));samples.append(error)
  assert len(samples)>=50 and sum(e<=40 for e in samples)/len(samples)>=.9,(era,view,len(samples),sum(e<=40 for e in samples))
  native_probes.append({'era':era.upper(),'view':view.upper(),'landmark':'giveaway_pavilion','near_opaque_flat_source_samples':len(samples),'samples_with_max_channel_error_le40':sum(e<=40 for e in samples),'pass_fraction':sum(e<=40 for e in samples)/len(samples),'threshold':'Source3x3 alpha>=248 and RGB variation<=28; at least50 samples; >=90percent within40/channel of source color; expected differs from ground by >=80 summed RGB. MAP label rectangles excluded from visible-source samples. Original source alpha is preserved, never rewritten to255.'})
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
    facades.append({'era':era,'id':o['id'],'near_opaque_source_samples':len(samples),'v10_visible_matches':count,'v8_same_region_matches':oldcount,'threshold':'At least40percent source-color matches and30more than old rendering; direct static color samples, not a critic or fidelity score.'})
 reports.append({'era':era,'schema':'PASS','world_props':34,'map_representations':8,'runtime_pngs':2,'bytes':resources['total_bytes'],'pixels':resources['total_pixels'],'native_projects':2})
report={'status':'PASS_V10_SOURCE_COMPOSITION_ONLY','geometry_hash':p['geometry_hash'],'ground_changed_pixels':changed_count,'ground_change_scope':'Only within7px independent raster-union edge neighborhood; lossless native-RGB parity.','only_terrain_changed_from_spatial_v4':True,'themes':reports,'north_facade_probes':facades,'lower_venue_probes':native_probes,'material_probes':[{'terrain_id':t,'samples':len(e),'matches_within25_per_channel':sum(x<=25 for x in e)} for t,e in material_samples.items()],'limits':['Manual road-center anchor distances are not a whole-target fidelity score.','No fourth critic score, application, source qualification, backend admission, physical or multiuser acceptance.']}
(OUT/'verification.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
