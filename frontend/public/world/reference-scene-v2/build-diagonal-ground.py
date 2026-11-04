#!/usr/bin/env python3
"""Native editable material tiles clipped to new authored paving polygons."""
import hashlib,json,subprocess
from pathlib import Path
from PIL import Image,ImageChops
HERE=Path(__file__).resolve().parent;OUT=HERE/'quality-v10';OUT.mkdir(exist_ok=True)
CLI=str(Path.home()/'Library/Application Support/Tesseract/bin/tsrct')
read=lambda p:json.loads(p.read_text());write=lambda p,v:p.write_text(json.dumps(v,indent=2)+'\n')
sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
p=read(HERE/'spatial-v5/geometry-proposal.json');source=HERE/'quality-v6/sources/surface-sheet.png';raw=Image.open(source).convert('RGBA');assert raw.size==(1254,1254)
pad=Image.new('RGBA',(2048,2048));pad.paste(raw,(0,0));pad.save(OUT/'surface-native.png',optimize=True,compress_level=9)
assert not any(c.getbbox() for c in ImageChops.difference(pad.crop((0,0,1254,1254)),raw).split())
commands=[]
def run(args):
 r=subprocess.run([CLI]+args,capture_output=True,text=True);commands.append({'command':['tsrct']+[a.replace(str(HERE)+'/', '') for a in args],'exit_code':r.returncode})
 if r.returncode:raise RuntimeError(r.stdout+r.stderr)
project=OUT/'material-ground.tsrct';editable=OUT/'material-ground-editable.json'
if not project.exists():
 run(['project','create','--project',str(project)])
 run(['project','import-asset','--project',str(project),'--file',str(OUT/'surface-native.png'),'--asset-id','original-materials','--kind','image'])
run(['project','checkout','--project',str(project),'--output',str(editable)])
doc=read(editable);doc.update(dimensions={'width':768,'height':1296},duration=3)
def clip(poly,axis,bound,sign):
 out=[]
 for a,b in zip(poly,poly[1:]+poly[:1]):
  ina=(a[axis]-bound)*sign>=0;inb=(b[axis]-bound)*sign>=0
  if ina:out.append(a)
  if ina!=inb:
   t=(bound-a[axis])/(b[axis]-a[axis]);out.append([a[0]+t*(b[0]-a[0]),a[1]+t*(b[1]-a[1])])
 return out
layers=[]
for material,sx in [('lawn',627),('paving',0)]:
 for y in range(0,1296,128):
  for x in range(0,768,128):
   if material=='lawn':polys=[[[x,y],[x+128,y],[x+128,y+128],[x,y+128]]]
   else:
    polys=[]
    for poly in p['paving_polygons']:
     q=poly
     for axis,bound,sign in [(0,x,1),(0,x+128,-1),(1,y,1),(1,y+128,-1)]:q=clip(q,axis,bound,sign) if q else []
     if len(q)>=3:polys.append(q)
   if not polys:continue
   path=[]
   for poly in polys:
    if sum(poly[i-1][0]*a[1]-a[0]*poly[i-1][1] for i,a in enumerate(poly))<0:poly=list(reversed(poly))
    path.extend([{'type':'moveTo' if i==0 else 'lineTo','x':px,'y':py} for i,(px,py) in enumerate(poly)]+[{'type':'close'}])
   scale=128/627
   layers.append({'id':len(layers)+1,'name':f'Original {material} tile {x},{y} - editable mask','type':'Image','blendMode':'normal','activeRange':{'start':0,'duration':3000},
    'transform':{'anchorPoint':[0,0],'position':[0,0],'scale':[100,100],'rotation':0,'opacity':100},
    'masks':[{'id':10000+len(layers),'mode':'add','opacity':100,'path':{'commands':path}}],
    'source':{'assetId':'original-materials','fit':{'custom':{'scale':[scale,scale],'contentCenter':[x+(1024-sx)*scale,y+1024*scale]}}}})
doc['composition']={'id':'main','name':'Original material ground for continuous diagonal spatial-v5; no raster reference pixels','layers':list(reversed(layers))}
write(editable,doc);run(['project','commit','--project',str(project),'--file',str(editable)])
run(['preview','--project',str(project),'--time','1','--output',str(OUT/'material-ground-native.png')])
i=Image.open(OUT/'material-ground-native.png');assert i.mode=='RGBA' and i.getchannel('A').getextrema()==(255,255)
i.convert('RGB').save(OUT/'material-ground.png',optimize=True,compress_level=9)
write(OUT/'material-ground-evidence.json',{'commands':commands,'geometry_hash':p['geometry_hash'],'source':{'path':'../quality-v6/sources/surface-sheet.png','sha256':sha(source)},'native_source':{'path':'surface-native.png','sha256':sha(OUT/'surface-native.png'),'method':'ExactRGBAoriginal pixels at0,0 with transparent native-only padding; no resampling or source changes.'},'layers':len(layers),'native_rgb_export':{'path':'material-ground.png','sha256':sha(OUT/'material-ground.png')},'method':'Native image layers sample original627pxgrass/cobble quadrants into128pxmaterial tiles; same scale and tile origin as v6. Editable masks clip paving to the new polygon union. No decorative ellipses, new textures from reference or characters.'})
print(json.dumps({'status':'SOURCE_GROUND_RENDERED','layers':len(layers),'bytes':(OUT/'material-ground.png').stat().st_size}))
