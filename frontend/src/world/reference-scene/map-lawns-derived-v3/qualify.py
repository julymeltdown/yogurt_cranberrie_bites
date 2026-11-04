"""Independent pure lawn predicates/replay; CLI evidence under admitted slot only."""
from pathlib import Path
import hashlib
import importlib.util
import json
import math
import os
import re
import xml.etree.ElementTree as ET
from PIL import Image, ImageDraw

ROOT=Path(__file__).resolve().parents[5]
PACK=ROOT/'frontend/public/world/map-lawns-derived-v3'
BASE=ROOT/'frontend/public/world/map-roof-street-derived-v2'
EVIDENCE=ROOT/'docs/mcr/art/evidence/map-lawns-derived-v3'
SIZE=(768,1296)


def check(test,message):
    if not test:raise ValueError(message)
def sha(raw):return hashlib.sha256(raw).hexdigest()
def encode(value):return (json.dumps(value,indent=2,ensure_ascii=True)+'\n').encode()
def pin(path):
    raw=path.read_bytes();return {'path':str(path.relative_to(ROOT)),'bytes':len(raw),'sha256':sha(raw)}
def load(path,name):
    spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m


def segment_sq(x,y,a,b):
    dx,dy=b[0]-a[0],b[1]-a[1]
    along=(x-a[0])*dx+(y-a[1])*dy
    length=dx*dx+dy*dy
    if along<=0:return (x-a[0])**2+(y-a[1])**2
    if along>=length:return (x-b[0])**2+(y-b[1])**2
    return ((x-a[0])*dy-(y-a[1])*dx)**2/length


def winding_inside(x,y,points):
    winding=0
    for a,b in zip(points,points[1:]+points[:1]):
        cross=(b[0]-a[0])*(y-a[1])-(x-a[0])*(b[1]-a[1])
        if a[1]<=y<b[1] and cross>0:winding+=1
        if b[1]<=y<a[1] and cross<0:winding-=1
    return winding!=0


def geometry_ok(x,y,contract,layout):
    if not any(winding_inside(x,y,p) and all(segment_sq(x,y,a,b)>=22**2 for a,b in zip(p,p[1:]+p[:1])) for p in contract['gardens']):return False
    if any(segment_sq(x,y,a,b)<=23**2 for p in contract['paths'] for a,b in zip(p,p[1:])):return False
    for a in layout['serviceLabelAnchors']:
        r=a['exclusionRect']
        if r['x']<=x<r['x']+r['width'] and r['y']<=y<r['y']+r['height']:return False
    return not any((x-a['x'])**2+(y-a['y'])**2<=45**2 for a in layout['canonicalDisplayAnchors'])


def projected_alpha(era,layout,proof,parent,v2):
    """Independent facade projections come from frozen SVG, not new builder math."""
    ns='{http://www.w3.org/2000/svg}'
    tree=ET.fromstring((BASE/(era.lower()+'-scene.svg')).read_bytes())
    ids={r['id'] for r in layout['decorations']}
    projections={n.attrib['id']:n.attrib for n in tree.iter(ns+'svg') if n.attrib.get('id') in ids}
    check(set(projections)==ids,'all285 original SVG projections')
    plans={p['id']:p for p in proof['roofPlansByTheme'][era]}
    crops={(p['themeId'],p['frame']):p for p in proof['originalCropPins']}
    occupied=Image.new('RGBA',SIZE);records=[]
    atlas_path=ROOT/f'frontend/public/world/reference-scene-v2/map-cozy-v1/{era.lower()}/atlas.png'
    with Image.open(atlas_path) as raw:
        atlas=raw.convert('RGBA')
        for row in layout['decorations']:
            a=projections[row['id']]
            x,y,w,h=(int(a[k]) for k in ('x','y','width','height'))
            sx,sy,sw,sh=map(int,a['viewBox'].split())
            occupied.alpha_composite(atlas.crop((sx,sy,sx+sw,sy+sh)).resize((w,h),Image.Resampling.NEAREST),(x,y))
            records.append({'id':row['id'],'rect':[x,y,w,h]})
            if row['frame'] not in parent.VENUES:continue
            if row['id'] in plans:
                dx,dy,rw,rh=plans[row['id']]['renderedRect']
                check((dx,rw)==(x,w),'roof source width/position')
                parent.overhead_roof(occupied,[],row['id'],dx,dy,rw,rh,parent.PALETTES[era])
                v2.draw_roof(occupied,[],row,[dx,dy,rw,y-dy],parent.PALETTES[era])
            else:
                # Whole protected-adjacent roofs are the pinned incumbent painter.
                source=crops[(era,row['frame'])]['sourceRect'];_,ry,rw0,rh0=row['rect']
                scale=min(rw0/source['width'],rh0/source['height'])
                rw=max(1,math.floor(source['width']*scale+.5));rh=max(1,math.floor(source['height']*scale+.5))
                dy=ry+rh0-rh
                check(rw==w,'protected roof projection width')
                parent.overhead_roof(occupied,[],row['id'],x,dy,rw,rh,parent.PALETTES[era])
    return occupied.getchannel('A'),records


def independent_mask(era,contract,layout,proof,parent,v2):
    occupied,records=projected_alpha(era,layout,proof,parent,v2)
    ground,_,_,_=parent.surface(era)
    base=Image.open(BASE/(era.lower()+'-scene.png')).convert('RGB')
    palette={tuple(bytes.fromhex(parent.PALETTES[era][r][1:])) for r in ('lawn','lawnLight','lawnDark','garden')}
    mask=Image.new('L',SIZE);m=mask.load();o,g,b=occupied.load(),ground.load(),base.load()
    # Winding/squared-distance recomputation deliberately differs from the builder.
    for y in range(390,1035):
        for x in range(188,548):
            if o[x,y] or g[x,y][:3] not in palette or b[x,y]!=g[x,y][:3]:continue
            if geometry_ok(x+.5,y+.5,contract,layout):m[x,y]=255
    return mask,base,records


def validate_ops(operations,mask,base):
    check(type(operations) is list,'operations array')
    image=Image.new('RGBA',base.size);draw=ImageDraw.Draw(image);m,b=mask.load(),base.load()
    for i,r in enumerate(operations):
        check(type(r) is dict and set(r)=={'id','kind','x','y','width','height','fill'},'closed rect operation')
        check(r['id']==f'ground_{i:05d}' and r['kind']=='rect','operation identity/order')
        check(all(type(r[k]) is int for k in ('x','y','width','height')),'integer operation')
        x,y,w,h=(r[k] for k in ('x','y','width','height'))
        check(0<=x<x+w<=base.width and 0<=y<y+h<=base.height,'whole rect bounds')
        check(type(r['fill']) is str and re.fullmatch('#[0-9A-F]{6}',r['fill']) is not None,'explicit uppercase RGB')
        color=tuple(bytes.fromhex(r['fill'][1:]))
        for yy in range(y,y+h):
            for xx in range(x,x+w):
                check(m[xx,yy]==255,'paint outside Root domain')
                check(b[xx,yy]!=color,'opaque unchanged pixel')
        draw.rectangle((x,y,x+w-1,y+h-1),fill=r['fill'])
    return image


def svg_ops(raw,operations):
    check(b'<!' not in raw and b'<?' not in raw,'no XML directives')
    root=ET.fromstring(raw);ns='{http://www.w3.org/2000/svg}'
    check(root.tag==ns+'svg' and root.attrib=={'width':'768','height':'1296','viewBox':'0 0 768 1296','shape-rendering':'crispEdges'},'closed transparent SVG')
    check(len(root)==len(operations),'SVG operation count')
    for node,op in zip(root,operations):
        attrs={k:str(op[k]) for k in ('id','x','y','width','height','fill')}
        check(node.tag==ns+'rect' and node.attrib==attrs and len(node)==0,'SVG exact ordered rect equivalence')


def main():
    contract=json.loads((PACK/'paint-contract.json').read_text());proof=json.loads((BASE/'source-provenance.json').read_text());layout=json.loads((BASE/'layout.json').read_text())
    check(contract['sourceCommit']=='da33467cd084ecba071244a1939b6217f4b0de02','frozen v2 revision')
    for expected in contract['baselinePins']:check(pin(ROOT/expected['path'])==expected,'v2 immutable file pin')
    hp=contract['helperPin'];check(sha((ROOT/hp['path']).read_bytes())==hp['sha256'],'immutable ground helper')
    for expected in proof['sourceInputs']+proof['license']['inputPins']:
        actual=pin(ROOT/expected['path'])
        check(actual['sha256']==expected['sha256'].removeprefix('sha256:') and actual['bytes']==expected['bytes'],'immutable original/license input')
    check((contract['gardenEdgeInset'],contract['pathClearanceStrict'],contract['canonicalRadiusStrict'])==(22,23,45),'Root distances')
    check(contract['groundRoles']==['lawn','lawnLight','lawnDark','garden'],'Root four ground roles')
    parent=load(ROOT/hp['path'],'independent_frozen_ground');v2=load(BASE/'build.py','independent_frozen_roofs')
    check(contract['gardens']==[list(map(list,p)) for p in parent.GARDENS] and contract['paths']==[list(map(list,p)) for p in parent.PATHS],'Root/helper polygons and paths')
    check(len(layout['decorations'])==285 and len(layout['serviceLabelAnchors'])*3==18,'incumbents/all18 era label instances')
    provenance=json.loads((PACK/'source-provenance.json').read_text());results={};interface_eras={}
    check(provenance['sourceCommit']==contract['sourceCommit'] and provenance['sourceImplementationCommit']==os.environ['MCR_ART_SOURCE_COMMIT'],'provenance source revisions')
    check(provenance['baselinePins']==contract['baselinePins'] and provenance['helperPin']==hp,'provenance immutable readset')
    check(provenance['contractPin']==pin(PACK/'paint-contract.json') and provenance['recipePin']==pin(PACK/'build.py'),'provenance authored contract/recipe pins')
    check(provenance['sourceInputs']==proof['sourceInputs'] and provenance['inheritedLicense']==proof['license'],'provenance original/license lineage')
    for era in ('REVENGE','PARADE','DANGER'):
        name=era.lower();record=provenance['eras'][era]
        mask,base,projections=independent_mask(era,contract,layout,proof,parent,v2)
        check(sha(encode(projections))==record['projectionRecordsSha256'],'independent actual285projection identity')
        actual_mask=Image.open(PACK/(name+'-allowed-mask.png'))
        check(actual_mask.mode=='L' and actual_mask.size==SIZE and actual_mask.tobytes()==mask.tobytes(),'independent exact allowed mask')
        check(sha(mask.tobytes())==record['allowedMaskSha256'],'raw L8 mask digest')
        operations=json.loads((PACK/(name+'-operations.json')).read_text())
        replayed=validate_ops(operations,mask,base)
        overlay=Image.open(PACK/(name+'-overlay.png'))
        check(overlay.mode=='RGBA' and overlay.size==SIZE and overlay.tobytes()==replayed.tobytes(),'every overlay RGBA pixel replay')
        svg_ops((PACK/(name+'-overlay.svg')).read_bytes(),operations)
        alpha=overlay.getchannel('A').tobytes();allowed=mask.tobytes();original=base.tobytes();rgba=overlay.tobytes()
        changed=0
        for i,a in enumerate(alpha):
            check(a in (0,255),'opaque paint or alpha0 only')
            if a:
                check(allowed[i]==255 and rgba[i*4:i*4+3]!=original[i*3:i*3+3],'actual authored changed eligible pixel');changed+=1
            else:check(rgba[i*4:i*4+4]==b'\0\0\0\0','canonical transparent pixels')
        composed=Image.alpha_composite(base.convert('RGBA'),overlay).convert('RGB').tobytes()
        check(all(composed[i*3:i*3+3]==original[i*3:i*3+3] for i,a in enumerate(alpha) if not a),'every protected/unpainted pixel exact')
        check(changed==record['changedPixels'] and changed>0,'changed pixel count')
        check(sum(1 for v in allowed if v)==record['allowedPixels'],'allowed count')
        for key in ('overlay','svg','operationsPin','allowedMaskPin'):check(pin(ROOT/record[key]['path'])==record[key],'output byte pin '+key)
        results[era]={'allowedPixels':record['allowedPixels'],'changedPixels':changed,'operations':len(operations),'rgbaReplayExact':True,'svgOperationsExact':True,'protectedPixelsExact':True,'independentMaskExact':True}
        interface_eras[era]={k:record[k] for k in ('overlay','svg','operationsPin','allowedMaskPin','allowedMaskEncoding','allowedMaskSha256','allowedPixels','changedPixels','allowedPerGarden','changedPerGarden')}
        interface_eras[era]['operations']=operations
        print(json.dumps({'era':era,'result':'PASS',**results[era]}),flush=True)
    evidence={'format':'MCR_GROUND_OVERLAY_QUALIFICATION_V1','status':'PASS','sourceImplementationCommit':os.environ['MCR_ART_SOURCE_COMMIT'],
              'sourceCommit':contract['sourceCommit'],'provenancePin':pin(PACK/'source-provenance.json'),'contractPin':pin(PACK/'paint-contract.json'),
              'layoutPin':pin(BASE/'layout.json'),'incumbentRecords':285,'protectedEraLabelRectangles':18,'originalGeometryMutation':False,'results':results,
              'limits':['Source overlay only, not App/native/live Map acceptance.','Owner image inspection is a separate recorded assessment.','Only Artist231 composes and qualifies the final combined source pack before later explicit consumer renewal.']}
    path=EVIDENCE/'source-qualification.json';path.write_bytes(encode(evidence))
    # The guard binds the final raw FULL receipt after all children and inspection.
    # This draft is not consumable until that final binding is written.
    interface={'format':'MCR_GROUND_OVERLAY_V1','sourceCommit':contract['sourceCommit'],'baselinePins':contract['baselinePins'],'helperPin':hp,
               'dimensions':list(SIZE),'eras':interface_eras,'provenancePin':pin(PACK/'source-provenance.json'),
               'qualification':{'sourceCommit':os.environ['MCR_ART_SOURCE_COMMIT'],'status':'PENDING_FULL'}}
    (PACK/'interface.json').write_bytes(encode(interface))
    print('PASS: independently recomputed masks, ordered285 projections/all roof layers, all RGBA/SVG operations and protected pixels; no layout or World authority changes')


if __name__=='__main__':main()
