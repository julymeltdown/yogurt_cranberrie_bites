#!/usr/bin/env python3
"""Prove bounded MAP illustration changes without conferring world authority."""
import hashlib,json,zipfile
from pathlib import Path
from PIL import Image,ImageChops,ImageDraw
HERE=Path(__file__).resolve().parent;SOURCE=HERE.parent;ROOT=SOURCE.parents[3]
read=lambda p:json.loads(p.read_text());sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
s=read(HERE/'presentation.json');p=read(SOURCE/'spatial-v5/geometry-proposal.json');labels=read(SOURCE/'quality-v10/map-label-presentation.json');decor=s['decorations'];ids={o['id'] for o in decor}
canonical='sha256:'+hashlib.sha256(json.dumps(p['geometry'],sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()).hexdigest()
assert canonical==s['geometry_hash']==p['geometry_hash']==labels['geometry_hash']
assert s['view']=='MAP_ONLY' and s['dimensions']=={'width':768,'height':1296} and len(ids)==len(decor)==10
for pin in s['frozen_inputs']:
 f=ROOT/pin['path'];assert sha(f)==pin['sha256'] and f.stat().st_size==pin['bytes']
def inside(x,y,poly):
 return sum((a[1]>y)!=(b[1]>y) and x<(b[0]-a[0])*(y-a[1])/(b[1]-a[1])+a[0] for a,b in zip(poly,poly[1:]+poly[:1]) if a[1]!=b[1])%2==1
mask=Image.new('L',(768,1296));draw=ImageDraw.Draw(mask)
points=p['geometry']['spawns']+p['geometry']['interactions']
for o in decor:
 assert o['interaction_id'] is None and o['frame']=='shrub_bed' and o['view']=='MAP_ONLY'
 assert o['kind']=='unbound_map_planting_illustration_not_a_world_solid'
 assert not any(k in o for k in ['place_id','destination_id','collision','footprint','physical_location'])
 x,y,w,h=[o[k] for k in ['x','y','width','height']];assert all(type(v) is int for v in [x,y,w,h,o['depth_y']])
 assert 0<=x<x+w<=768 and 0<=y<y+h<=1296 and o['depth_y']==y+h
 draw.rectangle((x,y,x+w-1,y+h-1),fill=255)
 for l in labels['labels']:
  r=l['rect'];assert max(x,r['x'])>=min(x+w,r['x']+r['width']) or max(y,r['y'])>=min(y+h,r['y']+r['height'])
 for foot in points:
  fx,fy=(foot['cell_x']+.5)*16,(foot['cell_y']+.5)*16;assert not(x-8<=fx<x+w+8 and y-8<=fy<y+h+8)
 for py in range(y,y+h):
  for px in range(x,x+w):
   if o['surface']=='semantic_plaza':assert inside(px+.5,py+.5,p['plaza_parcel'])
   else:assert o['surface']=='canonical_lawn' and not any(inside(px+.5,py+.5,poly) for poly in p['paving_polygons'])
reports=[];visible=[]
for t in s['themes']:
 era=t['theme_id'].lower();base=SOURCE/f'quality-v10/{era}';layout=read(base/'layout.json');scene=read(base/'scenery.json')
 assert t['geometry_hash']==layout['geometry_hash']==canonical
 for pin in [t['base_scene'],t['layout'],t['native_atlas'],t['source_provenance'],t['original_source']]:
  f=ROOT/pin['path'];assert sha(f)==pin['sha256'] and f.stat().st_size==pin['bytes']
 assert t['atlas']==scene['atlases'][0] and t['frame']==layout['frames']['shrub_bed']
 assert t['runtime_pngs']==2 and t['runtime_bytes']==scene['ground']['bytes']+t['atlas']['bytes']<=2097152
 assert t['runtime_pixels']==2043904<=2097152
 doc=read(HERE/f'{era}-map-editable.json');before=read(base/f'{era}-map-editable.json');layers=doc['composition']['layers'];old=before['composition']['layers']
 assert [l for l in layers if l['name'] not in ids]==old
 new=[l for l in layers if l['name'] in ids];assert len(new)==10 and all(l['type']=='Image' for l in new)
 assert len({l['id'] for l in layers})==len(layers)
 replaced={o.get('replaces_prop_id') for o in layout['overview_representations']}
 props=[o for o in layout['placements'] if o['id'] not in replaced]+layout['overview_representations']+decor
 depths={o['id']:o['depth_y'] for o in props};order=[depths[l['name']] for l in layers if l['name'] in depths];assert order==sorted(order,reverse=True)
 frame=t['frame'];atlas=Image.open(base/'atlas.png').convert('RGBA')
 for r in [scene['ground'],t['atlas']]:
  file=SOURCE.parents[1]/r['path'];assert sha(file)==r['sha256'] and file.stat().st_size==r['bytes']
 transform=t['atlas_transform'];provenance=read(ROOT/t['source_provenance']['path']);assert transform==next(v for v in provenance['transforms'] if v['frame']=='shrub_bed')
 assert transform['packed_rect']==frame and transform['resampling']=='nearest'
 raw=Image.open(ROOT/t['original_source']['path']).convert('RGBA');r=transform['source_rect'];expected=raw.crop((r['x'],r['y'],r['x']+r['width'],r['y']+r['height'])).resize((frame['width'],frame['height']),Image.Resampling.NEAREST)
 actual=atlas.crop((frame['x'],frame['y'],frame['x']+frame['width'],frame['y']+frame['height']));assert not any(b.getbbox() for b in ImageChops.difference(expected,actual).split())
 alpha=atlas.crop((frame['x'],frame['y'],frame['x']+frame['width'],frame['y']+frame['height'])).getchannel('A');bbox=alpha.point(lambda a:255 if a>=32 else 0).getbbox();assert bbox
 atlas_id=next(l['source']['assetId'] for l in old if l['type']=='Image' and l['name']!='Original material ground')
 for o in decor:
  l=next(l for l in new if l['name']==o['id']);sx,sy=o['width']/frame['width'],o['height']/frame['height']
  assert l['source']=={'assetId':atlas_id,'fit':{'custom':{'scale':[sx,sy],'contentCenter':[o['x']+(1024-frame['x'])*sx,o['y']+(1024-frame['y'])*sy]}}}
  assert 0<=o['depth_y']-(o['y']+bbox[3]*sy)<1
 with zipfile.ZipFile(HERE/f'{era}-map.tsrct') as z:
  assert json.loads(z.read('project.json'))['composition']==doc['composition']
  pngs=[n for n in z.namelist() if n.endswith('.png')];fonts=[n for n in z.namelist() if n.endswith('.ttf')];assert len(pngs)==2 and len(fonts)==1
  assert {'sha256:'+hashlib.sha256(z.read(n)).hexdigest() for n in pngs}=={scene['ground']['sha256'],sha(base/'native-atlas.png')}
  assert 'sha256:'+hashlib.sha256(z.read(fonts[0])).hexdigest()==labels['font']['sha256']
 image=Image.open(HERE/f'{era}-map-native.png').convert('RGB');previous=Image.open(base/f'{era}-map-native.png').convert('RGB');assert image.size==previous.size==(768,1296)
 diff=ImageChops.difference(image,previous);changed=ImageChops.lighter(ImageChops.lighter(*diff.split()[:2]),diff.split()[2]);assert ImageChops.subtract(changed,mask).getbbox() is None
 for o in decor:
  x,y,w,h=[o[k] for k in ['x','y','width','height']];count=sum(changed.crop((x,y,x+w,y+h)).histogram()[6:]);expected=sum(alpha.resize((w,h),Image.Resampling.LANCZOS).histogram()[32:]);assert expected>200 and count/expected>=.85,(era,o['id'],count,expected)
  visible.append({'era':era,'id':o['id'],'changed_pixels_gt5':count,'source_alpha_support_ge32':expected,'visible_change_fraction':count/expected,'threshold':'At least85percent of resized original source-alpha support changes the actual native image by >5/channel. This is visibility, not reference-match acceptance.'})
 reports.append({'era':era,'pixel_exact_outside_decoration_rects':True,'unchanged_base_layers':len(old),'new_image_layers':len(new),'runtime_pngs':2,'bytes':t['runtime_bytes'],'pixels':t['runtime_pixels']})
assert all(c['exit_code']==0 for c in read(HERE/'native-evidence.json')['commands'])
report={'status':'PASS_MAP_ILLUSTRATION_SOURCE_ONLY','geometry_hash':canonical,'decorations':len(decor),'new_world_solids_places_interactions':0,'themes':reports,'visible_illustrations':visible,'limits':['Original canonical terrain/collision/world props remain exactly frozen; this is a MAP-only illustration overlay.','Pixel equality outside rectangles and illustration visibility do not constitute whole-reference acceptance.','No fourth visual panel, new critic score, application/Phaser/server/release/source-review/device/multiuser/physical acceptance.']}
(HERE/'verification.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k!='visible_illustrations'}))
