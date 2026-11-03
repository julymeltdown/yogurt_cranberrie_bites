#!/usr/bin/env python3
"""Explicit non-generative runtime packing; immutable source sheets are retained."""
import hashlib
import json
import math
import copy
from pathlib import Path
from PIL import Image

def nearest(value):
    # Explicit artist-owned projection, including signed MAP-edge coordinates.
    return math.floor(value + .5) if value >= 0 else math.ceil(value - .5)

HERE=Path(__file__).resolve().parent
OUT=HERE/'runtime-v4';OUT.mkdir(exist_ok=True)
source_layout=json.loads((HERE/'painted/layout.json').read_text())
source_path=HERE/'painted'/source_layout['atlas']
source=Image.open(source_path).convert('RGBA')
atlas=Image.new('RGBA',(1024,1024));frames={};transforms=[]
for i,(name,f) in enumerate(source_layout['frames'].items()):
    rect={k:f[k] for k in ['x','y','width','height']}
    if name=='cross_banner':
        rect={'x':695,'y':934,'width':170,'height':320}
    crop=source.crop((rect['x'],rect['y'],rect['x']+rect['width'],rect['y']+rect['height']))
    width,height=nearest(rect['width']*.75),nearest(rect['height']*.75)
    assert width<=244 and height<=244
    crop=crop.resize((width,height),Image.Resampling.NEAREST)
    x=(i%4)*256+(256-width)//2;y=(i//4)*256+(256-height)//2
    atlas.paste(crop,(x,y))
    frames[name]={'x':x,'y':y,'width':width,'height':height}
    transforms.append({'frame':name,'source_rect':rect,'packed_rect':frames[name],'scale':.75,'resampling':'nearest','correction':'Tighter source-only banner crop excludes neighboring furniture spill.' if name=='cross_banner' else None})
atlas.save(OUT/'parade-atlas.png',optimize=True,compress_level=9)
ground=Image.open(HERE/'painted/parade-ground.png')
if ground.mode=='RGBA':assert ground.getchannel('A').getextrema()==(255,255);ground=ground.convert('RGB')
ground.save(OUT/'parade-ground.png',optimize=True,compress_level=9)
layout=copy.deepcopy(source_layout);layout.update(version=4,ground='parade-ground.png',atlas='parade-atlas.png',frames=frames,source_atlas_dimensions=[1024,1024])
projection=[]
for group in ['placements','overview_representations']:
    for index, obj in enumerate(layout[group]):
        authored={k:obj[k] for k in ['x','y','width','height','depth_y']}
        local_foot=authored['depth_y']-authored['y']
        obj.update({k:nearest(authored[k]) for k in ['x','width','height','depth_y']})
        obj['y']=obj['depth_y']-nearest(local_foot)
        runtime={k:obj[k] for k in authored}
        projection.append({'group':group,'index':index,'id':obj['id'],'authored':authored,'runtime':runtime,
            'authored_local_foot_y':local_foot,'runtime_local_foot_y':nearest(local_foot),
            'delta':{k:round(runtime[k]-authored[k],8) for k in authored},
            'foot_anchor_delta':round(nearest(local_foot)-local_foot,8)})
layout['runtime_derivative']='Versioned 0.75 nearest-neighbor atlas derivative; lossless RGB ground re-encoding; explicit integer destination/foot-depth projection. Source sheets and physical geometry unchanged.'
policy={'rounding':'nearest integer, ties away from zero',
    'foot_depth':'runtime depth_y = round(authored depth_y)',
    'local_foot_anchor':'runtime local foot = round(authored depth_y - authored y); runtime y = runtime depth_y - runtime local foot',
    'extent':'x, width and height round independently; rectangles are visual only',
    'ordering':'stable ascending depth_y, source array order breaks equal-depth ties',
    'geometry':'No changes to footprint polygons, collision/terrain/overdraw, IDs, spawn or interaction cells.',
    'map':'Overview representations remain MAP-only semantic art; never add them as C06 WORLD objects.'}
(OUT/'parade-integer-projection.json').write_text(json.dumps({'version':4,'policy':policy,'objects':projection},indent=2)+'\n')
(OUT/'parade-layout.json').write_text(json.dumps(layout,indent=2)+'\n')
sha=lambda file:'sha256:'+hashlib.sha256(file.read_bytes()).hexdigest()
resources=[]
for name in ['parade-ground.png','parade-atlas.png']:
    image=Image.open(OUT/name);resources.append({'path':name,'width':image.width,'height':image.height,'pixels':image.width*image.height,'bytes':(OUT/name).stat().st_size,'sha256':sha(OUT/name)})
total_pixels=sum(r['pixels'] for r in resources);total_bytes=sum(r['bytes'] for r in resources)
def descriptor(r, identity):
    return {'id':identity,'path':'world/reference-scene-v2/runtime-v4/'+r['path'],'sha256':r['sha256'],
        'width_px':r['width'],'height_px':r['height'],'bytes':r['bytes']}
atlas_id='parade_runtime_v4_atlas'
scenery={'version':1,'ground':descriptor(resources[0],'parade_runtime_v4_ground'),
    'atlases':[descriptor(resources[1],atlas_id)],
    'objects':[{'id':o['id'],'atlas_id':atlas_id,'frame':frames[o['frame']],
        **{k:o[k] for k in ['x','y','width','height','depth_y']}} for o in layout['placements']]}
(OUT/'parade-scenery.json').write_text(json.dumps(scenery,indent=2)+'\n')
proposal=json.loads((HERE/'geometry-proposal.json').read_text())
map_metadata={'version':4,'geometry_hash':layout['geometry_hash'],'world_scenery':'parade-scenery.json',
    'layout':'parade-layout.json','overview_representations':layout['overview_representations'],
    'overview_zone_policy':layout['overview_zone_policy'],'zone_rendering':{'mode':'solid','fill':'#7a8742','fill_rgba_native':[.48,.53,.26,1],'stroke':'#b4a27d','stroke_width_px':6,'optional_ground_sample':{'x':0,'y':0,'width':48,'height':48},'resources':'Use the already loaded ground and atlas only; no surface-source fetch.'},'plaza_parcel':proposal['plaza_parcel'],
    'map_labels':proposal['map_labels'],'places':proposal['places'],
    'camera':layout['overview_camera'],'authority':'Semantic MAP rendering only. Geometry/IDs remain in the pinned proposal; null label bindings stay unbound.'}
(OUT/'parade-map-metadata.json').write_text(json.dumps(map_metadata,indent=2)+'\n')
manifest={'format':'MCR_RUNTIME_SCENE_DERIVATIVE','version':4,'status':'PROPOSAL_NOT_RELEASE_BOUND','theme_id':'PARADE','geometry_url':'../geometry-proposal.json','geometry_hash':layout['geometry_hash'],'layout':'parade-layout.json','scenery':'parade-scenery.json','integer_projection':'parade-integer-projection.json','map_metadata':'parade-map-metadata.json','projection_policy':policy,'source_revision':'8f89d1905a86143d34ff77d2951fc60651ab2497','sources':[{'path':'../painted/'+source_layout['atlas'],'sha256':sha(source_path)},{'path':'../painted/parade-ground.png','sha256':sha(HERE/'painted/parade-ground.png')}],'resources':resources,'transforms':transforms,'total_pixels':total_pixels,'total_bytes':total_bytes,'limits':{'combined_scenery_pixels':2097152,'combined_scenery_bytes':2097152},'budget_pass':total_pixels<=2097152 and total_bytes<=2097152,'other_skins':'Painted REVENGE/DANGER unavailable; never alias to PARADE.','review_limit':'Original third-round scores apply to frozen v3; runtime packing has separate comparison evidence, not a fourth critic score.'}
(OUT/'parade-resource-inputs.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'pixels':total_pixels,'bytes':total_bytes,'budget_pass':manifest['budget_pass'],'resources':resources}))
