#!/usr/bin/env python3
"""New fidelity sibling: shared physical geometry, quieter ground, oblique venues."""
import copy
import hashlib
import json
import math
from pathlib import Path
from PIL import Image
HERE=Path(__file__).resolve().parent
OUT=HERE/'quality-v6'
read=lambda p:json.loads(p.read_text())
write=lambda p,v:p.write_text(json.dumps(v,indent=2)+'\n')
sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
nearest=lambda n:math.floor(n+.5) if n>=0 else math.ceil(n-.5)
proposal=read(HERE/'spatial-v3/geometry-proposal.json');projection=read(HERE/'spatial-v3/integer-projection.json');g=proposal['geometry']
source_path=OUT/'sources/venue-sheet.png';source=Image.open(source_path).convert('RGBA');alpha=source.getchannel('A')
assert alpha.getextrema()==(0,255)
def gutter(target,extent,score):
 return min(range(max(1,nearest(target)-35),min(extent-1,nearest(target)+35)+1),key=lambda n:(score(n),abs(n-target),n))
rows=[0]+[gutter(i*source.height/4,source.height,lambda y:sum(alpha.getpixel((x,y))>=32 for x in range(source.width))) for i in [1,2,3]]+[source.height]
venue_names=['meeting_pavilion','giveaway_pavilion','north_hall','west_hall'];venue_frames={};grids=[]
for row,name in enumerate(venue_names):
 top,bottom=rows[row:row+2]
 cols=[0]+[gutter(i*source.width/3,source.width,lambda x:sum(alpha.getpixel((x,y))>=32 for y in range(top,bottom))) for i in [1,2]]+[source.width];grids.append({'row':row,'y':[top,bottom],'columns':cols})
 for col,era in enumerate(['revenge','parade','danger']):
  left,right=cols[col:col+2];bounds=alpha.crop((left,top,right,bottom)).point(lambda a:255 if a>=32 else 0).getbbox();assert bounds
  x=max(left,left+bounds[0]-2);y=max(top,top+bounds[1]-2);x2=min(right,left+bounds[2]+2);y2=min(bottom,top+bounds[3]+2)
  venue_frames[(era,name)]={'x':x,'y':y,'width':x2-x,'height':y2-y}
stage_path=OUT/'sources/stage-sheet.png';stage_source=Image.open(stage_path).convert('RGBA');stage_alpha=stage_source.getchannel('A');assert stage_alpha.getextrema()==(0,255);stage_frames={}
for i,era in enumerate(['revenge','parade','danger']):
 left=math.floor(i*stage_source.width/3);right=math.floor((i+1)*stage_source.width/3);b=stage_alpha.crop((left,0,right,stage_source.height)).point(lambda a:255 if a>=32 else 0).getbbox();assert b
 x=max(left,left+b[0]-2);y=max(0,b[1]-2);x2=min(right,left+b[2]+2);y2=min(stage_source.height,b[3]+2);stage_frames[era]={'x':x,'y':y,'width':x2-x,'height':y2-y}
material=Image.open(OUT/'sources/surface-sheet.png');assert material.width==material.height
W,H=768,1296;half=material.width/2
pattern=lambda name,x,y:f'<pattern id="{name}" width="128" height="128" patternUnits="userSpaceOnUse"><svg width="128" height="128" viewBox="{x} {y} {half} {half}"><image href="sources/surface-sheet.png" width="{material.width}" height="{material.height}"/></svg></pattern>'
defs=pattern('lawn',half,0)+pattern('cobbles',0,0)+pattern('low-planting',half,half)
points=lambda ps:' '.join(f'{x},{y}' for x,y in ps)
paving=''.join(f'<polygon id="paving-{i}" points="{points(poly)}" fill="url(#cobbles)"/>' for i,poly in enumerate(proposal['paving_polygons']))
beds=''.join(f'<ellipse id="parcel-planting-{i}" cx="{x}" cy="{y}" rx="{rx}" ry="{ry}" fill="url(#low-planting)"/>' for i,(x,y,rx,ry) in enumerate([(b['cx'],b['cy'],b['rx'],b['ry']) for b in proposal['decorative_ground_beds']]))
(OUT/'ground-layout.svg').write_text(f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}"><title>v6 quieter ground materials, identical physical paving polygons</title><defs>{defs}</defs><rect id="base-lawn" width="{W}" height="{H}" fill="url(#lawn)"/>{paving}{beds}</svg>')
source_metadata={'version':6,'status':'ORIGINAL_ART_NOT_REFERENCE_ACCEPTED','source_sheet':{'path':'sources/venue-sheet.png','sha256':sha(source_path),'width':source.width,'height':source.height},'source_grid':grids,'stage_sheet':{'path':'sources/stage-sheet.png','sha256':sha(stage_path),'width':stage_source.width,'height':stage_source.height},'stage_frames':stage_frames,'stage_prompt':'sources/stage-prompt.md','surface_sheet':{'path':'sources/surface-sheet.png','sha256':sha(OUT/'sources/surface-sheet.png'),'width':material.width,'height':material.height},'venue_frames':[{ 'era':era,'frame':name,'rect':rect} for (era,name),rect in venue_frames.items()],'prompt':'sources/prompts.md','geometry_hash':proposal['geometry_hash']}
write(OUT/'source-metadata.json',source_metadata)
if not (OUT/'ground.png').exists():print('Ground SVG authored. Render ground.png, then rerun for complete runtime descriptors.');raise SystemExit(0)
ground=Image.open(OUT/'ground.png');assert ground.size==(W,H)
if ground.mode=='RGBA':assert ground.getchannel('A').getextrema()==(255,255);ground=ground.convert('RGB');ground.save(OUT/'ground.png',optimize=True,compress_level=9)
ground_descriptor={'id':'quality_v6_shared_ground','path':'world/reference-scene-v2/quality-v6/ground.png','sha256':sha(OUT/'ground.png'),'width_px':W,'height_px':H,'bytes':(OUT/'ground.png').stat().st_size}
results=[]
for era in ['revenge','parade','danger']:
 folder=OUT/era;folder.mkdir(exist_ok=True);base=HERE/'eras-v5'/era;layout=read(base/'layout.json');base_provenance=read(base/'provenance.json');base_source=Image.open(base/'source-atlas.png').convert('RGBA')
 names=list(layout['frames']);transforms=[]
 for scale_cap in [.75,.70,.65,.60]:
  atlas=Image.new('RGBA',(1024,1024));frames={};transforms=[]
  for i,name in enumerate(names):
   if name=='stage':src=stage_source;rect=stage_frames[era];src_path='../sources/stage-sheet.png'
   elif name in venue_names:src=source;rect=venue_frames[(era,name)];src_path='../sources/venue-sheet.png'
   elif name=='tree_red' and era=='parade':src=Image.open(HERE/'painted/parade-source-atlas-v3.png').convert('RGBA');rect={k:read(HERE/'painted/layout.json')['frames']['tree_red'][k] for k in ['x','y','width','height']};src_path='../../painted/parade-source-atlas-v3.png'
   else:src=base_source;rect={k:base_provenance['source_frames'][name][k] for k in ['x','y','width','height']};src_path=f'../../eras-v5/{era}/source-atlas.png'
   scale=min(scale_cap,244/max(rect['width'],rect['height']));w,h=nearest(rect['width']*scale),nearest(rect['height']*scale)
   x=i%4*256+(256-w)//2;y=i//4*256+(256-h)//2
   crop=src.crop((rect['x'],rect['y'],rect['x']+rect['width'],rect['y']+rect['height'])).resize((w,h),Image.Resampling.NEAREST);atlas.paste(crop,(x,y));frames[name]={'x':x,'y':y,'width':w,'height':h}
   transforms.append({'frame':name,'source':src_path,'source_rect':rect,'packed_rect':frames[name],'scale':scale,'resampling':'nearest','rounding':'nearest ties away from zero'})
  atlas.save(folder/'atlas.png',optimize=True,compress_level=9)
  if ground_descriptor['bytes']+(folder/'atlas.png').stat().st_size<=2097152:break
 else:raise AssertionError('Cannot fit incumbent scenery budget')
 native=Image.new('RGBA',(2048,2048));native.paste(atlas,(0,0));native.save(folder/'native-atlas.png',optimize=True,compress_level=9)
 layout.update(version=6,geometry_hash=proposal['geometry_hash'],placements=copy.deepcopy(projection['placements']),overview_representations=copy.deepcopy(projection['overview_representations']),world_camera=proposal['world_camera'],overview_camera=proposal['overview_camera'],frames=frames,ground='../ground.png',native_atlas='native-atlas.png',native_atlas_dimensions=[2048,2048],geometry_revision=3,skins='All three use explicit spatial-v3 geometry and integer projection; published v4/v5 remain unchanged.',runtime_derivative='New original oblique venue/stage sheets and quieter materials, with audited original v3/v5 scenery samples. Explicit nearest packing; separately versioned spatial-v3 physical coordinates/collision.')
 stage=next(o for o in layout['overview_representations'] if o['id']=='stage_venue_representation');stage.update(x=185,y=163,width=300,height=210)
 write(folder/'layout.json',layout)
 scenery=read(base/'scenery.json');scenery['ground']=ground_descriptor;aid=era+'_quality_v6_atlas';descriptor={'id':aid,'path':f'world/reference-scene-v2/quality-v6/{era}/atlas.png','sha256':sha(folder/'atlas.png'),'width_px':1024,'height_px':1024,'bytes':(folder/'atlas.png').stat().st_size};scenery['atlases']=[descriptor]
 for o,p in zip(scenery['objects'],layout['placements']):o.update(atlas_id=aid,frame=frames[p['frame']],**{k:p[k] for k in ['x','y','width','height','depth_y']})
 write(folder/'scenery.json',scenery)
 meta=read(base/'map-metadata.json');meta.update(version=6,geometry_hash=proposal['geometry_hash'],overview_representations=layout['overview_representations'],plaza_parcel=proposal['plaza_parcel'],map_labels=copy.deepcopy(proposal['map_labels']))
 next(o for o in meta['map_labels'] if o['interaction_id']=='stage_info').update(x=335,y=147)
 write(folder/'map-metadata.json',meta)
 provenance={'version':6,'theme_id':era.upper(),'status':'PROPOSAL_NOT_RELEASE_BOUND','base_revision':'023201343a452b859759e02039d367c11d1ff8ad','geometry_hash':proposal['geometry_hash'],'source_pins':[{'path':'../../painted/parade-source-atlas-v3.png','sha256':sha(HERE/'painted/parade-source-atlas-v3.png')},{'path':'../sources/stage-sheet.png','sha256':sha(stage_path)},{'path':f'../../eras-v5/{era}/source-atlas.png','sha256':sha(base/'source-atlas.png')},{'path':'../sources/venue-sheet.png','sha256':sha(source_path)},{'path':'../sources/surface-sheet.png','sha256':sha(OUT/'sources/surface-sheet.png')}],'transforms':transforms,'native_packaging':{'path':'native-atlas.png','sha256':sha(folder/'native-atlas.png'),'width':2048,'height':2048,'method':'Runtime atlas at0,0 with transparent padding only for native image-mask bounds.'},'spatial_revision':{'path':'../../spatial-v3/geometry-proposal.json','sha256':sha(HERE/'spatial-v3/geometry-proposal.json'),'projection':'../../spatial-v3/integer-projection.json','base_geometry_hash':'sha256:20168a1df64cd3c72ea3b8cb576ca895dcff8c7edfb0e77886cc647fb1df37e5'},'map_change':'Explicit spatial-v3 coordinates/collision/cell/camera candidate. Stage semantic representation300x210center335root373 and UIanchor335,147; no identity/place review changes. Never mix with geometry20168 releases.','physical_provenance':'All frozen nested places remain UNREVIEWED and virtual concept.','fidelity':'Candidate after concrete source fixes; not a new critic score or acceptance.'}
 write(folder/'provenance.json',provenance)
 write(folder/'resource-inputs.json',{'version':6,'theme_id':era.upper(),'geometry_hash':proposal['geometry_hash'],'status':'PROPOSAL_NOT_RELEASE_BOUND','resources':[ground_descriptor,descriptor],'total_bytes':ground_descriptor['bytes']+descriptor['bytes'],'total_pixels':W*H+1024**2,'limits':{'bytes':2097152,'pixels':2097152},'budget_pass':True,'scenery':'scenery.json','layout':'layout.json','map_metadata':'map-metadata.json','projection':'../../spatial-v3/integer-projection.json','provenance':'provenance.json'})
 for view in ['world','map']:
  props=copy.deepcopy(layout['placements']);zone=''
  if view=='map':
   replaced={o.get('replaces_prop_id') for o in layout['overview_representations']};props=[o for o in props if o['id'] not in replaced]+layout['overview_representations'];zone=f'<polygon id="semantic-plaza" points="{points(meta["plaza_parcel"])}" fill="#7a8742" stroke="#b4a27d" stroke-width="6"/>'
  objects=''.join(f'<svg id="{o["id"]}" x="{o["x"]}" y="{o["y"]}" width="{o["width"]}" height="{o["height"]}" viewBox="{frames[o["frame"]]["x"]} {frames[o["frame"]]["y"]} {frames[o["frame"]]["width"]} {frames[o["frame"]]["height"]}" preserveAspectRatio="none"><image href="atlas.png" width="1024" height="1024"/></svg>' for o in sorted(props,key=lambda o:o['depth_y']))
  (folder/f'{view}-layout.svg').write_text(f'<svg xmlns="http://www.w3.org/2000/svg" width="768" height="1296"><title>v6 {era} {view} editable original scenery</title><image href="../ground.png" width="768" height="1296"/>{zone}{objects}</svg>')
 results.append({'theme_id':era.upper(),'bytes':ground_descriptor['bytes']+descriptor['bytes'],'pixels':W*H+1024**2,'atlas_sha256':descriptor['sha256']})
write(OUT/'resource-summary.json',{'version':6,'status':'PROPOSAL_NOT_RELEASE_BOUND','geometry_hash':proposal['geometry_hash'],'themes':results,'ground':ground_descriptor})
write(OUT/'manifest.json',{'format':'MCR_REFERENCE_SCENE_SPATIAL3_QUALITY6','version':6,'status':'CONTRACT_READY_CANDIDATE_NOT_RELEASE_BOUND','visual_acceptance':'IN_PROGRESS_NOT_ACCEPTED','geometry_revision':3,
 'geometry':{'path':'../spatial-v3/geometry-proposal.json','sha256':sha(HERE/'spatial-v3/geometry-proposal.json'),'geometry_hash':proposal['geometry_hash']},
 'integer_projection':{'path':'../spatial-v3/integer-projection.json','sha256':sha(HERE/'spatial-v3/integer-projection.json')},
 'source_metadata':{'path':'source-metadata.json','sha256':sha(OUT/'source-metadata.json')},
 'sources':[{'path':name,'sha256':sha(OUT/name),'bytes':(OUT/name).stat().st_size} for name in ['sources/venue-sheet.png','sources/stage-sheet.png','sources/surface-sheet.png','sources/prompts.md','sources/stage-prompt.md']],
 'themes':[{'theme_id':era.upper(),'files':[{'path':era+'/'+name,'sha256':sha(OUT/era/name),'bytes':(OUT/era/name).stat().st_size} for name in ['scenery.json','layout.json','map-metadata.json','resource-inputs.json','provenance.json','atlas.png']]} for era in ['revenge','parade','danger']],
 'base_revision':'023201343a452b859759e02039d367c11d1ff8ad','base_geometry_hash':'sha256:20168a1df64cd3c72ea3b8cb576ca895dcff8c7edfb0e77886cc647fb1df37e5',
 'adoption':'Requires an explicit new coordinated server/client map revision, actual static Tiled/runtime descriptor from assigned runtime/** owner, and consumer source assignment. Never mix with geometry20168. Current v4source-fixture delivery remains independent.',
 'provenance_scope':'Existing VIRTUAL_CONCEPT/CONCEPT_ONLY place records preserved exactly. Not physical destinations; no fabricated review, no blanket physical-operator prerequisite for virtual art. Source-art qualification and actual physical flows remain separate.'})
print(json.dumps(results))
