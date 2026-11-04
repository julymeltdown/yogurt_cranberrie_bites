#!/usr/bin/env python3
"""Verify v6 original-source composition without promoting fidelity or admission."""
import argparse,copy,hashlib,json,math,zipfile
from pathlib import Path
from PIL import Image,ImageChops
from jsonschema import Draft202012Validator
parser=argparse.ArgumentParser();parser.add_argument('--revision',type=int,choices=[6,7],default=6);revision=parser.parse_args().revision
HERE=Path(__file__).resolve().parent;OUT=HERE/f'quality-v{revision}'
read=lambda p:json.loads(p.read_text());sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest();nearest=lambda n:math.floor(n+.5) if n>=0 else math.ceil(n-.5)
api=read(HERE.parents[3]/'backend/docs/api/mcr-core.openapi.json');validator=Draft202012Validator({'$ref':'#/components/schemas/Scenery','components':api['components']})
source_meta=read(OUT/'source-metadata.json');summary=read(OUT/'resource-summary.json');proposal=read(HERE/'spatial-v3/geometry-proposal.json');projection=read(HERE/'spatial-v3/integer-projection.json')
for key in ['source_sheet','surface_sheet','stage_sheet','base_manifest']:
 if key in source_meta:
  pin=source_meta[key];assert sha(OUT/pin['path'])==pin['sha256']
index=read(OUT/'manifest.json')
assert sha(OUT/index['geometry']['path'])==index['geometry']['sha256']
assert sha(OUT/index['integer_projection']['path'])==index['integer_projection']['sha256']
for pin in index['sources']+[pin for theme in index['themes'] for pin in theme['files']]:assert sha(OUT/pin['path'])==pin['sha256'] and (OUT/pin['path']).stat().st_size==pin['bytes']
reports=[];atlas_hashes=set();shape_evidence=[]
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
 assert all(c['exit_code']==0 for c in read(folder/'native-evidence.json')['commands'])
 atlas_hashes.add(scenery['atlases'][0]['sha256']);reports.append({'theme':era.upper(),'schema':'PASS','bytes':resources['total_bytes'],'pixels':resources['total_pixels'],'objects':22,'map_only_representations':6,'native_projects':2})
assert len(atlas_hashes)==3
report={'status':f'PASS_V{revision}_ART_INPUTS_ONLY','geometry_hash':proposal['geometry_hash'],'themes':reports,'explicit_new_geometry_revision':3,'place_identities_and_review_states_unchanged':True,'limitations':['Developer source/contract/render checks, not a fourth critic score or target acceptance.','Source-art qualification is separate from physical destination authorization; concept-only place states remain truthful.','New spatial-v3 coordinates require explicit coordinated server/client map and resource adoption; never mix with geometry20168.']}
if revision==7:
 report['geometry_revision']=report.pop('explicit_new_geometry_revision')
 report['unchanged_v6_inputs']=['ground','twelve other atlas frames','placements/footprints/collision/cells/cameras','MAP representations/labels/places']
 (OUT/'venue-shape-evidence.json').write_text(json.dumps({'status':'PASS_PROJECTED_VENUE_BASES','measure':'Actual alpha>=32 projected to unchanged destination rectangles; original source proportions use bottom-aligned transparent frame padding.','limits':'Opaque base is at most7px above authored depth. This is an art-root alignment check, not target-match scoring, collision authority or moving-avatar proof.','instances':shape_evidence},indent=2)+'\n')
(OUT/'verification.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
