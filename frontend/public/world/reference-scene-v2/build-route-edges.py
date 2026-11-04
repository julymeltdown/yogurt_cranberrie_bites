#!/usr/bin/env python3
"""Editable painted edge of the actual paving union; no new terrain or solids."""
import copy
import hashlib
import json
import math
from pathlib import Path
import subprocess
import argparse
from PIL import Image, ImageChops

parser=argparse.ArgumentParser();parser.add_argument('--geometry-revision',type=int,choices=[4,5],default=4);parser.add_argument('--quality-revision',type=int,choices=[9,10],default=9);parser.add_argument('--base-ground',default='quality-v6/ground.png');args=parser.parse_args()
HERE=Path(__file__).resolve().parent;OUT=HERE/f'quality-v{args.quality_revision}';OUT.mkdir(exist_ok=True)
base_ground=HERE/args.base_ground
CLI=str(Path.home()/'Library/Application Support/Tesseract/bin/tsrct')
read=lambda p:json.loads(p.read_text());write=lambda p,v:p.write_text(json.dumps(v,indent=2)+'\n')
sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
p=read(HERE/f'spatial-v{args.geometry_revision}/geometry-proposal.json');W,H=768,1296

def clip(poly,axis,value,sign):
    result=[]
    for a,b in zip(poly,poly[1:]+poly[:1]):
        ina=(a[axis]-value)*sign>=0;inb=(b[axis]-value)*sign>=0
        if ina:result.append(a)
        if ina!=inb:
            t=(value-a[axis])/(b[axis]-a[axis]);result.append([a[0]+t*(b[0]-a[0]),a[1]+t*(b[1]-a[1])])
    return result

polys=[]
for poly in p['paving_polygons']:
    poly=copy.deepcopy(poly)
    for axis,value,sign in [(0,0,1),(0,W,-1),(1,0,1),(1,H,-1)]:poly=clip(poly,axis,value,sign) if poly else []
    if len(poly)>=3:polys.append(poly)
edges=[(a,b) for poly in polys for a,b in zip(poly,poly[1:]+poly[:1]) if math.dist(a,b)>1e-7]
cross=lambda a,b:a[0]*b[1]-a[1]*b[0]
sub=lambda a,b:[a[0]-b[0],a[1]-b[1]]
dot=lambda a,b:a[0]*b[0]+a[1]*b[1]
def inside(x,y):
    for poly in polys:
        odd=False
        for a,b in zip(poly,poly[1:]+poly[:1]):
            if (a[1]>y)!=(b[1]>y) and x<(b[0]-a[0])*(y-a[1])/(b[1]-a[1])+a[0]:odd=not odd
        if odd:return True
    return False

boundary={}
for a,b in edges:
    r=sub(b,a);rr=dot(r,r);ts=[0.,1.]
    for c,d in edges:
        s=sub(d,c);q=sub(c,a);den=cross(r,s)
        if abs(den)>1e-9:
            t,u=cross(q,s)/den,cross(q,r)/den
            if -1e-9<=t<=1+1e-9 and -1e-9<=u<=1+1e-9:ts.append(max(0.,min(1.,t)))
        elif abs(cross(q,r))<1e-7:
            for v in [c,d]:
                t=dot(sub(v,a),r)/rr
                if 0<t<1:ts.append(t)
    ts=sorted(set(round(t,12) for t in ts))
    for t0,t1 in zip(ts,ts[1:]):
        if t1-t0<1e-9:continue
        start=[a[i]+r[i]*t0 for i in (0,1)];end=[a[i]+r[i]*t1 for i in (0,1)]
        mid=[(start[i]+end[i])/2 for i in (0,1)];length=math.sqrt(rr);nx,ny=-r[1]/length*.001,r[0]/length*.001
        left,right=inside(mid[0]+nx,mid[1]+ny),inside(mid[0]-nx,mid[1]-ny)
        if left==right:continue
        if any(abs(start[axis]-value)<1e-6 and abs(end[axis]-value)<1e-6 for axis,value in [(0,0),(0,W),(1,0),(1,H)]):continue
        ends=sorted([tuple(round(v,5) for v in start),tuple(round(v,5) for v in end)])
        boundary[tuple(ends)]={'start':[round(v,6) for v in start],'end':[round(v,6) for v in end]}
segments=sorted(boundary.values(),key=lambda v:(v['start'][1],v['start'][0],v['end'][1],v['end'][0]))
assert len(segments)>(100 if args.quality_revision==9 else 50)
spec={'version':args.quality_revision,'geometry_hash':p['geometry_hash'],'paving_source':{'path':f'../spatial-v{args.geometry_revision}/geometry-proposal.json','sha256':sha(HERE/f'spatial-v{args.geometry_revision}/geometry-proposal.json')},
      'segments':segments,'strokes':[{'width':5,'color':'#8c846e'},{'width':2,'color':'#cec3a8'}],
      'method':'Exact authored polygon edges split at all intersections; retain only subsegments whose left/right infinitesimal probes disagree on union membership. Deduplicate collinear overlaps; omit scene crop edges. No internal road-overlap seams.',
      'semantics':'Walkable painted paving edges. No additional collision or terrain; source geometry and all34 props/footprints/spawns/interactions/place identities unchanged.'}
if args.quality_revision==10:
    spec['semantics']='Walkable painted paving edges relative to selected spatial-v5 terrain. This paint step changes no collision, terrain or34 props/footprints/spawns/interactions/place identities; spatial-v5 itself explicitly changes560 terrain cells from spatial-v4.'
write(OUT/'route-edges.json',spec)
evidence=[]
def run(args):
    r=subprocess.run([CLI]+args,capture_output=True,text=True)
    evidence.append({'command':['tsrct']+[a.replace(str(HERE)+'/', '') for a in args],'exit_code':r.returncode})
    if r.returncode:raise RuntimeError(r.stdout+r.stderr)

project=OUT/'ground.tsrct';editable=OUT/'ground-editable.json'
if not project.exists():
    run(['project','create','--project',str(project)])
    run(['project','import-asset','--project',str(project),'--file',str(base_ground),'--asset-id','frozen-v6-ground','--kind','image'])
run(['project','checkout','--project',str(project),'--output',str(editable)])
doc=read(editable);doc.update(dimensions={'width':W,'height':H},duration=3)
common=lambda id,name,typ:{'id':id,'name':name,'type':typ,'blendMode':'normal','activeRange':{'start':0,'duration':3000},'transform':{'anchorPoint':[0,0],'position':[0,0],'scale':[100,100],'rotation':0,'opacity':100}}
ground=common(1,'Frozen v6 original ground materials' if args.quality_revision==9 else 'Original material tiles on spatial-v5 paving','Image');ground['source']={'assetId':'frozen-v6-ground','fit':{'custom':{'scale':[1,1],'contentCenter':[W/2,H/2]}}}
commands=[command for s in segments for command in [{'type':'moveTo','x':s['start'][0],'y':s['start'][1]},{'type':'lineTo','x':s['end'][0],'y':s['end'][1]}]]
layers=[ground]
for i,stroke in enumerate(spec['strokes']):
    color=stroke['color'];l=common(i+2,'Painted paving union '+('shadow' if i==0 else 'stone highlight'),'Shape')
    l['shape']={'path':{'commands':commands},'fills':[],'strokes':[{'paint':{'type':'solid','color':[int(color[k:k+2],16)/255 for k in (1,3,5)]+[1]},'width':stroke['width'],'cap':'round','join':'round','miterLimit':4,'blendMode':'normal','opacity':100}]}
    layers.append(l)
doc['composition']={'id':'main','name':'Original editable route edges over frozen ground; unchanged spatial-v4 terrain' if args.quality_revision==9 else 'Original editable continuous-diagonal route ground; explicit spatial-v5 terrain','layers':list(reversed(layers))}
write(editable,doc);run(['project','commit','--project',str(project),'--file',str(editable)])
run(['preview','--project',str(project),'--time','1','--output',str(OUT/'ground-native.png')])
native=Image.open(OUT/'ground-native.png');assert native.mode=='RGBA' and native.getchannel('A').getextrema()==(255,255)
native.convert('RGB').save(OUT/'ground.png',optimize=True,compress_level=9)
assert not any(c.getbbox() for c in ImageChops.difference(native.convert('RGB'),Image.open(OUT/'ground.png')).split())
svgpath=' '.join(f'M{s["start"][0]},{s["start"][1]} L{s["end"][0]},{s["end"][1]}' for s in segments)
base_reference='../quality-v6/ground.png' if args.quality_revision==9 else 'material-ground.png'
svg=f'<svg xmlns="http://www.w3.org/2000/svg" width="768" height="1296"><image href="{base_reference}" width="768" height="1296"/>'+''.join(f'<path d="{svgpath}" fill="none" stroke="{s["color"]}" stroke-width="{s["width"]}" stroke-linecap="round"/>' for s in spec['strokes'])+'</svg>'
(OUT/'ground-layout.svg').write_text(svg+'\n')
write(OUT/'ground-native-evidence.json',{'commands':evidence,'geometry_hash':p['geometry_hash'],'painted_boundary_segments':len(segments),'frozen_input':{'path':base_reference,'sha256':sha(base_ground)},'native_output':{'path':'ground-native.png','sha256':sha(OUT/'ground-native.png')},'packaging':'Native RGBA alpha is uniformly255; runtime RGB removes only that redundant channel and applies lossless PNG compression. Every RGB pixel exactly equals actual native export; no palette quantization/resampling.','output':{'path':'ground.png','sha256':sha(OUT/'ground.png'),'bytes':(OUT/'ground.png').stat().st_size}})
print(json.dumps({'status':'RENDERED_SOURCE_NOT_ACCEPTED','segments':len(segments),'ground_bytes':(OUT/'ground.png').stat().st_size}))
