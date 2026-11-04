#!/usr/bin/env python3
"""Measure projected dominant tree alpha, never a reference-fidelity score."""
import hashlib,json
from pathlib import Path
from PIL import Image, ImageChops, ImageDraw

HERE=Path(__file__).resolve().parent
read=lambda p:json.loads(p.read_text())
def measure(revision,era,view):
 folder=HERE/f'quality-v{revision}'/era;layout=read(folder/'layout.json');atlas=Image.open(folder/'atlas.png').convert('RGBA')
 objects=list(layout['placements'])
 if view=='map':
  reps=layout['overview_representations'];replaced={o.get('replaces_prop_id') for o in reps};objects=[o for o in objects if o['id'] not in replaced]+reps
 mask=Image.new('L',(768,1296))
 for o in sorted(objects,key=lambda o:o['depth_y']):
  f=layout['frames'][o['frame']]
  alpha=atlas.getchannel('A').crop((f['x'],f['y'],f['x']+f['width'],f['y']+f['height'])).resize((o['width'],o['height']),Image.Resampling.NEAREST).point(lambda a:255 if a>128 else 0)
  # Later opaque foreground objects remove tree coverage they actually obscure.
  mask.paste(255 if o['frame'].startswith('tree_') else 0,(o['x'],o['y'],o['x']+o['width'],o['y']+o['height']),alpha)
 c=layout['world_camera'] if view=='world' else layout['overview_camera'];box=(c['x'],c['y'],c['x']+c['width'],c['y']+c['height'])
 perimeter=Image.new('L',(768,1296));draw=ImageDraw.Draw(perimeter);draw.rectangle((0,0,179,1295),fill=255);draw.rectangle((588,0,767,1295),fill=255)
 visible=mask.crop(box).histogram()[255];edge=ImageChops.multiply(mask,perimeter).crop(box).histogram()[255]
 return {'tree_props':sum(o['frame'].startswith('tree_') for o in objects),'camera':c,'visible_tree_pixels':visible,'edge_band_visible_tree_pixels':edge,'camera_pixels':c['width']*c['height']}
rows=[]
for era in ['revenge','parade','danger']:
 for view in ['world','map']:
  before=measure(7,era,view);after=measure(8,era,view)
  assert after['visible_tree_pixels']>before['visible_tree_pixels']
  rows.append({'era':era.upper(),'view':view.upper(),'before_v7':before,'after_v8':after,'visible_tree_pixel_ratio':after['visible_tree_pixels']/before['visible_tree_pixels'],'edge_band_pixel_ratio':after['edge_band_visible_tree_pixels']/before['edge_band_visible_tree_pixels']})
report={'status':'MEASURED_SOURCE_DENSITY_CHANGE','input_manifests':[{'path':f'../quality-v{v}/manifest.json','sha256':'sha256:'+hashlib.sha256((HERE/f'quality-v{v}/manifest.json').read_bytes()).hexdigest()} for v in [7,8]],'method':'Nearest-raster projection of actual PNG alpha>128 in depth order; non-tree opaque foreground erases tree coverage. Exact source views; side bands are sceneX0..179 and588..767. No UI labels/characters.','limitations':'Not a target match percentage, critic score, native/browser exact-pixel parity claim or runtime/avatar proof. Regions were fixed by the authored perimeter intent, not optimized for this metric.','comparisons':rows}
(HERE/'quality-v8/density-measurements.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps([{k:r[k] for k in ['era','view','visible_tree_pixel_ratio','edge_band_pixel_ratio']} for r in rows]))
