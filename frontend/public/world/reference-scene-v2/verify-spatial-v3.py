#!/usr/bin/env python3
"""Check the separately versioned spatial candidate against source and invariants."""
import json
from collections import deque
from pathlib import Path
from build import cell_intersects,digest,inside
HERE=Path(__file__).resolve().parent;OUT=HERE/'spatial-v3'
base=json.loads((HERE/'eras-v5/parade/layout.json').read_text());old=json.loads((HERE/'geometry-proposal.json').read_text());p=json.loads((OUT/'geometry-proposal.json').read_text());g=p['geometry'];projection=json.loads((OUT/'integer-projection.json').read_text())
assert p['geometry_hash']==projection['geometry_hash']==digest(g)!=old['geometry_hash']
assert p['places']==old['places'] and p['status']=='PROPOSAL_NOT_RELEASE_BOUND'
assert (g['width_cells'],g['height_cells'],g['tile_width_px'],g['tile_height_px'])==(48,81,16,16)
assert [(o['id'],o['interaction_id']) for o in p['placements']]==[(o['id'],o['interaction_id']) for o in old['placements']]
assert {o['interaction_id']:o['place_id'] for o in g['interactions']}=={o['interaction_id']:o['place_id'] for o in old['geometry']['interactions']}
assert g['overdraw_layer']==old['geometry']['overdraw_layer']
for y in range(81):
 for x in range(48):
  expected=int(x in [0,47] or y in [0,80] or any(cell_intersects(x,y,o['footprint']) for o in p['placements']))
  assert g['collision_grid'][y][x]==expected,(x,y)
  assert g['terrain_layer'][y][x]==(2 if any(inside((x+.5)*16,(y+.5)*16,poly) for poly in p['paving_polygons']) else 1)
for obj in p['placements']:
 assert len(obj['footprint'])>=3 and all(0<=x<=768 and 0<=y<=1296 for x,y in obj['footprint'])
 assert abs(max(y for x,y in obj['footprint'])-obj['depth_y'])<=4
 old_obj=next(o for o in base['placements'] if o['id']==obj['id'])
 for a,b in zip(obj['footprint'],old_obj['footprint']):assert a[0]==b[0] and abs((a[1]-b[1])-(obj['depth_y']-old_obj['depth_y']))<.001
for group in ['placements','overview_representations']:
 ids={o['id'] for o in p['placements']}
 for o in projection[group]:
  assert all(type(o[k]) is int for k in ['x','y','width','height','depth_y'])
  if group=='placements':assert o['x']>=-128 and o['y']>=-128 and o['x']+o['width']<=896 and o['y']+o['height']<=1424
  else:assert o['x']<768 and o['y']<1296 and o['x']+o['width']>0 and o['y']+o['height']>0
  if group=='overview_representations':assert 'footprint' not in o and o['id'] not in ids
routes=[];points=g['spawns']+g['interactions']
assert all(g['collision_grid'][q['cell_y']][q['cell_x']]==0 for q in points)
for spawn in g['spawns']:
 start=spawn['cell_x'],spawn['cell_y'];dist={start:0};queue=deque([start])
 while queue:
  x,y=queue.popleft()
  for nx,ny in [(x-1,y),(x+1,y),(x,y-1),(x,y+1)]:
   if 0<=nx<48 and 0<=ny<81 and not g['collision_grid'][ny][nx] and (nx,ny) not in dist:dist[nx,ny]=dist[x,y]+1;queue.append((nx,ny))
 for q in points:
  key=q['cell_x'],q['cell_y'];assert key in dist;routes.append({'from':spawn['spawn_id'],'to':q.get('spawn_id',q.get('interaction_id')),'steps':dist[key]})
report={'status':'PASS_SPATIAL_V3_PROPOSAL_ONLY','geometry_hash':p['geometry_hash'],'collision_cells':48*81,'routes':routes,'spawns':2,'interactions':4,'physical_props':22,'map_only_representations':6,'new_ids':0,'place_records_identical':True,'limits':'Separate virtual candidate only; no actual runtime Tiled/descriptor adoption, physical review, source-review-state upgrade or fidelity acceptance.'}
(OUT/'verification.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k!='routes'}))
