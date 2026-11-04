#!/usr/bin/env python3
"""Verify v6 original-source composition without promoting fidelity or admission."""
import argparse,copy,hashlib,json,math,zipfile
from pathlib import Path
from PIL import Image,ImageChops
from jsonschema import Draft202012Validator
parser=argparse.ArgumentParser();parser.add_argument('--revision',type=int,choices=[6,7,8],default=6);revision=parser.parse_args().revision
HERE=Path(__file__).resolve().parent;OUT=HERE/f'quality-v{revision}'
read=lambda p:json.loads(p.read_text());sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest();nearest=lambda n:math.floor(n+.5) if n>=0 else math.ceil(n-.5)
api=read(HERE.parents[3]/'backend/docs/api/mcr-core.openapi.json');validator=Draft202012Validator({'$ref':'#/components/schemas/Scenery','components':api['components']})
source_meta=read(OUT/'source-metadata.json');summary=read(OUT/'resource-summary.json');geometry_revision=4 if revision==8 else 3;proposal=read(HERE/f'spatial-v{geometry_revision}/geometry-proposal.json');projection=read(HERE/f'spatial-v{geometry_revision}/integer-projection.json')
for key in ['source_sheet','surface_sheet','stage_sheet','base_manifest']:
 if key in source_meta:
  pin=source_meta[key];assert sha(OUT/pin['path'])==pin['sha256']
index=read(OUT/'manifest.json')
assert sha(OUT/index['geometry']['path'])==index['geometry']['sha256']
assert sha(OUT/index['integer_projection']['path'])==index['integer_projection']['sha256']
for key in ['geometry_revision_evidence','source_metadata']:
 if key in index:
  pin=index[key];assert sha(OUT/pin['path'])==pin['sha256']
for pin in index['sources']+[pin for theme in index['themes'] for pin in theme['files']]:assert sha(OUT/pin['path'])==pin['sha256'] and (OUT/pin['path']).stat().st_size==pin['bytes']
reports=[];atlas_hashes=set();shape_evidence=[];native_probes=[]
for era in ['revenge','parade','danger']:
 folder=OUT/era;base=HERE/'eras-v5'/era;layout=read(folder/'layout.json');old=read(base/'layout.json');scenery=read(folder/'scenery.json');old_scenery=read(base/'scenery.json');meta=read(folder/'map-metadata.json');old_meta=read(base/'map-metadata.json');provenance=read(folder/'provenance.json');resources=read(folder/'resource-inputs.json')
 validator.validate(scenery)
 assert layout['placements']==projection['placements']
 assert layout['geometry_hash']==proposal['geometry_hash']==summary['geometry_hash']==meta['geometry_hash']
 expected=projection['overview_representations'];assert layout['overview_representations']==meta['overview_representations']==expected
 assert meta['places']==old_meta['places'] and meta['plaza_parcel']==proposal['plaza_parcel']
 assert meta['map_labels']==proposal['map_labels']
 assert resources['resources']==[scenery['ground'],*scenery['atlases']]
 for d in resources['resources']:
  p=HERE.parents[1]/d['path'];assert sha(p)==d['sha256'] and p.stat().st_size==d['bytes'];assert Image.open(p).size==(d['width_px'],d['height_px'])
 assert sum(d['bytes'] for d in resources['resources'])==resources['total_bytes']<=2097152
 assert sum(d['width_px']*d['height_px'] for d in resources['resources'])==resources['total_pixels']<=2097152
 for pin in provenance['source_pins']:assert sha(folder/pin['path'])==pin['sha256']
 atlas=Image.open(folder/'atlas.png');assert atlas.mode=='RGBA' and atlas.size==(1024,1024)
 for t in provenance['transforms']:
  source=Image.open(folder/t['source']).convert('RGBA');f=t['source_rect'];r=t['packed_rect'];assert layout['frames'][t['frame']]==t.get('frame_rect',r)
  expected=source.crop((f['x'],f['y'],f['x']+f['width'],f['y']+f['height'])).resize((r['width'],r['height']),Image.Resampling.NEAREST);actual=atlas.crop((r['x'],r['y'],r['x']+r['width'],r['y']+r['height']))
  assert not any(band.getbbox() for band in ImageChops.difference(expected,actual).split())
  assert r['width']==nearest(f['width']*t['scale']) and r['height']==nearest(f['height']*t['scale'])
  for u in provenance['transforms']:
   if u is t:continue
   b=u['packed_rect'];assert r['x']+r['width']<=b['x'] or b['x']+b['width']<=r['x'] or r['y']+r['height']<=b['y'] or b['y']+b['height']<=r['y']
 if revision in [7,8]:
  frozen=HERE/f'quality-v{revision-1}'/era;old_layout=read(frozen/'layout.json');old_atlas=Image.open(frozen/'atlas.png').convert('RGBA')
  if revision==8:
   assert layout['placements'][:22]==old_layout['placements'] and len(layout['placements'])==34
   assert layout['overview_representations']==old_layout['overview_representations']
   old_meta=read(frozen/'map-metadata.json');assert {k:v for k,v in meta.items() if k not in ['version','geometry_hash']}=={k:v for k,v in old_meta.items() if k not in ['version','geometry_hash']}
   assert layout['world_camera']==old_layout['world_camera'] and layout['overview_camera']==old_layout['overview_camera']
   assert scenery['ground']==read(frozen/'scenery.json')['ground']
   for t in provenance['transforms']:
    if t['frame'] in ['tree_red','tree_green','tree_yellow','tree_dark']:continue
    f=layout['frames'][t['frame']];assert f==old_layout['frames'][t['frame']];x,y=f['x']//256*256,f['y']//256*256
    assert not any(b.getbbox() for b in ImageChops.difference(atlas.crop((x,y,x+256,y+256)),old_atlas.crop((x,y,x+256,y+256))).split())
   for region in proposal['protected_art_regions']:
    box=region['rect']
    for p in layout['placements']:
     if p['depth_y']<region['depth_y'] or p['frame'] not in ['tree_red','tree_green','tree_yellow','tree_dark']:continue
     f=layout['frames'][p['frame']]
     for y in range(max(p['y'],box['y']),min(p['y']+p['height'],box['y']+box['height'])):
      for x in range(max(p['x'],box['x']),min(p['x']+p['width'],box['x']+box['width'])):
       sx=f['x']+int((x-p['x'])*f['width']/p['width']);sy=f['y']+int((y-p['y'])*f['height']/p['height'])
       assert atlas.getpixel((sx,sy))[3]<=128,('Protected plaque text',p['id'],x,y)
 if revision==7:
  frozen=HERE/'quality-v6'/era;old_layout=read(frozen/'layout.json');old_atlas=Image.open(frozen/'atlas.png').convert('RGBA')
  old_meta=read(frozen/'map-metadata.json');assert {k:v for k,v in meta.items() if k!='version'}=={k:v for k,v in old_meta.items() if k!='version'}
  assert layout['placements']==old_layout['placements'] and layout['overview_representations']==old_layout['overview_representations']
  assert layout['world_camera']==old_layout['world_camera'] and layout['overview_camera']==old_layout['overview_camera']
  assert scenery['ground']==read(frozen/'scenery.json')['ground']
  for t in provenance['transforms']:
   f=layout['frames'][t['frame']];x,y=f['x']//256*256,f['y']//256*256;tile=atlas.crop((x,y,x+256,y+256))
   if 'frame_rect' not in t:
    assert f==old_layout['frames'][t['frame']]
    assert not any(b.getbbox() for b in ImageChops.difference(tile,old_atlas.crop((x,y,x+256,y+256))).split())
   else:
    r=t['packed_rect'];expected=Image.new('RGBA',(256,256));src=Image.open(folder/t['source']).convert('RGBA');a=t['source_rect']
    expected.paste(src.crop((a['x'],a['y'],a['x']+a['width'],a['y']+a['height'])).resize((r['width'],r['height']),Image.Resampling.NEAREST),(r['x']-x,r['y']-y))
    assert not any(b.getbbox() for b in ImageChops.difference(tile,expected).split())
    assert r['y']+r['height']==f['y']+f['height']-2
    assert f['x']<=r['x'] and r['x']+r['width']<=f['x']+f['width'] and f['y']<=r['y']
  for p in layout['placements']+layout['overview_representations']:
   if p['frame'] not in ['meeting_pavilion','giveaway_pavilion','north_hall','west_hall']:continue
   f=layout['frames'][p['frame']];b=atlas.getchannel('A').crop((f['x'],f['y'],f['x']+f['width'],f['y']+f['height'])).point(lambda a:255 if a>=32 else 0).getbbox();assert b
   foot=p['y']+b[3]/f['height']*p['height'];gap=p['depth_y']-foot
   assert -.5<=gap<=7,(p['id'],gap)
   shape_evidence.append({'era':era.upper(),'id':p['id'],'image_box':[p['x'],p['y'],p['width'],p['height']],'opaque_bounds_in_frame':list(b),'opaque_base_y':foot,'authored_depth_y':p['depth_y'],'base_to_depth_gap_px':gap,'visible_projected_width':(b[2]-b[0])/f['width']*p['width'],'visible_projected_height':(b[3]-b[1])/f['height']*p['height']})
 assert [a['id'] for a in scenery['objects']]==[p['id'] for p in projection['placements']]
 if revision==8:
  for p in layout['placements']:
   if not p['frame'].startswith('tree_'):continue
   f=layout['frames'][p['frame']];b=atlas.getchannel('A').crop((f['x'],f['y'],f['x']+f['width'],f['y']+f['height'])).point(lambda a:255 if a>=32 else 0).getbbox();assert b
   gap=p['depth_y']-(p['y']+b[3]/f['height']*p['height']);assert -.5<=gap<=5,(p['id'],gap)
   shape_evidence.append({'era':era.upper(),'id':p['id'],'opaque_base_to_authored_depth_gap_px':gap,'footprint_max_y':max(y for x,y in p['footprint']),'depth_y':p['depth_y']})
 for a,c in zip(scenery['objects'],projection['placements']):
  assert a['id']==c['id'] and a['frame']==layout['frames'][c['frame']]
  assert all(a[k]==c[k] for k in ['x','y','width','height','depth_y'])
 padded=Image.open(folder/'native-atlas.png');assert padded.size==(2048,2048) and sha(folder/'native-atlas.png')==provenance['native_packaging']['sha256']
 assert not any(band.getbbox() for band in ImageChops.difference(padded.crop((0,0,1024,1024)),atlas).split())
 assert padded.getchannel('A').crop((1024,0,2048,2048)).getbbox() is None and padded.getchannel('A').crop((0,1024,2048,2048)).getbbox() is None
 ground=Image.open(HERE.parents[1]/scenery['ground']['path']).convert('RGB')
 for view in ['world','map']:
  native=Image.open(folder/f'{era}-{view}-native.png').convert('RGB');assert native.size==(768,1296)
  diff=ImageChops.difference(native.crop((440,1060,676,1266)),ground.crop((440,1060,676,1266)));a,b,c=diff.split();maximum=ImageChops.lighter(ImageChops.lighter(a,b),c);assert sum(maximum.histogram()[25:])>236*206*.35
  with zipfile.ZipFile(folder/f'{era}-{view}.tsrct') as z:
   assets=[n for n in z.namelist() if n.startswith('assets/') and n.endswith('.png')];assert len(assets)==2
   assert {'sha256:'+hashlib.sha256(z.read(n)).hexdigest() for n in assets}=={sha(folder/'native-atlas.png'),scenery['ground']['sha256']}
  doc=read(folder/f'{era}-{view}-editable.json');ids=[]
  for layer in doc['composition']['layers']:ids.append(layer['id']);ids.extend(m['id'] for m in layer.get('masks',[]))
  assert len(ids)==len(set(ids))
  if revision==8:
   props=layout['placements']
   if view=='map':
    replaced={o.get('replaces_prop_id') for o in layout['overview_representations']};props=[o for o in props if o['id'] not in replaced]+layout['overview_representations']
   assert {l['name'] for l in doc['composition']['layers'] if l['type']=='Image'}=={'Original material ground',*[o['id'] for o in props]}
   # A broad difference-from-ground test could pass on new trees even if the
   # lower building vanished. Probe its actual opaque, locally flat source
   # colors in the exported native PNG instead.
   o=next(p for p in props if p['id']=='giveaway_pavilion');f=layout['frames'][o['frame']];samples=[]
   for dy in range(8,o['height']-8,4):
    for dx in range(8,o['width']-8,4):
     sx=int(f['x']+(dx+.5)*f['width']/o['width']);sy=int(f['y']+(dy+.5)*f['height']/o['height']);patch=atlas.crop((sx-1,sy-1,sx+2,sy+2))
     if patch.getchannel('A').getextrema()[0]<248 or any(b-a>28 for a,b in patch.convert('RGB').getextrema()):continue
     expected=atlas.getpixel((sx,sy))[:3];px,py=o['x']+dx,o['y']+dy
     if sum(abs(a-b) for a,b in zip(expected,ground.getpixel((px,py))))<80:continue
     error=max(abs(a-b) for a,b in zip(expected,native.getpixel((px,py))));samples.append(error)
   assert len(samples)>=50 and sum(e<=40 for e in samples)/len(samples)>=.9,(era,view,len(samples),sum(e<=40 for e in samples))
   native_probes.append({'era':era.upper(),'view':view.upper(),'landmark':'giveaway_pavilion','near_opaque_flat_source_samples':len(samples),'samples_with_max_channel_error_le40':sum(e<=40 for e in samples),'pass_fraction':sum(e<=40 for e in samples)/len(samples),'threshold':'Source3x3 alpha>=248 and RGB variation<=28; at least50 samples; >=90percent within40/channel of source color; expected differs from ground by >=80 summed RGB. Near-opaque original source alpha is preserved, never rewritten to255.'})
 assert all(c['exit_code']==0 for c in read(folder/'native-evidence.json')['commands'])
 atlas_hashes.add(scenery['atlases'][0]['sha256']);reports.append({'theme':era.upper(),'schema':'PASS','bytes':resources['total_bytes'],'pixels':resources['total_pixels'],'objects':len(projection['placements']),'map_only_representations':6,'native_projects':2})
assert len(atlas_hashes)==3
report={'status':f'PASS_V{revision}_ART_INPUTS_ONLY','geometry_hash':proposal['geometry_hash'],'themes':reports,'explicit_new_geometry_revision':3,'place_identities_and_review_states_unchanged':True,'limitations':['Developer source/contract/render checks, not a fourth critic score or target acceptance.','Source-art qualification is separate from physical destination authorization; concept-only place states remain truthful.','New spatial-v3 coordinates require explicit coordinated server/client map and resource adoption; never mix with geometry20168.']}
if revision==8:
 report['explicit_new_geometry_revision']=4
 report['new_unbound_rooted_props']=12
 report['protected_plaque_text']='PASS_ALL_ERAS'
 report['unchanged_v7_inputs']=['twelve non-tree atlas cells','existing22props/footprints','ground/paving/terrain/spawns/interactions/cameras/places']
 report['limitations'][2]='New spatial-v4 collision/resources require explicit new canonical Tiled and coordinated server/client binding; never mix with spatial-v3/map4/v6.'
 (OUT/'canopy-root-evidence.json').write_text(json.dumps({'status':'PASS_SOURCE_ROOT_ALIGNMENT','limits':'Actual alpha>=32 base projected to unchanged/new image boxes, within5px above authored depth; no physical-source or avatar proof.','instances':shape_evidence},indent=2)+'\n')
 (OUT/'native-venue-probes.json').write_text(json.dumps({'status':'PASS_ACTUAL_LOWER_VENUE_PNG_COLORS','reason':'Prevents earlier native mask clipping from being hidden by new tree pixels in a broad image-vs-ground difference test.','probes':native_probes},indent=2)+'\n')
if revision==7:
 report['geometry_revision']=report.pop('explicit_new_geometry_revision')
 report['unchanged_v6_inputs']=['ground','twelve other atlas frames','placements/footprints/collision/cells/cameras','MAP representations/labels/places']
 (OUT/'venue-shape-evidence.json').write_text(json.dumps({'status':'PASS_PROJECTED_VENUE_BASES','measure':'Actual alpha>=32 projected to unchanged destination rectangles; original source proportions use bottom-aligned transparent frame padding.','limits':'Opaque base is at most7px above authored depth. This is an art-root alignment check, not target-match scoring, collision authority or moving-avatar proof.','instances':shape_evidence},indent=2)+'\n')
(OUT/'verification.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
