#!/usr/bin/env python3
"""Author a separately versioned, virtual-only shared WORLD/MAP geometry candidate."""
import copy,json,math
from collections import deque
from pathlib import Path
from build import cell_intersects,inside,digest
HERE=Path(__file__).resolve().parent;OUT=HERE/'spatial-v3';OUT.mkdir(exist_ok=True)
read=lambda p:json.loads(p.read_text());write=lambda p,v:p.write_text(json.dumps(v,indent=2)+'\n')
old=read(HERE/'geometry-proposal.json');base=read(HERE/'eras-v5/parade/layout.json');p=copy.deepcopy(old)
def ymap(y):return y-179 if y<=954 else 775+(y-954)*521/342
def roundaway(n):return math.floor(n+.5) if n>=0 else math.ceil(n-.5)
def ground_polygon(poly):
 result=[]
 for a,b in zip(poly,poly[1:]+poly[:1]):
  result.append([a[0],round(ymap(a[1]),6)])
  if (a[1]-954)*(b[1]-954)<0:
   t=(954-a[1])/(b[1]-a[1]);result.append([round(a[0]+t*(b[0]-a[0]),6),775])
 return result
props=[];deltas=[]
for o in base['placements']:
 n=copy.deepcopy(o)
 if n['id']=='rooted_tree_02':n['frame']='tree_red'
 new_depth=96 if o['id'] in ['northern_hall','north_eastern_hall'] else ymap(o['depth_y']);delta=new_depth-o['depth_y']
 n['y']=round(o['y']+delta,6);n['depth_y']=round(new_depth,6);n['footprint']=[[x,round(y+delta,6)] for x,y in o['footprint']]
 assert all(0<=x<=768 and 0<=y<=1296 for x,y in n['footprint']),n['id']
 assert n['y']>=-128 and n['y']+n['height']<=1424,n['id']
 props.append(n);deltas.append({'id':o['id'],'old_depth_y':o['depth_y'],'new_depth_y':n['depth_y'],'delta_y':round(delta,6),'exception':'Top hall rooted depth96 keeps source art within C06 -128px overhang.' if o['id'] in ['northern_hall','north_eastern_hall'] else None})
w,h=48,81
collision=[[int(x in [0,w-1] or y in [0,h-1] or any(cell_intersects(x,y,o['footprint']) for o in props)) for x in range(w)] for y in range(h)]
g=copy.deepcopy(old['geometry']);g['collision_id']='reference_scene_v3_proposal_collision';g['collision_grid']=collision
paving=[ground_polygon(poly) for poly in old['paving_polygons']]
g['terrain_layer']=[[2 if any(inside((x+.5)*16,(y+.5)*16,poly) for poly in paving) else 1 for x in range(w)] for y in range(h)]
g['overdraw_layer']=[[0]*w for _ in range(h)]
point_projection=[]
for group in ['spawns','interactions']:
 for point in g[group]:
  previous=point.copy();target_x=point['cell_x'];target_y=math.floor(ymap((point['cell_y']+.5)*16)/16)
  candidates=[(x,y) for y in range(max(1,target_y-2),min(h-2,target_y+2)+1) for x in range(max(1,target_x-2),min(w-2,target_x+2)+1) if not collision[y][x]]
  assert candidates,previous
  x,y=min(candidates,key=lambda c:((c[0]-target_x)**2+(c[1]-target_y)**2,abs(c[0]-target_x),c[1],c[0]))
  point.update(cell_x=x,cell_y=y);point_projection.append({'group':group,'previous':previous,'proposed':point.copy(),'rule':'Floor transformed cell-center into grid; nearest walkable cell within2 cells, deterministic distance/x/y tie break. Virtual geometry only.'})
reps=copy.deepcopy(base['overview_representations'])
for o in reps:
 depth=ymap(o['depth_y']);o['y']=round(o['y']+depth-o['depth_y'],6);o['depth_y']=round(depth,6)
 if o['id']=='stage_venue_representation':o.update(x=185,y=163,width=300,height=210,depth_y=373)
labels=copy.deepcopy(old['map_labels'])
for o in labels:o['y']=round(ymap(o['y']),6)
next(o for o in labels if o['interaction_id']=='stage_info').update(x=335,y=147)
p.update(proposal_version=3,status='PROPOSAL_NOT_RELEASE_BOUND',worker_base='023201343a452b859759e02039d367c11d1ff8ad',geometry=g,geometry_hash=digest(g),placements=props,paving_polygons=paving,plaza_parcel=ground_polygon(old['plaza_parcel']),overview_representations=reps,map_labels=labels)
p['world_camera']=dict(old['world_camera'],y=13,notes='Same close-view composition after upper-district translation179px; not a player teleport or physical position claim.')
p['decorative_ground_beds']=[{'cx':x,'cy':round(ymap(y),6),'rx':rx,'ry':ry} for x,y,rx,ry in [(48,80,40,35),(708,750,42,70),(45,1030,44,70),(650,1100,38,56),(385,1215,86,38),(400,955,35,40),(595,865,40,38)]]
p['resources']=[]
p['revision_reason']='Required target fidelity: upper MAPdistrict was too low. Upper root/court translation and continuous lower redistribution preserve WORLD crop while producing a coherent single physical geometry. New explicit candidate; no existing release may mix these coordinates with old collision.'
p['limitations']=['Virtual concept proposal; no physical arrival, safe routing, event layout or venue authorization.','All existing place IDs/values/provenance unchanged. Concept-only places do not require fabricated physical review.','Needs explicit coordinated server/client map+theme revision adoption; published v4/v5 remain unchanged.','Art generation and developer checks do not establish reference acceptance or a formal source-review contract.']
assert p['places']==old['places']
write(OUT/'geometry-proposal.json',p)
# Explicit integer destinations for C06 and native consumers; original float footprints retained.
projected=copy.deepcopy(props);projected_reps=copy.deepcopy(reps);projection=[]
for group,objects in [('placements',projected),('overview_representations',projected_reps)]:
 for o in objects:
  authored={k:o[k] for k in ['x','y','width','height','depth_y']};local=o['depth_y']-o['y'];o.update({k:roundaway(o[k]) for k in ['x','width','height','depth_y']});o['y']=o['depth_y']-roundaway(local)
  projection.append({'group':group,'id':o['id'],'authored':authored,'runtime':{k:o[k] for k in authored},'authored_local_foot_y':local,'runtime_local_foot_y':roundaway(local),'delta':{k:round(o[k]-v,6) for k,v in authored.items()}})
write(OUT/'integer-projection.json',{'version':3,'policy':'Nearest integer, ties away from zero; Y=rounded depth minus rounded local foot anchor. Footprints never rounded.','geometry_hash':p['geometry_hash'],'objects':projection,'placements':projected,'overview_representations':projected_reps})
# Required geometry proofs: exact solid coverage and twelve named reachability routes.
routes=[];points=g['spawns']+g['interactions']
for spawn in g['spawns']:
 start=(spawn['cell_x'],spawn['cell_y']);dist={start:0};q=deque([start])
 while q:
  x,y=q.popleft()
  for a,b in [(x-1,y),(x+1,y),(x,y-1),(x,y+1)]:
   if 0<=a<w and 0<=b<h and not collision[b][a] and (a,b) not in dist:dist[a,b]=dist[x,y]+1;q.append((a,b))
 for point in points:
  key=(point['cell_x'],point['cell_y']);assert key in dist,(spawn,point)
  routes.append({'from':spawn['spawn_id'],'to':point.get('spawn_id',point.get('interaction_id')),'steps':dist[key]})
write(OUT/'revision-evidence.json',{'status':'PASS_VIRTUAL_GEOMETRY_CANDIDATE_ONLY','base_geometry_hash':old['geometry_hash'],'geometry_hash':p['geometry_hash'],'world_camera_delta_y':-179,'transformation':'RootY<=954: Y-179; largerY:775+(Y-954)*521/342. Preserve each prop footprint/body shape by root-relative translation; explicit top-hall exceptiondepth96. Ground polygons use continuous same transform with kink vertices.','prop_deltas':deltas,'point_projection':point_projection,'routes':routes,'existing_place_records_unchanged':True,'limits':'No release activation, physical validation or target acceptance.'})
print(json.dumps({'status':'PASS_CANDIDATE_ONLY','geometry_hash':p['geometry_hash'],'routes':len(routes),'points':points,'world_camera':p['world_camera']}))
