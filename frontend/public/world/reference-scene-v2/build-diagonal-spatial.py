#!/usr/bin/env python3
"""Continuous target-comparison southwest approach, explicit terrain revision."""
import copy,json,math
from collections import deque
from pathlib import Path
from build import corridor,inside,digest
HERE=Path(__file__).resolve().parent;OUT=HERE/'spatial-v5';OUT.mkdir(exist_ok=True)
read=lambda p:json.loads(p.read_text());write=lambda p,v:p.write_text(json.dumps(v,indent=2)+'\n')
old=read(HERE/'spatial-v4/geometry-proposal.json');p=copy.deepcopy(old);g=p['geometry']
primary=[[-20,1030],[310,741],[557,494],[800,300]]
lower=[[-20,1030],[180,915],[180,1320]]
paving=corridor(primary,43)+copy.deepcopy(old['paving_polygons'][9:16])+copy.deepcopy(old['paving_polygons'][23:])+corridor(lower,32)
g['terrain_layer']=[[2 if any(inside((x+.5)*16,(y+.5)*16,poly) for poly in paving) else 1 for x in range(48)] for y in range(81)]
g_hash=digest(g);p.update(proposal_version=5,geometry_hash=g_hash,paving_polygons=paving,
 overview_representations=copy.deepcopy(read(HERE/'quality-v9/parade/layout.json')['overview_representations']),
 revision_reason='Required reference route hierarchy: one continuous southwest-to-northeast approach replaces the old mid-district vertical detour and duplicate cross-route. A lower west connector joins the retained southern venue link. Original northern courtyard/approaches and descending road retained. Terrain revision only; identical collision, 34 WORLD props, places, spawn/interaction cells and cameras.',
 worker_base='4af00fdb5048bb080e49dcfbddcc2cee61b45478')
p['primary_route']={'id':'southwest_diagonal','centerline':primary,'radius':43,'reason':'Manual semantic route alignment to supplied MAP panel; no reference raster pixels in artwork.'}
p['lower_west_connector']={'centerline':lower,'radius':32}
p['decorative_ground_beds']=[]
p['limitations']=['New terrain geometry candidate, no actual canonical Tiled/server map/release binding supplied by art.','Collision and WORLD prop footprint geometry are exactly frozen spatial-v4; terrain changes require explicit new geometry hash adoption, never oldmap4/899 or0448 reseal.','No new place/interaction, physical route safety/arrival, fourth critic score or target acceptance.']
write(OUT/'geometry-proposal.json',p)
write(OUT/'integer-projection.json',{'version':5,'geometry_hash':g_hash,'policy':'Exact frozen spatial-v4 integer WORLD placements/float footprints; explicit v9 eight MAP-only representations. Terrain changes never reround or relocate players/landmarks.','placements':p['placements'],'overview_representations':p['overview_representations']})
changed=[{'cell_x':x,'cell_y':y,'previous':old['geometry']['terrain_layer'][y][x],'proposed':g['terrain_layer'][y][x]} for y in range(81) for x in range(48) if old['geometry']['terrain_layer'][y][x]!=g['terrain_layer'][y][x]]
routes=[]
for spawn in g['spawns']:
 start=(spawn['cell_x'],spawn['cell_y']);dist={start:0};queue=deque([start])
 while queue:
  x,y=queue.popleft()
  for n in [(x-1,y),(x+1,y),(x,y-1),(x,y+1)]:
   a,b=n
   if 0<=a<48 and 0<=b<81 and not g['collision_grid'][b][a] and n not in dist:dist[n]=dist[(x,y)]+1;queue.append(n)
 for target in g['spawns']+g['interactions']:
  cell=(target['cell_x'],target['cell_y']);assert cell in dist;routes.append({'from':spawn['spawn_id'],'to':target.get('spawn_id',target.get('interaction_id')),'steps':dist[cell]})
def distance(point,line):
 values=[]
 for a,b in zip(line,line[1:]):
  dx,dy=b[0]-a[0],b[1]-a[1];t=max(0,min(1,((point[0]-a[0])*dx+(point[1]-a[1])*dy)/(dx*dx+dy*dy)))
  values.append(math.hypot(point[0]-a[0]-t*dx,point[1]-a[1]-t*dy))
 return min(values)
previous=[[16,1286.859649],[270,875.54386],[270,661],[500,411],[752,151]]
anchors=[]
for x,y in [[975,420],[1075,335],[1155,255],[1223,195]]:
 q=[(x-975)*768/248,(y-95)*1296/420]
 anchors.append({'reference_pixel':[x,y],'map_point':q,'old_primary_distance':distance(q,previous),'new_primary_distance':distance(q,primary)})
write(OUT/'revision-evidence.json',{'status':'PASS_TERRAIN_CANDIDATE_ONLY','base_geometry_hash':old['geometry_hash'],'geometry_hash':g_hash,'terrain_changed_cells':changed,'collision_changed_cells':0,'world_props_changed':0,'routes':routes,
 'reference_route_anchors':{'reference_crop':[975,95,248,420],'method':'Four manually read semantic road-center anchors from the original supplied image. Approximate visual annotation, not raster extraction, whole-map score or independent acceptance.','anchors':anchors},
 'source_mapping':'Replace old paving polygons0..8(primary detour) and16..22(duplicate cross-route), retain9..15/23..46exact; add authored continuous diagonal and lowerwest connector. Ground material decoration ellipses removed; no new solid planting.'})
assert changed and all(p[k]==old[k] for k in ['placements','world_camera','overview_camera','map_labels','places','plaza_parcel'])
assert {k:v for k,v in g.items() if k!='terrain_layer'}=={k:v for k,v in old['geometry'].items() if k!='terrain_layer'}
print(json.dumps({'status':'PASS_SOURCE_GEOMETRY_ONLY','geometry_hash':g_hash,'terrain_changed_cells':len(changed),'collision_changed_cells':0,'routes':len(routes),'primary_anchor_distances':anchors}))
