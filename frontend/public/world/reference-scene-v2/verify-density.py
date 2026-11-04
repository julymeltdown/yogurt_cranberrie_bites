#!/usr/bin/env python3
"""Independently check new rooted decoration without admitting a map release."""
import json
from collections import deque
from pathlib import Path
from build import cell_intersects,digest

HERE=Path(__file__).resolve().parent;OUT=HERE/'spatial-v4'
read=lambda p:json.loads(p.read_text())
p=read(OUT/'geometry-proposal.json');g=p['geometry'];base=read(HERE/'spatial-v3/geometry-proposal.json');old=read(HERE/'spatial-v3/integer-projection.json');projection=read(OUT/'integer-projection.json');evidence=read(OUT/'revision-evidence.json')
assert digest(g)==p['geometry_hash']==projection['geometry_hash']!=base['geometry_hash']
assert p['placements']==projection['placements'] and len(p['placements'])==34
assert p['placements'][:22]==old['placements']
assert p['overview_representations']==projection['overview_representations']==old['overview_representations']
assert p['places']==base['places'] and p['map_labels']==base['map_labels'] and p['plaza_parcel']==base['plaza_parcel']
assert p['paving_polygons']==base['paving_polygons']
assert p['world_camera']==base['world_camera'] and p['overview_camera']==base['overview_camera']
assert (g['width_cells'],g['height_cells'],g['tile_width_px'],g['tile_height_px'])==(48,81,16,16)
assert {k:v for k,v in g.items() if k not in ['collision_grid','collision_id']}=={k:v for k,v in base['geometry'].items() if k not in ['collision_grid','collision_id']}
ids=[o['id'] for o in p['placements']];assert len(ids)==len(set(ids))
new=p['placements'][22:];assert all(o['interaction_id'] is None for o in new)
assert len(new)==len(evidence['planting_intent'])==12
for o in p['placements']:
 assert all(type(o[k]) is int for k in ['x','y','width','height','depth_y'])
 assert -128<=o['x'] and -128<=o['y'] and o['x']+o['width']<=896 and o['y']+o['height']<=1424
 assert all(0<=x<=768 and 0<=y<=1296 for x,y in o['footprint'])
 assert abs(max(y for x,y in o['footprint'])-o['depth_y'])<=4
points=g['spawns']+g['interactions']
for o,intent in zip(new,evidence['planting_intent']):
 assert o['id']==intent['id'];cx,cy=intent['root'];sx,sy=intent['target_root']
 assert (cx-sx)**2+(cy-sy)**2<=64**2 and cy==o['depth_y']
 assert all((cx-(q['cell_x']+.5)*16)**2+(cy-(q['cell_y']+.5)*16)**2>=80**2 for q in points)
 actual={(x,y) for y in range(81) for x in range(48) if cell_intersects(x,y,o['footprint'])}
 assert actual=={tuple(c) for c in intent['root_collision_cells']}
 assert all(base['geometry']['collision_grid'][y][x]==0 and base['geometry']['terrain_layer'][y][x] in intent['allowed_terrain_ids'] for x,y in actual)
for y in range(81):
 for x in range(48):
  exact=int(x in [0,47] or y in [0,80] or any(cell_intersects(x,y,o['footprint']) for o in p['placements']))
  assert g['collision_grid'][y][x]==exact
  assert g['collision_grid'][y][x]>=base['geometry']['collision_grid'][y][x]
routes=[]
for spawn in g['spawns']:
 start=spawn['cell_x'],spawn['cell_y'];dist={start:0};q=deque([start])
 while q:
  x,y=q.popleft()
  for nx,ny in [(x-1,y),(x+1,y),(x,y-1),(x,y+1)]:
   if 0<=nx<48 and 0<=ny<81 and not g['collision_grid'][ny][nx] and (nx,ny) not in dist:dist[nx,ny]=dist[x,y]+1;q.append((nx,ny))
 for point in points:
  key=point['cell_x'],point['cell_y'];assert key in dist
  routes.append({'from':spawn['spawn_id'],'to':point.get('spawn_id',point.get('interaction_id')),'steps':dist[key]})
assert routes==evidence['routes']
changed=sum(g['collision_grid'][y][x]!=base['geometry']['collision_grid'][y][x] for y in range(81) for x in range(48))
assert evidence['changed_cells']==[{'cell_x':x,'cell_y':y,'previous':base['geometry']['collision_grid'][y][x],'proposed':g['collision_grid'][y][x]} for y in range(81) for x in range(48) if g['collision_grid'][y][x]!=base['geometry']['collision_grid'][y][x]]
assert changed==72 and all(c['previous']==0 and c['proposed']==1 for c in evidence['changed_cells'])
report={'status':'PASS_SPATIAL_V4_PROPOSAL_ONLY','geometry_hash':p['geometry_hash'],'collision_cells_checked':3888,'new_solid_cells':changed,'physical_props':34,'new_unbound_roots':12,'new_place_or_interaction_ids':0,'old22props_exact':True,'terrain_paving_cells_spawns_interactions_cameras_places_unchanged':True,'routes':routes,'limits':'Virtual candidate. No release/Tiled authority, physical safety/arrival, moving-avatar or target acceptance.'}
(OUT/'verification.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k!='routes'}))
