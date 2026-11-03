#!/usr/bin/env python3
"""Source-preserving layout authoring. Reads alpha to measure frames; no raster edits."""
import hashlib
import json
from pathlib import Path
from PIL import Image

HERE=Path(__file__).resolve().parent
OUT=HERE/'painted'
p=json.loads((HERE/'geometry-proposal.json').read_text())
source=OUT/'parade-source-atlas-v3.png'
image=Image.open(source)
assert image.mode=='RGBA' and image.size==(1254,1254)
columns=[0,324,628,944,1254];rows=[0,328,646,934,1254]
names=['stage','white_merch_booth','white_food_booth','park_plaque','tree_green','tree_yellow','tree_dark','tree_red','meeting_pavilion','giveaway_pavilion','north_hall','west_hall','food_counter','plaza_furniture','cross_banner','shrub_bed']
frames={}
for i,name in enumerate(names):
    left,top,right,bottom=columns[i%4],rows[i//4],columns[i%4+1],rows[i//4+1]
    # Inspect source alpha only: metadata crops do not rewrite a single source pixel.
    occupied=[(x,y) for y in range(top,bottom) for x in range(left,right) if image.getpixel((x,y))[3]>=32]
    assert occupied
    xs,ys=zip(*occupied);x=max(left,min(xs)-2);y=max(top,min(ys)-2);x2=min(right,max(xs)+3);y2=min(bottom,max(ys)+3)
    frames[name]={'x':x,'y':y,'width':x2-x,'height':y2-y,'cell':i,'source_alpha_threshold_for_bounds':32}
tree_frames=['tree_red','tree_green','tree_yellow','tree_green','tree_yellow','tree_green','tree_green','tree_yellow','tree_dark','tree_dark','tree_green']
placements=[]
native_frames=json.loads((HERE/'parade-frames.json').read_text())
for original in p['placements']:
    o=original.copy();f=native_frames[o['frame']]
    o['width']=o.get('width',f['width']);o['height']=o.get('height',f['height'])
    if o['frame'].startswith('tree_'):
        o['frame']=tree_frames[int(o['frame'].split('_')[1])-1]
        # Preserve the painted canopy aspect ratio and its rooted foot. Do not
        # stretch broad atlas foliage into the native draft's tall tree boxes.
        painted_frame=frames[o['frame']]
        o['height']=round(o['width']*painted_frame['height']/painted_frame['width'])
        o['y']=round(o['depth_y']-o['height'],2)
        o['placement_reason']='Named rooted parcel/perimeter planting in shared proposal. Four distinct illustrated species/seasonal frames are intentionally reused at 11 fixed roots; not randomly stamped filler.'
    placements.append(o)
layout={'format':'MCR_PAINTED_SCENE_LAYOUT','version':3,'status':'PROPOSAL_NOT_RELEASE_BOUND','theme_id':'PARADE','geometry_hash':p['geometry_hash'],'ground':'parade-ground.png','atlas':source.name,'frames':frames,'placements':placements,'source_atlas_dimensions':[1254,1254],'world_camera':p['world_camera'],'overview_camera':p['overview_camera'],'raster_source_policy':'Source atlas bytes unchanged. Frame rectangles are non-destructive sampling metadata. Scaled instances and editable SVG layout reconstruct the scene. Map labels are runtime text, never baked into textures.','skins':'PARADE painted increment only. REVENGE/DANGER native drafts are not qualified painted skins.'}
layout['overview_representations']=p['overview_representations']
layout['overview_zone_policy']=p['overview_zone_policy']
(OUT/'layout.json').write_text(json.dumps(layout,indent=2)+'\n')
width,height=p['coordinates']['width_px'],p['coordinates']['height_px']
pattern=lambda name,x,y:f'<pattern id="{name}" width="128" height="128" patternUnits="userSpaceOnUse"><svg width="128" height="128" viewBox="{x} {y} 627 627"><image href="surface-source-v3.png" width="1254" height="1254"/></svg></pattern>'
defs=pattern('lawn',627,0)+pattern('cobbles',0,0)+pattern('low-planting',627,627)
poly=lambda points:' '.join(f'{x},{y}' for x,y in points)
polygons=''.join(f'<polygon id="paving-{i}" points="{poly(points)}" fill="url(#cobbles)"/>' for i,points in enumerate(p['paving_polygons']))
island=''
beds=''.join(f'<ellipse id="parcel-planting-{i}" cx="{x}" cy="{y}" rx="{rx}" ry="{ry}" fill="url(#low-planting)"/>' for i,(x,y,rx,ry) in enumerate([(48,80,40,35),(708,750,42,70),(45,1030,44,70),(650,1100,38,56),(385,1215,86,38),(400,955,35,40),(595,865,40,38)]))
(OUT/'ground-layout.svg').write_text(f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><title>PARADE editable ground geometry with original generated surface materials</title><defs>{defs}</defs><rect id="base-lawn" width="{width}" height="{height}" fill="url(#lawn)"/>{polygons}{island}{beds}</svg>')
objects=''
for o in sorted(placements,key=lambda o:o['depth_y']):
    f=frames[o['frame']]
    objects+=f'<svg id="{o["id"]}" x="{o["x"]}" y="{o["y"]}" width="{o["width"]}" height="{o["height"]}" viewBox="{f["x"]} {f["y"]} {f["width"]} {f["height"]}" preserveAspectRatio="none"><image href="{source.name}" width="1254" height="1254"/></svg>'
(OUT/'scene-layout.svg').write_text(f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><title>PARADE independently editable scene object layout, no baked map labels or characters</title><image id="ground" href="parade-ground.png" width="{width}" height="{height}"/>{objects}</svg>')
map_props=[o for o in placements if o['id'] not in {r.get('replaces_prop_id') for r in p['overview_representations']}]+p['overview_representations']
map_objects=''
for o in sorted(map_props,key=lambda o:o['depth_y']):
    f=frames[o['frame']]
    map_objects+=f'<svg id="{o["id"]}" x="{o["x"]}" y="{o["y"]}" width="{o["width"]}" height="{o["height"]}" viewBox="{f["x"]} {f["y"]} {f["width"]} {f["height"]}" preserveAspectRatio="none"><image href="{source.name}" width="1254" height="1254"/></svg>'
map_zone=f'<polygon id="semantic-plaza-zone" points="{poly(p["plaza_parcel"])}" fill="url(#lawn)" stroke="#b4a27d" stroke-width="6"/>'
(OUT/'map-layout.svg').write_text(f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><title>PARADE semantic overview, same canonical geometry; map art is not physical collision</title><defs>{defs}</defs><image id="ground" href="parade-ground.png" width="{width}" height="{height}"/>{map_zone}{map_objects}</svg>')
sha=lambda file:'sha256:'+hashlib.sha256(file.read_bytes()).hexdigest()
provenance={'method':'Built-in image_gen: original character-free raster scenery and surface source sheets, followed by user-authorized non-generative source-preserving reconstruction. Revision3 style-transfer uses own source atlas and supplied reference for scenery style only; no reference pixel extraction, source atlas pixel rewrite or character authoring.','source_atlas':{'path':source.name,'sha256':sha(source),'dimensions':[1254,1254]},'source_surfaces':{'path':'surface-source-v3.png','sha256':sha(OUT/'surface-source-v3.png'),'dimensions':[1254,1254]},'previous_sources_preserved':[{'path':name,'sha256':sha(OUT/name)} for name in ['parade-source-atlas.png','surface-source.png']],'requested_dimensions_note':'Requested 2048-square atlas and1024-square surface sheet; tool returned1254-square for each. Actual bounds measured and pinned, never assumed.','source_editability':'Original source rasters plus named-object scene-layout.svg, ground-layout.svg pattern polygons, layout.json, build-painted.py and original native proposal compositions. Generated raster contents are not native vector layers.','alpha_note':'Source atlas has genuine RGBA alpha; source pixels are retained unchanged. Runtime alpha composition is checked separately from RGB-only viewers.','reference':p['target'],'geometry_hash':p['geometry_hash'],'authorship':'AI-generated original scenery; no external stock/license or original character asset was sampled.','runtime_labels':'Only original stage/sign typography is in sprite artwork. All six overview destination labels are runtime text.'}
(OUT/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n')
files=[source.name,'surface-source-v3.png','layout.json','provenance.json','ground-layout.svg','scene-layout.svg','map-layout.svg','prompts.md']
if (OUT/'parade-ground.png').exists():files.append('parade-ground.png')
(OUT/'resource-inputs.json').write_text(json.dumps({'format':'MCR_PAINTED_RESOURCE_INPUTS','version':3,'status':'PROPOSAL_NOT_RELEASE_BOUND','theme_id':'PARADE','geometry_hash':p['geometry_hash'],'worker_base':p['worker_base'],'reference':p['target'],'resources':[{'path':name,'sha256':sha(OUT/name),'bytes':(OUT/name).stat().st_size} for name in files],'runtime_resources':['parade-ground.png',source.name],'runtime_labels':'Not baked; map_labels live in shared geometry proposal.','adoption':'Scene proposal only; no current MapManifest/C06/backend binding is changed.'},indent=2)+'\n')
print(json.dumps({'frames':len(frames),'placements':len(placements),'geometry_hash':p['geometry_hash'],'atlas_sha256':sha(source)}))
