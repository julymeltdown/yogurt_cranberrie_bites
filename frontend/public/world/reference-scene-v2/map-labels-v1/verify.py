#!/usr/bin/env python3
"""Inspect saved native assets, editable layers and pixels outside the new labels."""
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile
from PIL import Image, ImageChops, ImageDraw
from fontTools.ttLib import TTFont

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
read=lambda p:json.loads(p.read_text())
sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
p=read(HERE/'presentation.json');results=[]
for item in p['frozen_inputs']+[p['font'],p['font_license'],p['provenance']['font_source'],p['provenance']['font_source_license']]:
 path=ROOT/item['path'];assert sha(path)==item['sha256'] and path.stat().st_size==item['bytes']
for frozen in p['frozen_inputs']:
 path=ROOT/frozen['path']
 assert path.read_bytes()==subprocess.check_output(['git','show',p['reference_scene_commit']+':'+frozen['path']])
oldmeta=read(HERE.parent/'quality-v8/parade/map-metadata.json')
assert [{k:l[k] for k in ('label','x','y','interaction_id')} for l in p['labels']]==oldmeta['map_labels']
assert len(p['labels'])==6 and sum(bool(l['icon']) for l in p['labels'])==5
assert sum(l['interaction_id'] is not None for l in p['labels'])==2
assert TTFont(HERE/'map-labels-bold.ttf')['OS/2'].usWeightClass==800
for i,l in enumerate(p['labels']):
 r=l['rect'];assert r['x']>=5 and r['y']>=5 and r['x']+r['width']<=763 and r['y']+r['height']<=1291
 for o in p['labels'][i+1:]:
  b=o['rect'];assert r['x']+r['width']<=b['x'] or b['x']+b['width']<=r['x'] or r['y']+r['height']<=b['y'] or b['y']+b['height']<=r['y']

for era in ['parade','revenge','danger']:
 old=HERE.parent/f'quality-v8/{era}'
 doc=read(HERE/f'{era}-map-labels-editable.json');layers=doc['composition']['layers']
 oldlayers=read(old/f'{era}-map-editable.json')['composition']['layers']
 assert layers[-len(oldlayers):]==oldlayers
 ids=[l['id'] for l in layers];assert len(ids)==len(set(ids))
 overlay=layers[:-len(oldlayers)];texts=[l for l in overlay if l['type']=='Text']
 assert sorted(t['sourceText']['text'] for t in texts)==sorted(line for l in p['labels'] for line in l['lines'])
 assert len(texts)==8 and all(t['sourceText']['fontFamily']==p['style']['font_family'] for t in texts)
 assert all(l['type'] in ('Shape','Text') for l in overlay)
 expected_font=hashlib.sha256((HERE/'map-labels-bold.ttf').read_bytes()).hexdigest()
 with zipfile.ZipFile(HERE/f'{era}-map-labels.tsrct') as z,zipfile.ZipFile(old/f'{era}-map.tsrct') as oldz:
  assert json.loads(z.read('project.json'))['composition']==doc['composition']
  native_pngs=[n for n in z.namelist() if n.lower().endswith('.png')]
  old_pngs=[n for n in oldz.namelist() if n.lower().endswith('.png')]
  assert len(native_pngs)==len(old_pngs)==2
  assert sorted(hashlib.sha256(z.read(n)).hexdigest() for n in native_pngs)==sorted(hashlib.sha256(oldz.read(n)).hexdigest() for n in old_pngs)
  packaged_fonts=[n for n in z.namelist() if n.endswith('.ttf')]
  assert len(packaged_fonts)==1 and hashlib.sha256(z.read(packaged_fonts[0])).hexdigest()==expected_font
 before=Image.open(old/f'{era}-map-native.png').convert('RGBA');after=Image.open(HERE/f'{era}-map-labels-native.png').convert('RGBA')
 assert before.size==after.size==(768,1296)
 diff=ImageChops.difference(before,after);channels=diff.split();mask=Image.new('L',before.size,0)
 for channel in channels:mask=ImageChops.lighter(mask,channel)
 allowed=Image.new('L',before.size,0);draw=ImageDraw.Draw(allowed)
 for l in p['labels']:
  r=l['rect'];draw.rectangle((r['x'],r['y'],r['x']+r['width']-1,r['y']+r['height']-1),fill=255)
 outside=ImageChops.subtract(mask,allowed);assert outside.getbbox() is None,('Unexpected scenery pixel change',era,outside.getbbox())
 assert mask.getbbox() is not None
 # Every authored icon must actually differ from its flat square in the rendered PNG.
 for l in p['labels']:
  if not l['icon']:continue
  r=l['rect'];x=r['x']+p['style']['padding'];y=r['y']+(r['height']-p['style']['icon_size'])/2;s=p['style']['icon_size']
  crop=after.crop((int(x),int(y),int(x+s),int(y+s))).convert('RGB');colors=crop.getcolors(crop.width*crop.height)
  dark=sum(count for count,color in colors if max(color)<70)
  assert dark>=120,(era,l['icon'],dark)
 results.append({'era':era,'dimensions':list(after.size),'native_overlay_layers':len(overlay),'editable_text_layers':len(texts),
                 'scenery_pixels_outside_labels':'EXACT_V8','unchanged_packaged_pngs':2,'packaged_exact_font':True,'rendered_icons':5})
write={'status':'PASS_NATIVE_LABEL_COMPOSITION_ONLY','geometry_hash':p['geometry_hash'],'presentation_sha256':sha(HERE/'presentation.json'),'results':results,
       'limits':'Original v8 source and PNG assets remain frozen. No application, backend/source admission, physical qualification, independent visual score or target acceptance.'}
(HERE/'verification.json').write_text(json.dumps(write,indent=2)+'\n')
print(json.dumps({'status':'PASS','variants':len(results),'editable_text_layers':24,'icons':15,'outside_label_pixels':'EXACT_V8'}))
