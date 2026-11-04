#!/usr/bin/env python3
"""Recompute every terrain/collision cell and reachable destination in spatial-v5."""
import hashlib,json,math
from collections import deque
from pathlib import Path
from build import cell_intersects
HERE=Path(__file__).resolve().parent;OUT=HERE/'spatial-v5'
read=lambda p:json.loads(p.read_text())
p=read(OUT/'geometry-proposal.json');old=read(HERE/'spatial-v4/geometry-proposal.json');g=p['geometry'];projection=read(OUT/'integer-projection.json');evidence=read(OUT/'revision-evidence.json')
actual='sha256:'+hashlib.sha256(json.dumps(g,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()).hexdigest()
target=HERE.parents[3]/p['target']['path'];assert 'sha256:'+hashlib.sha256(target.read_bytes()).hexdigest()==p['target']['sha256']
assert actual==p['geometry_hash']==projection['geometry_hash']!=old['geometry_hash']
assert {k:v for k,v in g.items() if k!='terrain_layer'}=={k:v for k,v in old['geometry'].items() if k!='terrain_layer'}
for k in ['placements','world_camera','overview_camera','map_labels','places','plaza_parcel']:
 assert p[k]==old[k],k
assert len(p['placements'])==34 and p['placements']==projection['placements']
assert p['overview_representations']==projection['overview_representations']==read(HERE/'quality-v9/parade/layout.json')['overview_representations']
assert p['decorative_ground_beds']==[]
assert (g['width_cells'],g['height_cells'],g['tile_width_px'],g['tile_height_px'])==(48,81,16,16)
def contained(x,y,poly):
 count=0
 for a,b in zip(poly,poly[1:]+poly[:1]):
  if (a[1]>y)!=(b[1]>y) and x<(b[0]-a[0])*(y-a[1])/(b[1]-a[1])+a[0]:count+=1
 return count%2==1
changed=[]
for y in range(81):
 for x in range(48):
  terrain=2 if any(contained((x+.5)*16,(y+.5)*16,q) for q in p['paving_polygons']) else 1
  assert g['terrain_layer'][y][x]==terrain,(x,y)
  solid=int(x in [0,47] or y in [0,80] or any(cell_intersects(x,y,o['footprint']) for o in p['placements']))
  assert g['collision_grid'][y][x]==solid
  if terrain!=old['geometry']['terrain_layer'][y][x]:changed.append({'cell_x':x,'cell_y':y,'previous':old['geometry']['terrain_layer'][y][x],'proposed':terrain})
assert len(changed)==560 and changed==evidence['terrain_changed_cells']
assert evidence['collision_changed_cells']==0 and evidence['world_props_changed']==0
routes=[]
for spawn in g['spawns']:
 start=(spawn['cell_x'],spawn['cell_y']);dist={start:0};q=deque([start])
 while q:
  x,y=q.popleft()
  for cell in [(x-1,y),(x+1,y),(x,y-1),(x,y+1)]:
   a,b=cell
   if 0<=a<48 and 0<=b<81 and not g['collision_grid'][b][a] and cell not in dist:dist[cell]=dist[(x,y)]+1;q.append(cell)
 for target in g['spawns']+g['interactions']:
  cell=(target['cell_x'],target['cell_y']);assert cell in dist
  routes.append({'from':spawn['spawn_id'],'to':target.get('spawn_id',target.get('interaction_id')),'steps':dist[cell]})
assert routes==evidence['routes'] and len(routes)==12
# Independent distance computation for explicitly approximate manual annotations.
anchors=evidence['reference_route_anchors']['anchors'];line=p['primary_route']['centerline'];distances=[]
for anchor in anchors:
 x,y=anchor['reference_pixel'];point=((x-975)*768/248,(y-95)*1296/420)
 assert list(point)==anchor['map_point']
 candidates=[]
 for a,b in zip(line,line[1:]):
  length=math.dist(a,b);t=max(0,min(length,((point[0]-a[0])*(b[0]-a[0])+(point[1]-a[1])*(b[1]-a[1]))/length))
  candidates.append(math.dist(point,(a[0]+t*(b[0]-a[0])/length,a[1]+t*(b[1]-a[1])/length)))
 d=min(candidates);assert abs(d-anchor['new_primary_distance'])<1e-9 and d<16
 assert anchor['old_primary_distance']>30 and d<anchor['old_primary_distance'];distances.append(d)
report={'status':'PASS_SPATIAL_V5_CANDIDATE_ONLY','geometry_hash':actual,'terrain_cells_recomputed':3888,'collision_cells_sat_checked':3888,'changed_terrain_cells':len(changed),'changed_collision_cells':0,'world_props':34,'world_props_changed':0,'routes':routes,'manual_anchor_distance_px':distances,'limits':['Manual approximate semantic annotations are not independent whole-reference acceptance.','New terrain requires a new canonical server/client map revision greater than allocated5; proposed6 is not allocated by art.','No physical arrival, runtime release, source review or application acceptance.']}
(OUT/'verification.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k!='routes'}))
