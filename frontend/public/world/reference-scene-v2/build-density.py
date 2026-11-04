#!/usr/bin/env python3
"""Fixed natural perimeter groves; explicit new geometry, no release mutation."""
import copy
import hashlib
import json
import math
from collections import deque
from pathlib import Path
from PIL import Image
from build import cell_intersects, digest

HERE=Path(__file__).resolve().parent
BASE=HERE/'quality-v7';OUT=HERE/'quality-v8';SPATIAL=HERE/'spatial-v4'
ERAS=['revenge','parade','danger']
TREES=['tree_red','tree_green','tree_yellow','tree_dark']
read=lambda p:json.loads(p.read_text())
write=lambda p,v:p.write_text(json.dumps(v,indent=2)+'\n')
sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
roundaway=lambda n:math.floor(n+.5) if n>=0 else math.ceil(n-.5)
source_path=OUT/'sources/canopy-sheet.png';source=Image.open(source_path).convert('RGBA');alpha=source.getchannel('A')
assert alpha.getextrema()==(0,255)
rects={}
for i,name in enumerate(TREES):
    left=math.floor(i%2*source.width/2);right=math.floor((i%2+1)*source.width/2)
    top=math.floor(i//2*source.height/2);bottom=math.floor((i//2+1)*source.height/2)
    b=alpha.crop((left,top,right,bottom)).point(lambda a:255 if a>=32 else 0).getbbox();assert b
    x,y=max(left,left+b[0]-2),max(top,top+b[1]-2)
    x2,y2=min(right,left+b[2]+2),min(bottom,top+b[3]+2)
    rects[name]={'x':x,'y':y,'width':x2-x,'height':y2-y}

old=read(HERE/'spatial-v3/geometry-proposal.json')
old_projection=read(HERE/'spatial-v3/integer-projection.json')
p=copy.deepcopy(old);g=p['geometry'];props=copy.deepcopy(old_projection['placements'])
points=g['spawns']+g['interactions'];point_feet=[((o['cell_x']+.5)*16,(o['cell_y']+.5)*16) for o in points]
protected_regions=[{'prop_id':'culture_park_plaque','rect':{'x':594,'y':331,'width':138,'height':63},'depth_y':411,'purpose':'Keep the Culture Park plaque text and its inner face readable; ordinary behind-sign canopy overlap is allowed.'}]
# One authored target per named group, never random placement. Only the named
# already paved outer court/exit verges allow paved tiles beneath a root.
seeds=[
 ('grove_north_west',110,125,184,170,'tree_red',[1],'Join the west red canopy band behind the court.'),
 ('grove_north_gap',520,125,176,166,'tree_red',[1],'Close the upper canopy gap between the existing rooted red/yellow trees.'),
 ('grove_east_court',735,230,160,158,'tree_yellow',[1,2],'Plant the outer court verge behind the park plaque, outside the route center.'),
 ('grove_east_middle',746,616,152,144,'tree_green',[1,2],'Join the outer eastern verge below the park plaque; keep its lettered face clear.'),
 ('grove_east_connector',740,850,142,146,'tree_green',[1],'Narrow east connecting-verge clump, keeping the food counter legible.'),
 ('grove_east_lower',735,1060,176,164,'tree_dark',[1],'Connect the lower east garden perimeter.'),
 ('grove_east_exit',735,1220,164,154,'tree_yellow',[1,2],'Frame the outer exit verge without moving the giveaway destination.'),
 ('grove_west_upper',35,260,174,166,'tree_green',[1],'Join the upper west perimeter planting.'),
 ('grove_west_middle',50,500,184,170,'tree_yellow',[1],'Connect west canopy groups above the merch hall.'),
 ('grove_west_lower',45,850,182,174,'tree_green',[1],'Fill the lower west lawn edge beside the connecting road.'),
 ('grove_west_bottom',45,970,172,164,'tree_dark',[1],'Join the lower west canopy chain before the existing yellow tree.'),
 ('grove_south_lawn',365,1190,190,174,'tree_green',[1],'One rooted lower lawn group between the southwest edge and giveaway parcel.'),
]
intent=[]
def footprint(cx,cy):return [[cx-16,cy-14],[cx+16,cy-14],[cx+19,cy],[cx-19,cy]]
def occludes_named_foot(obj):
    r=rects[obj['frame']]
    for x,y in point_feet:
        if not (y<obj['depth_y'] and obj['x']<=x<obj['x']+obj['width'] and obj['y']<=y<obj['y']+obj['height']):continue
        sx=min(r['width']-1,int((x-obj['x'])*r['width']/obj['width']));sy=min(r['height']-1,int((y-obj['y'])*r['height']/obj['height']))
        if alpha.getpixel((r['x']+sx,r['y']+sy))>128:return True
    return False
def covers_protected_text(obj):
    r=rects[obj['frame']]
    for region in protected_regions:
        if obj['depth_y']<region['depth_y']:continue
        box=region['rect']
        for y in range(max(obj['y'],box['y']),min(obj['y']+obj['height'],box['y']+box['height'])):
            for x in range(max(obj['x'],box['x']),min(obj['x']+obj['width'],box['x']+box['width'])):
                sx=min(r['width']-1,int((x-obj['x'])*r['width']/obj['width']));sy=min(r['height']-1,int((y-obj['y'])*r['height']/obj['height']))
                if alpha.getpixel((r['x']+sx,r['y']+sy))>128:return True
    return False
for ident,sx,sy,width,height,frame,terrains,reason in seeds:
    candidates=[]
    for cy in range(1,80):
        for cx in range(1,47):
            x,y=(cx+.5)*16,(cy+.5)*16
            if (x-sx)**2+(y-sy)**2>64**2:continue
            if any((x-fx)**2+(y-fy)**2<80**2 for fx,fy in point_feet):continue
            foot=footprint(x,y)
            cells=[(a,b) for b in range(max(0,cy-2),min(81,cy+2)) for a in range(max(0,cx-2),min(48,cx+2)) if cell_intersects(a,b,foot)]
            if any(a in [0,47] or b in [0,80] or g['collision_grid'][b][a] or g['terrain_layer'][b][a] not in terrains for a,b in cells):continue
            if any(cell_intersects(a,b,o['footprint']) for a,b in cells for o in props[len(old_projection['placements']):]):continue
            obj={'id':ident,'frame':frame,'x':roundaway(x-width*.52),'y':int(y-height),'width':width,'height':height,'depth_y':int(y),'footprint':foot,'interaction_id':None,'placement_reason':reason+' Unbound virtual decoration; root is solid, canopy uses ordinary depth occlusion.'}
            if occludes_named_foot(obj) or covers_protected_text(obj):continue
            candidates.append(((x-sx)**2+(y-sy)**2,abs(x-sx),y,x,obj,cells))
    assert candidates,('No safe root within authored64px radius',ident)
    chosen=min(candidates,key=lambda c:c[:4]);obj,cells=chosen[4:];props.append(obj)
    intent.append({'id':ident,'target_root':[sx,sy],'root':[chosen[3],chosen[2]],'delta':[chosen[3]-sx,chosen[2]-sy],'reason':reason,'allowed_terrain_ids':terrains,'root_collision_cells':[list(c) for c in cells],'policy':'Nearest deterministic valid grid-center root within64px; footprint clear of existing roots/boundary, >=80px from named feet, no named-foot alpha occlusion. No random stamping.'})
g['collision_grid']=[[int(x in [0,47] or y in [0,80] or any(cell_intersects(x,y,o['footprint']) for o in props)) for x in range(48)] for y in range(81)]
g['collision_id']='reference_scene_v4_natural_perimeter_proposal_collision'
p.update(proposal_version=4,status='PROPOSAL_NOT_RELEASE_BOUND',worker_base='12f6fbf880fab927be130656e05c406f0084d0c1',placements=props,geometry_hash=digest(g),overview_representations=copy.deepcopy(old_projection['overview_representations']),protected_art_regions=protected_regions)
p['revision_reason']='Required target density: natural canopy clusters and twelve fixed unbound rooted verge/lawn groups replace sparse isolated ornamental-tree presentation. New collision revision; old place/spawn/interaction cells/cameras/ground/roads remain unchanged.'
p['resources']=[]
p['limitations']=['Virtual-art geometry candidate, not physical venue/routing/arrival evidence.','No new place identity or interaction. Four truthful virtual-concept places unchanged.','Needs explicit new canonical runtime/server geometry and resource binding by their owners; never mix with spatial-v3/map4/v6.','Named-foot alpha samples and BFS do not certify moving-avatar or multiuser behavior.','No fourth critic score or asserted reference acceptance.']
SPATIAL.mkdir(exist_ok=True);write(SPATIAL/'geometry-proposal.json',p)
write(SPATIAL/'integer-projection.json',{'version':4,'geometry_hash':p['geometry_hash'],'policy':'Retain frozen v3 integer destinations/float footprints; all new destination numbers are explicitly authored integers at chosen roots. No consumer rerounding.','placements':props,'overview_representations':p['overview_representations']})
routes=[]
for spawn in g['spawns']:
    start=spawn['cell_x'],spawn['cell_y'];dist={start:0};queue=deque([start])
    while queue:
        x,y=queue.popleft()
        for a,b in [(x-1,y),(x+1,y),(x,y-1),(x,y+1)]:
            if 0<=a<48 and 0<=b<81 and not g['collision_grid'][b][a] and (a,b) not in dist:dist[a,b]=dist[x,y]+1;queue.append((a,b))
    for q in points:
        key=q['cell_x'],q['cell_y'];assert key in dist,(spawn,q)
        routes.append({'from':spawn['spawn_id'],'to':q.get('spawn_id',q.get('interaction_id')),'steps':dist[key]})
changed_cells=[{'cell_x':x,'cell_y':y,'previous':old['geometry']['collision_grid'][y][x],'proposed':g['collision_grid'][y][x]} for y in range(81) for x in range(48) if old['geometry']['collision_grid'][y][x]!=g['collision_grid'][y][x]]
write(SPATIAL/'revision-evidence.json',{'status':'PASS_VIRTUAL_DENSITY_CANDIDATE_ONLY','base_geometry_hash':old['geometry_hash'],'geometry_hash':p['geometry_hash'],'planting_intent':intent,'old_props':22,'new_unbound_rooted_props':len(seeds),'physical_props':len(props),'new_place_ids':0,'unchanged':['existing22placements/footprints','terrain/paving/ground','spawns/interactions/cameras','sixMAPrepresentations/labels/fourplaces'],'changed_cells':changed_cells,'routes':routes})

results=[]
for era in ERAS:
    old_folder=BASE/era;folder=OUT/era;folder.mkdir(exist_ok=True)
    layout=read(old_folder/'layout.json');atlas=Image.open(old_folder/'atlas.png').convert('RGBA');transforms=[]
    for t in read(old_folder/'provenance.json')['transforms']:
        t=copy.deepcopy(t)
        if t['frame'] not in TREES:
            if t['source'].startswith('../sources/'):t['source']='../../quality-v7/'+t['source'][3:]
            transforms.append(t);continue
        name=t['frame'];old_frame=layout['frames'][name];slot_x,slot_y=old_frame['x']//256*256,old_frame['y']//256*256
        atlas.paste((0,0,0,0),(slot_x,slot_y,slot_x+256,slot_y+256))
        r=rects[name];scale=min(244/r['width'],244/r['height']);w,h=roundaway(r['width']*scale),roundaway(r['height']*scale)
        frame={'x':slot_x+(256-w)//2,'y':slot_y+(256-h)//2,'width':w,'height':h}
        crop=source.crop((r['x'],r['y'],r['x']+r['width'],r['y']+r['height'])).resize((w,h),Image.Resampling.NEAREST);atlas.paste(crop,(frame['x'],frame['y']))
        layout['frames'][name]=frame;transforms.append({'frame':name,'source':'../sources/canopy-sheet.png','source_rect':r,'packed_rect':frame,'scale':scale,'resampling':'nearest','rounding':'nearest ties away from zero'})
    atlas.save(folder/'atlas.png',optimize=True,compress_level=9)
    native=Image.new('RGBA',(2048,2048));native.paste(atlas,(0,0));native.save(folder/'native-atlas.png',optimize=True,compress_level=9)
    layout.update(version=8,geometry_revision=4,geometry_hash=p['geometry_hash'],placements=copy.deepcopy(props),overview_representations=p['overview_representations'],ground='../../quality-v6/ground.png',skins='Three era-specific venue/stage/booth atlases; shared original neutral natural canopy quartet.',runtime_derivative='Original natural canopy quartet, twelve fixed unbound rooted perimeter groups, source-preserving nearest atlas packing; explicit new spatial-v4 collision geometry. Frozen v6/v7 unchanged.')
    write(folder/'layout.json',layout)
    scenery=read(old_folder/'scenery.json');aid=era+'_quality_v8_atlas';descriptor={'id':aid,'path':f'world/reference-scene-v2/quality-v8/{era}/atlas.png','sha256':sha(folder/'atlas.png'),'width_px':1024,'height_px':1024,'bytes':(folder/'atlas.png').stat().st_size};scenery['atlases']=[descriptor]
    template=copy.deepcopy(next(o for o in scenery['objects'] if o['id']=='rooted_tree_01'));objects=[]
    by_id={o['id']:o for o in scenery['objects']}
    for o in props:
        obj=copy.deepcopy(by_id.get(o['id'],template));obj.update(id=o['id'],atlas_id=aid,frame=layout['frames'][o['frame']],**{k:o[k] for k in ['x','y','width','height','depth_y']});objects.append(obj)
    scenery['objects']=objects;write(folder/'scenery.json',scenery)
    meta=read(old_folder/'map-metadata.json');meta.update(version=8,geometry_hash=p['geometry_hash']);write(folder/'map-metadata.json',meta)
    provenance={'version':8,'status':'PROPOSAL_NOT_RELEASE_BOUND','theme_id':era.upper(),'geometry_hash':p['geometry_hash'],'base_revision':'12f6fbf880fab927be130656e05c406f0084d0c1','source_pins':[{'path':'../sources/canopy-sheet.png','sha256':sha(source_path)},{'path':'../../quality-v7/manifest.json','sha256':sha(BASE/'manifest.json')},{'path':'../../quality-v7/'+era+'/atlas.png','sha256':sha(old_folder/'atlas.png')}],'transforms':transforms,'native_packaging':{'path':'native-atlas.png','sha256':sha(folder/'native-atlas.png'),'width':2048,'height':2048,'method':'Runtime source at0,0 with transparent native-only padding.'},'geometry':{'path':'../../spatial-v4/geometry-proposal.json','sha256':sha(SPATIAL/'geometry-proposal.json'),'projection':'../../spatial-v4/integer-projection.json'},'source_scope':'Twelve non-tree cells are pixel-identical v7. New natural canopy quartet shared by all eras; other stage/venue/booth identity art remains era-specific.','physical_provenance':'Existing four virtual-concept places unchanged; new trees are unbound virtual decoration, never physical destinations.','fidelity':'Unscored required density correction, not target acceptance.'};write(folder/'provenance.json',provenance)
    resources=read(old_folder/'resource-inputs.json');resources.update(version=8,geometry_hash=p['geometry_hash'],resources=[scenery['ground'],descriptor],total_bytes=scenery['ground']['bytes']+descriptor['bytes'],projection='../../spatial-v4/integer-projection.json');assert resources['total_bytes']<=2097152;write(folder/'resource-inputs.json',resources)
    points_svg=lambda ps:' '.join(f'{x},{y}' for x,y in ps)
    for view in ['world','map']:
        chosen=copy.deepcopy(props);zone=''
        if view=='map':
            replaced={o.get('replaces_prop_id') for o in layout['overview_representations']};chosen=[o for o in chosen if o['id'] not in replaced]+layout['overview_representations'];zone=f'<polygon id="semantic-plaza" points="{points_svg(meta["plaza_parcel"])}" fill="#7a8742" stroke="#b4a27d" stroke-width="6"/>'
        objects=''.join(f'<svg id="{o["id"]}" x="{o["x"]}" y="{o["y"]}" width="{o["width"]}" height="{o["height"]}" viewBox="{layout["frames"][o["frame"]]["x"]} {layout["frames"][o["frame"]]["y"]} {layout["frames"][o["frame"]]["width"]} {layout["frames"][o["frame"]]["height"]}" preserveAspectRatio="none"><image href="atlas.png" width="1024" height="1024"/></svg>' for o in sorted(chosen,key=lambda o:o['depth_y']))
        (folder/f'{view}-layout.svg').write_text(f'<svg xmlns="http://www.w3.org/2000/svg" width="768" height="1296"><title>v8 {era} {view}: natural perimeter groves, explicit spatial-v4</title><image href="../../quality-v6/ground.png" width="768" height="1296"/>{zone}{objects}</svg>')
    results.append({'theme_id':era.upper(),'bytes':resources['total_bytes'],'pixels':resources['total_pixels'],'atlas_sha256':descriptor['sha256']})
write(OUT/'source-metadata.json',{'version':8,'status':'ORIGINAL_ART_NOT_REFERENCE_ACCEPTED','source_sheet':{'path':'sources/canopy-sheet.png','sha256':sha(source_path),'bytes':source_path.stat().st_size,'width':source.width,'height':source.height},'canopy_frames':rects,'prompt':'sources/prompt.md','base_manifest':{'path':'../quality-v7/manifest.json','sha256':sha(BASE/'manifest.json')},'geometry_hash':p['geometry_hash']})
write(OUT/'resource-summary.json',{'version':8,'status':'PROPOSAL_NOT_RELEASE_BOUND','geometry_hash':p['geometry_hash'],'themes':results,'ground':read(BASE/'resource-summary.json')['ground']})
write(OUT/'manifest.json',{'format':'MCR_REFERENCE_SCENE_SPATIAL4_QUALITY8','version':8,'status':'CONTRACT_READY_CANDIDATE_NOT_RELEASE_BOUND','visual_acceptance':'IN_PROGRESS_NOT_ACCEPTED','geometry_revision':4,'geometry':{'path':'../spatial-v4/geometry-proposal.json','sha256':sha(SPATIAL/'geometry-proposal.json'),'geometry_hash':p['geometry_hash']},'integer_projection':{'path':'../spatial-v4/integer-projection.json','sha256':sha(SPATIAL/'integer-projection.json')},'geometry_revision_evidence':{'path':'../spatial-v4/revision-evidence.json','sha256':sha(SPATIAL/'revision-evidence.json')},'source_metadata':{'path':'source-metadata.json','sha256':sha(OUT/'source-metadata.json')},'sources':[{'path':n,'sha256':sha(OUT/n),'bytes':(OUT/n).stat().st_size} for n in ['sources/canopy-sheet.png','sources/prompt.md']],'themes':[{'theme_id':era.upper(),'files':[{'path':era+'/'+n,'sha256':sha(OUT/era/n),'bytes':(OUT/era/n).stat().st_size} for n in ['scenery.json','layout.json','map-metadata.json','resource-inputs.json','provenance.json','atlas.png']]} for era in ERAS],'base_revision':'12f6fbf880fab927be130656e05c406f0084d0c1','adoption':'New spatial-v4 collision and34objects. Requires actual new canonical Tiled/server/client geometry/resource revision from assigned owners. No runtime/** or backend edits here, no silent activation/mixing with map4/v6.','adoption_requirements':{'map_id':'mcr_reference_scene_r3','current_published_map_revision':4,'required_new_map_revision_minimum':5,'proposed_next_map_revision':5,'allocation':'Proposal only, not a reserved/published release; responsible backend/runtime owners confirm the actual revision before binding. Existing map4 and its tilemap/resource/source pins remain frozen.','new_geometry_hash':p['geometry_hash'],'new_collision_cells':len(changed_cells),'new_physical_props':len(props),'actual_canonical_tiled':'NOT_RUN - reserved runtime/** owner publishes it only under an explicit new assignment.','root_impact':'No implicit task switch: current spatial-v3 Root publication and reserved FE09 serial handoff remain independent. New source composition needs an explicit later consumer assignment.'},'provenance_scope':'No new place identities, no physical review/arrival claims. All new grove props are unbound virtual decoration with explicit collision and planted intent.'})
print(json.dumps({'geometry_hash':p['geometry_hash'],'props':len(props),'routes':len(routes),'planting_roots':[{k:o[k] for k in ['id','root','delta']} for o in intent],'resources':results}))
