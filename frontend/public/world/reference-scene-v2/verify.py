#!/usr/bin/env python3
"""Validate this proposal without changing current product release bindings."""
from collections import deque
import hashlib
import json
from pathlib import Path
import struct
from build import canonical, cell_intersects, digest

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[3]

def read(path):return json.loads(path.read_text())
def png(path):
    raw=path.read_bytes();assert raw[:8]==b'\x89PNG\r\n\x1a\n'
    return struct.unpack('>II',raw[16:24])
def resource(name):
    raw=(HERE/name).read_bytes()
    return {'path':name,'sha256':'sha256:'+hashlib.sha256(raw).hexdigest(),'bytes':len(raw)}

def main():
    p=read(HERE/'geometry-proposal.json');g=p['geometry'];w,h=g['width_cells'],g['height_cells'];grid=g['collision_grid']
    assert p['status']=='PROPOSAL_NOT_RELEASE_BOUND' and p['geometry_hash']==digest(g)
    assert (w,h)==(48,81)
    assert p['coordinates']['width_px']==w*16 and p['coordinates']['height_px']==h*16
    assert len(grid)==h and all(len(row)==w and set(row)<={0,1} for row in grid)
    for layer in ['terrain_layer','overdraw_layer']:
        assert len(g[layer])==h and all(len(row)==w for row in g[layer])
    assert len({o['id'] for o in p['placements']})==len(p['placements'])
    assert len({o['frame'] for o in p['placements'] if o['frame'].startswith('tree_')})==11
    current=read(ROOT/'frontend/public/world/maps/geometry.json')
    existing={i['interaction_id']:i['place_id'] for i in current['interactions']}
    assert all(existing[i['interaction_id']]==i['place_id'] for i in g['interactions'])
    assert {i['place_id'] for i in g['interactions']}=={i['place_id'] for i in p['places']}
    assert all(o['interaction_id'] is None or o['interaction_id'] in existing for o in p['placements'])
    assert all(o['interaction_id'] is None or o['interaction_id'] in existing for o in p['map_labels'])
    physical_ids={o['id'] for o in p['placements']}
    for representation in p['overview_representations']:
        assert 'footprint' not in representation and 'cell_x' not in representation
        assert representation['interaction_id'] is None or representation['interaction_id'] in existing
        assert representation.get('replaces_prop_id') is None or representation['replaces_prop_id'] in physical_ids
        assert representation['width']>0 and representation['height']>0
        assert representation['x']<w*16 and representation['y']<h*16 and representation['x']+representation['width']>0 and representation['y']+representation['height']>0
    for o in p['placements']:
        assert len(o['footprint'])>=3
        assert all(0<=x<=w*16 and 0<=y<=h*16 for x,y in o['footprint']),o['id']
        assert abs(o['depth_y']-max(y for _,y in o['footprint']))<=4,o['id']
        for y in range(h):
            for x in range(w):
                if cell_intersects(x,y,o['footprint']):assert grid[y][x],(o['id'],x,y)
    routes=[]
    points=g['spawns']+g['interactions']
    for point in points:
        assert 0<=point['cell_x']<w and 0<=point['cell_y']<h
        assert grid[point['cell_y']][point['cell_x']]==0,point
    for spawn in g['spawns']:
        start=(spawn['cell_x'],spawn['cell_y']);seen={start};queue=deque([start]);distance={start:0}
        while queue:
            x,y=queue.popleft()
            for nx,ny in [(x-1,y),(x+1,y),(x,y-1),(x,y+1)]:
                if 0<=nx<w and 0<=ny<h and not grid[ny][nx] and (nx,ny) not in seen:
                    seen.add((nx,ny));distance[nx,ny]=distance[x,y]+1;queue.append((nx,ny))
        for point in points:
            key=(point['cell_x'],point['cell_y']);assert key in seen,(spawn,point)
            routes.append({'from':spawn['spawn_id'],'to':point.get('spawn_id',point.get('interaction_id')),'steps':distance[key]})
    resources=[]
    for skin in ['parade','revenge','danger']:
        frames=read(HERE/f'{skin}-frames.json');assert len(frames)==22
        rects=[]
        for key,f in frames.items():
            assert all(isinstance(f[k],int) and f[k]>=0 for k in ['x','y','width','height'])
            assert f['width']>0 and f['height']>0 and f['x']+f['width']<=1024 and f['y']+f['height']<=1024
            for other in rects:
                assert f['x']>=other['x']+other['width'] or other['x']>=f['x']+f['width'] or f['y']>=other['y']+other['height'] or other['y']>=f['y']+f['height']
            rects.append(f)
        for o in p['placements']:
            f=frames[o['frame']]
            assert o['x']>=0 and o['y']>=0 and o['x']+o.get('width',f['width'])<=w*16 and o['y']+o.get('height',f['height'])<=h*16,o['id']
        assert png(HERE/f'{skin}-ground.png')==(w*16,h*16)
        assert png(HERE/f'{skin}-scenery.png')==(1024,1024)
        sources=[]
        for kind in ['ground','scenery']:
            d=read(HERE/f'{skin}-{kind}-editable.json');expected=(w*16,h*16) if kind=='ground' else (1024,1024)
            assert tuple(d['dimensions'][k] for k in ['width','height'])==expected
            assert d['composition']['layers'] and all(l['type']=='Shape' for l in d['composition']['layers'])
            for l in d['composition']['layers']:
                for c in l['shape']['path']['commands']:
                    if 'x' in c:assert 0<=c['x']<=expected[0] and 0<=c['y']<=expected[1],(skin,kind,l['name'])
            for ext in ['svg','tsrct']:sources.append(resource(f'{skin}-{kind}.{ext}'))
            sources.append(resource(f'{skin}-{kind}-editable.json'))
        resources.append({'theme_id':skin.upper(),'ground':resource(f'{skin}-ground.png')|{'width':w*16,'height':h*16},'atlas':resource(f'{skin}-scenery.png')|{'width':1024,'height':1024,'frames':resource(f'{skin}-frames.json')},'editable_sources':sources})
    manifest={'format':'MCR_REFERENCE_SCENE_RESOURCE_INPUTS','version':2,'status':'PROPOSAL_NOT_RELEASE_BOUND','geometry_hash':p['geometry_hash'],'geometry':resource('geometry-proposal.json'),'worker_base':p['worker_base'],'target':p['target'],'themes':resources,'placement_contract':'geometry-proposal.json placements; stable id/frame/x/y/optional width/height/depth_y/footprint/interaction_id. Draw atlas frames sorted ascending depth_y, actor depth=feet y. Alpha canopy overhang is not a solid wall.','adoption':'Integrator must validate coordinated server collision/release and renderer adoption. This file is not a current release manifest.'}
    (HERE/'resource-inputs.json').write_text(json.dumps(manifest,indent=2)+'\n')
    report={'status':'PASS_PROPOSAL_ONLY','geometry_hash':p['geometry_hash'],'dimensions':[w,h],'placements':len(p['placements']),'unique_tree_frames':11,'existing_interactions':len(g['interactions']),'spawns':len(g['spawns']),'collision_rule':'Exact convex footprint/cell SAT, touching is blocked; lawn is walkable virtual terrain.','routes':routes,'pngs':6,'native_compositions':6,'scope':'Asset/geometry checks only, not application/server/browser multiuser acceptance.'}
    (HERE/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items() if k!='routes'}))

if __name__=='__main__':main()
