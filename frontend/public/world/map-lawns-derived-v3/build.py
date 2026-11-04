"""Original garden-interior art; execute only under a fresh finite source offer."""
from pathlib import Path
import hashlib
import importlib.util
import json
import math
import os
from PIL import Image, ImageDraw

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[3]
BASE=ROOT/'frontend/public/world/map-roof-street-derived-v2'
CONTRACT=json.loads((HERE/'paint-contract.json').read_text())
SIZE=(768,1296)
ERAS=('REVENGE','PARADE','DANGER')


def sha(raw):return hashlib.sha256(raw).hexdigest()
def encode(value):return (json.dumps(value,indent=2,ensure_ascii=True)+'\n').encode()
def pin(path):
    raw=path.read_bytes()
    return {'path':str(path.relative_to(ROOT)),'bytes':len(raw),'sha256':sha(raw)}
def save_json(path,value):path.write_bytes(encode(value))


def module(path,name):
    spec=importlib.util.spec_from_file_location(name,path)
    value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value)
    return value


def frozen_inputs():
    for record in CONTRACT['baselinePins']:
        actual=pin(ROOT/record['path']);assert actual==record,record['path']
    hp=CONTRACT['helperPin'];assert sha((ROOT/hp['path']).read_bytes())==hp['sha256']
    layout=json.loads((BASE/'layout.json').read_text())
    proof=json.loads((BASE/'source-provenance.json').read_text())
    for record in proof['sourceInputs']+proof['license']['inputPins']:
        actual=pin(ROOT/record['path'])
        assert actual['sha256']==record['sha256'].removeprefix('sha256:') and actual['bytes']==record['bytes']
    parent=module(ROOT/hp['path'],'frozen_lawn_ground')
    v2=module(BASE/'build.py','frozen_lawn_roofs')
    assert [list(map(list,p)) for p in parent.GARDENS]==CONTRACT['gardens']
    assert [list(map(list,p)) for p in parent.PATHS]==CONTRACT['paths']
    assert len(layout['decorations'])==285 and len(layout['serviceLabelAnchors'])==6
    return layout,proof,parent,v2


def edge_distance(px,py,a,b):
    dx,dy=b[0]-a[0],b[1]-a[1]
    t=max(0,min(1,((px-a[0])*dx+(py-a[1])*dy)/(dx*dx+dy*dy)))
    return math.hypot(px-a[0]-t*dx,py-a[1]-t*dy)


def inside(x,y,poly):
    result=False
    for a,b in zip(poly,poly[1:]+poly[:1]):
        if (a[1]>y)!=(b[1]>y) and x<(b[0]-a[0])*(y-a[1])/(b[1]-a[1])+a[0]:result=not result
    return result


def geometric_candidates(layout):
    result=[]
    labels=[a['exclusionRect'] for a in layout['serviceLabelAnchors']]
    paths=[(a,b) for path in CONTRACT['paths'] for a,b in zip(path,path[1:])]
    for region,poly in enumerate(CONTRACT['gardens']):
        for y in range(min(p[1] for p in poly),max(p[1] for p in poly)):
            for x in range(min(p[0] for p in poly),max(p[0] for p in poly)):
                px,py=x+.5,y+.5
                if not inside(px,py,poly):continue
                if min(edge_distance(px,py,a,b) for a,b in zip(poly,poly[1:]+poly[:1]))<22:continue
                if any(r['x']<=px<r['x']+r['width'] and r['y']<=py<r['y']+r['height'] for r in labels):continue
                if any(math.hypot(px-a['x'],py-a['y'])<=45 for a in layout['canonicalDisplayAnchors']):continue
                if any(edge_distance(px,py,a,b)<=23 for a,b in paths):continue
                result.append((x,y,region))
    return result


def occupancy(era,layout,proof,parent,v2):
    """Replay actual ordered RGBA projections and roof painters, never scene RGB."""
    canvas=Image.new('RGBA',SIZE)
    projections=[]
    crops={(p['themeId'],p['frame']):p for p in proof['originalCropPins']}
    atlas_path=ROOT/f'frontend/public/world/reference-scene-v2/map-cozy-v1/{era.lower()}/atlas.png'
    with Image.open(atlas_path) as source:
        atlas=source.convert('RGBA');cache={}
        for row in layout['decorations']:
            frame=row['frame'];record=crops[(era,frame)];r=record['sourceRect']
            sx,sy,sw,sh=(r[k] for k in ('x','y','width','height'))
            if frame not in cache:
                crop=atlas.crop((sx,sy,sx+sw,sy+sh))
                assert 'sha256:'+sha(crop.tobytes())==record['rgbaSha256']
                cache[frame]=crop
            x,y,w,h=row['rect'];end=math.floor(sh*parent.CANOPY_SOURCE_END) if row['crop_role']=='canopy' else sh
            scale=min(w/sw,h/end);rw,rh=max(1,math.floor(sw*scale+.5)),max(1,math.floor(end*scale+.5))
            dx,dy=x+(w-rw)//2,y+h-rh
            venue=frame in parent.VENUES;trim=math.floor(sh*parent.FACADE_SOURCE_START) if venue else 0
            fh=max(1,math.floor(rh*(1-parent.FACADE_SOURCE_START)*parent.FACADE_HEIGHT_SCALE+.5)) if venue else rh
            top=rh-fh
            layer=cache[frame].crop((0,trim,sw,end)).resize((rw,fh),Image.Resampling.NEAREST)
            canvas.alpha_composite(layer,(dx,dy+top))
            projections.append({'id':row['id'],'rect':[dx,dy+top,rw,fh]})
            if venue:
                # Every venue retains the complete old structural roof underlay.
                parent.overhead_roof(canvas,[],row['id'],dx,dy,rw,rh,parent.PALETTES[era])
                domain=[dx,dy,rw,max(top,math.ceil(rh*.72))]
                if not any(v2.intersects(domain,r) for r in v2.reserves(layout)):
                    v2.draw_roof(canvas,[],row,[dx,dy,rw,top],parent.PALETTES[era])
    return canvas.getchannel('A'),projections


def allowed_mask(era,layout,proof,parent,v2,candidates):
    ground,_,_,_=parent.surface(era)
    occupied,projections=occupancy(era,layout,proof,parent,v2)
    scene=Image.open(BASE/(era.lower()+'-scene.png')).convert('RGB')
    gp,op,bp=ground.load(),occupied.load(),scene.load()
    colors={tuple(bytes.fromhex(parent.PALETTES[era][k][1:])) for k in CONTRACT['groundRoles']}
    mask=Image.new('L',SIZE);mp=mask.load();counts=[0,0]
    for x,y,region in candidates:
        # Color is an additional ground constraint, never an occlusion shortcut.
        if not op[x,y] and gp[x,y][:3] in colors and bp[x,y]==gp[x,y][:3]:
            mp[x,y]=255;counts[region]+=1
    return mask,scene,counts,projections


def mix(a,b,f):
    return '#'+''.join(f'{round(x*(1-f)+y*f):02X}' for x,y in zip(bytes.fromhex(a[1:]),bytes.fromhex(b[1:])))


def draw_operations(era,mask,scene,parent):
    """Hand-authored micro planting vocabulary plus irregular low-contrast grain."""
    p=parent.PALETTES[era]
    ink={'grain':mix(p['garden'],p['lawnDark'],.65),
         'shade':mix(p['lawnDark'],'#617B60',.42),
         'leaf':mix(p['garden'],'#718B69',.52),
         'glint':mix(p['lawnLight'],'#E8E5B8',.36),
         'petal':{'REVENGE':'#E6B8BA','PARADE':'#E5D293','DANGER':'#D3DBBC'}[era]}
    # Rectangle sequences are original editable planting, not cropped source art.
    motifs=[
        [(3,2,2,7,'shade'),(0,2,3,2,'leaf'),(5,1,3,2,'leaf'),(1,6,2,2,'leaf'),(5,5,3,2,'leaf'),(3,0,2,2,'glint')],
        [(3,4,2,4,'shade'),(0,1,3,3,'leaf'),(5,1,3,3,'leaf'),(2,0,4,3,'leaf'),(1,1,1,1,'glint'),(6,1,1,1,'glint')],
        [(3,4,1,4,'shade'),(0,2,2,2,'petal'),(5,2,2,2,'petal'),(2,0,3,2,'petal'),(2,4,3,2,'petal'),(2,2,3,2,'glint'),(4,6,3,2,'leaf')],
        [(1,2,1,5,'shade'),(4,1,1,5,'shade'),(0,0,3,2,'glint'),(3,0,3,2,'glint'),(2,5,2,2,'leaf')],
    ]
    operations=[];mp,bp=mask.load(),scene.load();stats={'grain':0,'motifs':[0]*4}
    def fits(x,y,w,h,color=None):
        if x<0 or y<0 or x+w>768 or y+h>1296:return False
        rgb=tuple(bytes.fromhex(color[1:])) if color else None
        return all(mp[xx,yy]==255 and (rgb is None or bp[xx,yy]!=rgb) for yy in range(y,y+h) for xx in range(x,x+w))
    def add(x,y,w,h,color):
        if not fits(x,y,w,h,color):return False
        operations.append({'id':f'ground_{len(operations):05d}','kind':'rect','x':x,'y':y,'width':w,'height':h,'fill':color});return True
    for y in range(390,1035,2):
        for x in range(188,548,3):
            phase=math.sin(x/21)+math.cos(y/29)+math.sin((x+y)/37)
            seed=(x*73+y*139+x*y)%31
            if phase>.05 and seed<18 and add(x,y,3 if seed%3 else 2,1 if seed%2 else 2,ink['grain']):stats['grain']+=1
    for y in range(397,1028,12):
        for x in range(195+(y//12%3)*3,540,12):
            seed=(x*43+y*71)%97;xx=x+seed%7-3;yy=y+(seed//7)%5-2
            chosen=seed%4;motif=motifs[chosen]
            # Accept/reject the entire motif's PAINTED footprint together. Its
            # transparent gaps need no permission; no painted rect is clipped.
            if all(fits(xx+dx,yy+dy,w,h,ink[k]) for dx,dy,w,h,k in motif):
                for dx,dy,w,h,k in motif:add(xx+dx,yy+dy,w,h,ink[k])
                stats['motifs'][chosen]+=1
    image=Image.new('RGBA',SIZE);draw=ImageDraw.Draw(image)
    for r in operations:
        x,y,w,h=(r[k] for k in ('x','y','width','height'))
        draw.rectangle((x,y,x+w-1,y+h-1),fill=r['fill'])
    return operations,image,stats


def main():
    layout,proof,parent,v2=frozen_inputs();candidates=geometric_candidates(layout)
    results={};records={}
    for era in ERAS:
        name=era.lower();mask,scene,counts,projections=allowed_mask(era,layout,proof,parent,v2,candidates)
        operations,overlay,stats=draw_operations(era,mask,scene,parent)
        changed=sum(1 for a in overlay.getchannel('A').tobytes() if a)
        print(json.dumps({'era':era,'allowedPerGarden':counts,'changedPixels':changed,'design':stats}),flush=True)
        assert changed>0 and all(counts),'No eligible art; report constraint, never widen predicate'
        overlay.save(HERE/(name+'-overlay.png'),compress_level=9)
        mask.save(HERE/(name+'-allowed-mask.png'),compress_level=9)
        save_json(HERE/(name+'-operations.json'),operations)
        svg='<svg xmlns="http://www.w3.org/2000/svg" width="768" height="1296" viewBox="0 0 768 1296" shape-rendering="crispEdges">'
        for op in operations:svg+=f'<rect id="{op["id"]}" x="{op["x"]}" y="{op["y"]}" width="{op["width"]}" height="{op["height"]}" fill="{op["fill"]}"/>'
        (HERE/(name+'-overlay.svg')).write_text(svg+'</svg>\n')
        mask_counts=[0,0];rawalpha=overlay.getchannel('A').load()
        for x,y,r in candidates:
            if rawalpha[x,y]:mask_counts[r]+=1
        records[era]={'overlay':pin(HERE/(name+'-overlay.png')),'svg':pin(HERE/(name+'-overlay.svg')),
                      'operationsPin':pin(HERE/(name+'-operations.json')),'allowedMaskPin':pin(HERE/(name+'-allowed-mask.png')),
                      'allowedMaskEncoding':'PNG_L8_BINARY_0_255','allowedMaskSha256':sha(mask.tobytes()),
                      'allowedPixels':sum(counts),'changedPixels':changed,'allowedPerGarden':counts,'changedPerGarden':mask_counts,'design':stats,
                      'projectionRecordsSha256':sha(encode(projections))}
        results[era]={'allowed':counts,'changed':changed,'operations':len(operations)}
    provenance={'format':'MCR_GROUND_OVERLAY_PROVENANCE_V1','sourceCommit':CONTRACT['sourceCommit'],
                'sourceImplementationCommit':os.environ['MCR_ART_SOURCE_COMMIT'],'dimensions':list(SIZE),
                'baselinePins':CONTRACT['baselinePins'],'helperPin':CONTRACT['helperPin'],
                'contractPin':pin(HERE/'paint-contract.json'),'recipePin':pin(HERE/'build.py'),
                'sourceInputs':proof['sourceInputs'],'inheritedLicense':proof['license'],'eras':records,
                'originalArt':'Original project-authored irregular ground grain, fern tufts, clover, seed heads and small flowers. No copied character, crop, text, font or external asset.',
                'authority':'MAP_ILLUSTRATION_ONLY_NO_WORLD_OR_PHYSICAL_AUTHORITY',
                'limits':['Overlay source only; not an App/native/live Map capture.','Artist231 alone qualifies the final combined facade+lawn composition before Map registration.','No v1/v2/original source, layout, World/C06 identity, label, collision, spawn, interaction or character mutation.']}
    save_json(HERE/'source-provenance.json',provenance)
    print(json.dumps({'result':'BUILT_NOT_YET_QUALIFIED','actual':results}))


if __name__=='__main__':main()
