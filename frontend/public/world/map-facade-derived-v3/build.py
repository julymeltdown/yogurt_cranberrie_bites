"""Project-authored facade layers and the single qualified ground/facade compositor."""
from __future__ import annotations
import argparse
import base64
import copy
import hashlib
import importlib.util
import io
import json
import math
import xml.etree.ElementTree as ET
from pathlib import Path
from PIL import Image, ImageChops, ImageDraw

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[3]
BASE=ROOT/'frontend/public/world/map-roof-street-derived-v2'
OLD=ROOT/'frontend/public/world/map-reference-composition-v1'
VERSION='map-facade-derived-v3'
SOURCE='da33467cd084ecba071244a1939b6217f4b0de02'
PUBLICATION='efa9d95653de7595273c903741fb283bcfe3d90f'
SIZE=(768,1296)
ERAS=('REVENGE','PARADE','DANGER')
FROZEN={
    "README.md": "5994b8074a7c201b001bb0ebe04bec6b39deb4826f5790c41b04fb83c4684311",
    "build.py": "4c192b13a967bb490a1318f5a6cce0944e5d52ee91e33f1bdc233432b0ae1314",
    "danger-scene.png": "b4dbf3f41e392e2f37ce78f907336055938c4b6611925c79ad13f41b15d2e931",
    "danger-scene.svg": "995d576574c0221d61599db395061f26b8a0208db97cc97775e01e764ce1c513",
    "layout.json": "d9e401f6254df05530174b04b16655aa5f89fed68fa02c90f6a653e6a8010e91",
    "parade-scene.png": "d820853d14efe04f84cd8b8f08da5bc8c67850f3b19c43170b237207583a93c2",
    "parade-scene.svg": "956f6a6aab8ceec3756d4ad60a45aeed9b1b67a99ebd31fcf5f08f8b867dbcc3",
    "revenge-scene.png": "5669a7861851a1b95bd1f2ad48e222f2e73dcd471622405f48000a33419f1606",
    "revenge-scene.svg": "6c1d91478f8131e347f96b0a8a5c186618196d282fac366afe861e6080ce8e8b",
    "source-provenance.json": "259a407d711e520e4abc5c0d8c3259cf118d3511d9effd9198521a5b7d996990"
}
HELPER_SHA='6bf696b7c3edd5adf35e0b5995ca9cd33f915e87898de0e9266f2923cffc0433'
BOUNDS={
 'map_only_stage_west_wing':[159,361,164,33],
 'map_only_stage_central_hall':[307,361,199,41],
 'map_only_stage_east_wing':[456,362,161,33],
}

def sha(raw): return 'sha256:'+hashlib.sha256(raw).hexdigest()
def hexsha(value):
    result=value.removeprefix('sha256:')
    assert len(result)==64 and all(c in '0123456789abcdef' for c in result)
    return result
def encode(value): return (json.dumps(value,indent=2,ensure_ascii=True)+'\n').encode()
def pin(path,raw): return dict(path=str(path),bytes=len(raw),sha256=sha(raw))
def read_pin(p):
    path=ROOT/p['path']
    assert path.resolve().is_relative_to(ROOT) and not path.is_symlink()
    raw=path.read_bytes()
    assert len(raw)==p['bytes'] and hexsha(sha(raw))==hexsha(p['sha256']),p['path']
    return raw
def load_module(path,name,expected):
    assert sha(path.read_bytes())=='sha256:'+expected
    spec=importlib.util.spec_from_file_location(name,path)
    result=importlib.util.module_from_spec(spec);spec.loader.exec_module(result)
    return result
def png(im):
    out=io.BytesIO();im.save(out,format='PNG',compress_level=9);return out.getvalue()
def changed(a,b):
    cs=ImageChops.difference(a.convert('RGB'),b.convert('RGB')).split()
    return ImageChops.lighter(ImageChops.lighter(cs[0],cs[1]),cs[2]).point(lambda v:255 if v else 0)
def binary(alpha): return alpha.point(lambda v:255 if v else 0)
def inside_rect(x,y,r): return r[0]<=x<r[0]+r[2] and r[1]<=y<r[1]+r[3]
def rect_mask(rect):
    im=Image.new('L',SIZE);x,y,w,h=rect;ImageDraw.Draw(im).rectangle((x,y,x+w-1,y+h-1),fill=255);return im
def placed_alpha(local,position):
    im=Image.new('L',SIZE)
    if local is not None: im.paste(binary(local.getchannel('A')),position)
    return im
def labels(layout):
    return [[r['exclusionRect'][k] for k in ('x','y','width','height')] for r in layout['serviceLabelAnchors']]
def distance(p,a,b):
    dx,dy=b[0]-a[0],b[1]-a[1]
    t=max(0,min(1,((p[0]-a[0])*dx+(p[1]-a[1])*dy)/(dx*dx+dy*dy))) if dx or dy else 0
    return math.hypot(p[0]-a[0]-t*dx,p[1]-a[1]-t*dy)
def polygon_inside(p,poly):
    result=False
    for a,b in zip(poly,poly[1:]+poly[:1]):
        if (a[1]>p[1])!=(b[1]>p[1]) and p[0]<(b[0]-a[0])*(p[1]-a[1])/(b[1]-a[1])+a[0]: result=not result
    return result
def edges(poly,closed=False): return list(zip(poly,poly[1:]+(poly[:1] if closed else [])))

def projected(row,crop):
    f=crop['sourceRect'];sw,sh=f['width'],f['height'];x,y,w,h=row['rect']
    end=sh
    scale=min(w/sw,h/end);rw,rh=max(1,math.floor(sw*scale+.5)),max(1,math.floor(end*scale+.5))
    venue=row['frame'] in ('meeting_pavilion','giveaway_pavilion','north_hall','west_hall')
    trim=math.floor(sh*.54) if venue else 0
    fh=max(1,math.floor(rh*.46*.80+.5)) if venue else rh
    return [x+(w-rw)//2,y+h-rh,rw,rh,trim,fh]

def layers_for(era,layout,proof,parent,v2):
    original={(p['themeId'],p['frame']):p for p in proof['originalCropPins']}
    atlas_path=ROOT/f'frontend/public/world/reference-scene-v2/map-cozy-v1/{era.lower()}/atlas.png'
    with Image.open(atlas_path) as atlas:
        assert atlas.mode=='RGBA'
        layers=[];cache={}
        for row in layout['decorations']:
            crop=original[(era,row['frame'])];f=crop['sourceRect'];sx,sy,sw,sh=[f[k] for k in ('x','y','width','height')]
            if row['frame'] not in cache:
                raw=atlas.crop((sx,sy,sx+sw,sy+sh));assert sha(raw.tobytes())==crop['rgbaSha256'];cache[row['frame']]=raw
            dx,dy,rw,rh,trim,fh=projected(row,crop)
            local=cache[row['frame']].crop((0,trim,sw,sh)).resize((rw,fh),Image.Resampling.NEAREST)
            roof=Image.new('RGBA',SIZE);vectors=[]
            if row['frame'] in parent.VENUES:
                parent.overhead_roof(roof,vectors,row['id'],dx,dy,rw,rh,parent.PALETTES[era])
                domain=[dx,dy,rw,max(rh-fh,math.ceil(rh*.72))]
                if not any(v2.intersects(domain,r) for r in labels(layout)):
                    v2.draw_roof(roof,vectors,row,[dx,dy,rw,rh-fh],parent.PALETTES[era])
            bbox=roof.getbbox();roof_position=bbox[:2] if bbox else (0,0)
            layers.append(dict(row=row,projection=[dx,dy,rw,rh,trim,fh],sourceRect=f,local=local,
                               position=(dx,dy+rh-fh),roof=roof.crop(bbox) if bbox else None,
                               roofPosition=roof_position,roofVectors=vectors))
        return layers,atlas_path.read_bytes()

def masks_for(layout,proof,era,layers):
    roofs=Image.new('L',SIZE);foreground=Image.new('L',SIZE);all_scenery=Image.new('L',SIZE)
    for layer in layers:
        roofs=ImageChops.lighter(roofs,placed_alpha(layer['roof'],layer['roofPosition']))
        all_scenery=ImageChops.lighter(all_scenery,placed_alpha(layer['local'],layer['position']))
    actual_roofs=roofs.copy()
    # Facades additionally exclude full roof domains. Ground permissions exclude
    # actual opacity, not unused empty parts of those illustration bounds.
    for plan in proof['roofPlansByTheme'][era]: roofs=ImageChops.lighter(roofs,rect_mask(plan['roofDomain']))
    excluded=roofs.copy();draw=ImageDraw.Draw(excluded)
    for r in labels(layout):
        x,y,w,h=r;draw.rectangle((x,y,x+w-1,y+h-1),fill=255)
    # Pixel centers; <=45 is excluded. Do not substitute a rounded ellipse.
    xp=excluded.load()
    for a in layout['canonicalDisplayAnchors']:
        for y in range(max(0,a['y']-46),min(SIZE[1],a['y']+46)):
            for x in range(max(0,a['x']-46),min(SIZE[0],a['x']+46)):
                if math.hypot(x+.5-a['x'],y+.5-a['y'])<=45: xp[x,y]=255
    result={}
    for layer in reversed(layers):
        row=layer['row']
        if row['id'] in BOUNDS:
            candidate=ImageChops.darker(placed_alpha(layer['local'],layer['position']),rect_mask(BOUNDS[row['id']]))
            result[row['id']]=ImageChops.subtract(candidate,ImageChops.lighter(excluded,foreground))
        foreground=ImageChops.lighter(foreground,placed_alpha(layer['local'],layer['position']))
        foreground=ImageChops.lighter(foreground,placed_alpha(layer['roof'],layer['roofPosition']))
    return result,ImageChops.lighter(all_scenery,actual_roofs),actual_roofs

def ground_mask(layout,parent,scenery,era,baseline):
    mask=Image.new('L',SIZE);pixels=mask.load();occupied=scenery.load()
    ground,_,_,_=parent.surface(era);gp=ground.load();bp=baseline.load()
    roles={tuple(bytes.fromhex(parent.PALETTES[era][k][1:])) for k in ('lawn','lawnLight','lawnDark','garden')}
    path_edges=[e for path in parent.PATHS for e in edges(path)]
    for poly in parent.GARDENS:
        lines=edges(poly,True)
        for y in range(max(0,min(p[1] for p in poly)),min(SIZE[1],max(p[1] for p in poly)+1)):
            for x in range(max(0,min(p[0] for p in poly)),min(SIZE[0],max(p[0] for p in poly)+1)):
                q=(x+.5,y+.5)
                if occupied[x,y] or gp[x,y][:3] not in roles or bp[x,y][:3]!=gp[x,y][:3] or any(inside_rect(x,y,r) for r in labels(layout)): continue
                if any(math.hypot(q[0]-a['x'],q[1]-a['y'])<=45 for a in layout['canonicalDisplayAnchors']): continue
                if not polygon_inside(q,poly) or min(distance(q,a,b) for a,b in lines)<22: continue
                if min(distance(q,a,b) for a,b in path_edges)<=23: continue
                pixels[x,y]=255
    return mask

def fits(mask,rect):
    x,y,w,h=rect
    return w>0 and h>0 and 0<=x<x+w<=SIZE[0] and 0<=y<y+h<=SIZE[1] and mask.crop((x,y,x+w,y+h)).histogram()[255]==w*h

def facade_components(name,mask,era):
    x,y,w,h=BOUNDS[name];components=[]
    variant=list(BOUNDS).index(name)
    wall=('#ecd4bb','#ede0bf','#e5d9be')[variant];stone='#c0a38b';edge='#8a6d68';ink='#6c6166';glow='#f7deb0'
    def add(role,rect,ops):
        if fits(mask,rect):
            components.append(dict(id=name+'_'+role+'_'+str(len(components)),role=role,rect=rect,operations=ops))
            return True
        return False
    def op(rect,color):return dict(kind='rect',rect=rect,rgba=list(bytes.fromhex(color[1:]))+[255])
    # Authored clear gaps remove this facade's own alpha. The ground/previous
    # layers are revealed by recomposition; no copied pixels or flat cover.
    for fraction in (.28,.70):
        cx=x+round(w*fraction)
        found=False
        for gh in (h-8,24,18,14):
            if found:break
            for shift in (0,-3,3,-6,6):
                r=[cx+shift,y+4,6,gh]
                if add('negative_gap',r,[dict(kind='clear',rect=r)]):found=True;break
    for i,fraction in enumerate((.08,.40,.78)):
        bx=x+round(w*fraction);by=y+6;bw,bh=24,min(24,h-10)
        for shift in (0,-4,4,-8,8):
            r=[bx+shift,by,bw,bh];px,py,_,_=r
            ops=[op(r,wall),op([px,py,bw,2],edge),op([px,py+bh-3,bw,3],stone),
                 op([px+3,py+3,bw-6,bh-8],ink),op([px+5,py+5,bw-10,bh-12],glow),
                 op([px+bw//2-1,py+4,2,bh-10],edge),op([px+2,py+bh-6,bw-4,2],edge),
                 op([px+1,py+3,2,bh-6],stone),op([px+bw-3,py+3,2,bh-6],stone)]
            if i==1:
                ops += [op([px+4,py+bh-4,bw-8,2],'#b4bd97'),op([px+6,py+bh-6,3,2],'#e6b5ac'),
                        op([px+14,py+bh-6,3,2],'#e6b5ac')]
            if add('recessed_bay',r,ops):break
    # Avoid later bay primitives refilling an authored gap.
    gaps=[c['rect'] for c in components if c['role']=='negative_gap']
    components=[c for c in components if c['role']=='negative_gap' or
                not any(c['rect'][0]<g[0]+g[2] and g[0]<c['rect'][0]+c['rect'][2] and c['rect'][1]<g[1]+g[3] and g[1]<c['rect'][1]+c['rect'][3] for g in gaps)]
    assert any(c['role']=='negative_gap' for c in components),('No actual gap',name)
    assert any(c['role']=='recessed_bay' for c in components),('No facade articulation',name)
    return components

def apply_facade(layer,components,mask):
    edited=layer['local'].copy();dx,dy,rw,rh,trim,fh=layer['projection'];origin_y=dy+rh-fh
    draw=ImageDraw.Draw(edited)
    for component in components:
        assert fits(mask,component['rect']),component['id']
        for operation in component['operations']:
            r=operation['rect'];assert fits(mask,r)
            x,y,w,h=r
            assert inside_rect(x,y,component['rect']) and inside_rect(x+w-1,y+h-1,component['rect'])
            draw.rectangle((x-dx,y-origin_y,x-dx+w-1,y-origin_y+h-1),
                           fill=(0,0,0,0) if operation['kind']=='clear' else tuple(operation['rgba']))
    return edited

def source_svg(layer,components):
    row=layer['row'];dx,dy,rw,rh,trim,fh=layer['projection'];f=layer['sourceRect']
    sprite=f'<svg id="{row["id"]}" x="{dx}" y="{dy+rh-fh}" width="{rw}" height="{fh}" viewBox="{f["x"]} {f["y"]+trim} {f["width"]} {f["height"]-trim}" preserveAspectRatio="none"><use xlink:href="#atlas"/></svg>'
    if not components:return sprite
    # A union of retained source strips represents true removed source areas,
    # rather than hiding old pixels beneath a painted background rectangle.
    discarded=[c['rect'] for c in components];strips=[]
    for yy in range(dy+rh-fh,dy+rh):
        intervals=sorted((max(dx,r[0]),min(dx+rw,r[0]+r[2])) for r in discarded if r[1]<=yy<r[1]+r[3])
        cursor=dx
        for left,right in intervals:
            if cursor<left:strips.append(f'<rect x="{cursor}" y="{yy}" width="{left-cursor}" height="1"/>')
            cursor=max(cursor,right)
        if cursor<dx+rw:strips.append(f'<rect x="{cursor}" y="{yy}" width="{dx+rw-cursor}" height="1"/>')
    clip=row['id']+'_retained_original_source'
    result=f'<defs><clipPath id="{clip}" clipPathUnits="userSpaceOnUse">{"".join(strips)}</clipPath></defs><g clip-path="url(#{clip})">{sprite}</g>'
    for c in components:
        result+=f'<g id="{c["id"]}" data-role="{c["role"]}" shape-rendering="crispEdges">'
        for op in c['operations']:
            if op['kind']=='clear':continue
            x,y,w,h=op['rect'];color='#'+''.join(f'{v:02x}' for v in op['rgba'][:3])
            result+=f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{color}"/>'
        result+='</g>'
    return result

def produce(ground_binding):
    frozen={n:(BASE/n).read_bytes() for n in FROZEN}
    assert all(sha(raw)=='sha256:'+FROZEN[n] for n,raw in frozen.items()),'Frozen v2 drift'
    layout=json.loads(frozen['layout.json']);proof=json.loads(frozen['source-provenance.json'])
    assert len(layout['decorations'])==285 and set(BOUNDS)<=set(r['id'] for r in layout['decorations'])
    for p in proof['sourceInputs']:read_pin(p)
    parent=load_module(OLD/'build.py','frozen_ground_helper_v3',HELPER_SHA)
    v2=load_module(BASE/'build.py','frozen_roof_primitives_v3',FROZEN['build.py'])
    ground=read_ground(ground_binding,layout,parent)  # Defined by the agreed owner ABI below.
    outputs={};assets=[];actual={};plans={};mask_pins={};ground_mask_pins={}
    for era in ERAS:
        name=era.lower();layers,atlas=layers_for(era,layout,proof,parent,v2)
        facade_masks,scenery,roof_mask=masks_for(layout,proof,era,layers)
        with Image.open(io.BytesIO(frozen[name+'-scene.png'])) as im: baseline_rgb=im.convert('RGB')
        gm=ground_mask(layout,parent,scenery,era,baseline_rgb)
        overlay,ground_vectors,ground_stats=ground(era,gm)
        assert not ImageChops.darker(binary(overlay.getchannel('A')),roof_mask).getbbox()
        canvas,ground_vectors_base,substrate,_=parent.surface(era);assert substrate==0
        street_vectors=[];v2.street_relief(canvas,street_vectors,layout,parent.PALETTES[era])
        unchanged_scene=canvas.copy()
        for layer in layers:
            unchanged_scene.alpha_composite(layer['local'],layer['position'])
            if layer['roof'] is not None:unchanged_scene.alpha_composite(layer['roof'],layer['roofPosition'])
        baseline_error=changed(unchanged_scene,baseline_rgb).getbbox()
        assert baseline_error is None,('Frozen v2 layer reconstruction is not exact',era,baseline_error)
        del unchanged_scene
        canvas.alpha_composite(overlay)
        vectors=[];facade_count=0;gap_proofs=[];plans[era]={}
        for layer in layers:
            row=layer['row'];key=row['id'];components=[]
            dx,dy,rw,rh,trim,fh=layer['projection']
            if key in BOUNDS:
                components=facade_components(key,facade_masks[key],era);plans[era][key]=components
                local=apply_facade(layer,components,facade_masks[key])
                # Record the actual previous scene below each cleared original
                # facade, and later assert unchanged visible underlying pixels.
                for c in components:
                    if c['role']=='negative_gap':
                        x,y,w,h=c['rect'];gap_proofs.append(dict(id=c['id'],rect=c['rect'],
                            underlyingRgbSha256=sha(canvas.convert('RGB').crop((x,y,x+w,y+h)).tobytes())))
            else:local=layer['local']
            canvas.alpha_composite(local,(dx,dy+rh-fh))
            vectors.append(source_svg(layer,components))
            if layer['roof'] is not None: canvas.alpha_composite(layer['roof'],layer['roofPosition'])
            vectors+=layer['roofVectors']
        final=canvas.convert('RGB')
        for p in gap_proofs:
            x,y,w,h=p['rect'];assert sha(final.crop((x,y,x+w,y+h)).tobytes())==p['underlyingRgbSha256'],('Gap not true underlying scene',p['id'])
        with Image.open(io.BytesIO(frozen[name+'-scene.png'])) as baseline:
            delta=changed(final,baseline)
            for x,y,w,h in labels(layout):assert final.crop((x,y,x+w,y+h)).tobytes()==baseline.crop((x,y,x+w,y+h)).tobytes()
        facade_union=Image.new('L',SIZE)
        for m in facade_masks.values():facade_union=ImageChops.lighter(facade_union,m)
        assert not ImageChops.darker(facade_union,gm).getbbox(),'Facade/ground permissions overlap'
        outside=ImageChops.subtract(delta,ImageChops.lighter(facade_union,gm))
        assert not outside.getbbox(),('Unowned pixel change',era,outside.getbbox())
        assert not ImageChops.darker(delta,roof_mask).getbbox(),('Frozen roof changed',era)
        facade_count=ImageChops.darker(delta,facade_union).histogram()[255]
        assert facade_count>=1000 and len(gap_proofs)>=3,('Insufficient visible facade change',era,facade_count,len(gap_proofs))
        mask_pins[era]={k:dict(bytes=len(m.tobytes()),sha256=sha(m.tobytes()),pixels=m.histogram()[255]) for k,m in facade_masks.items()}
        ground_mask_pins[era]=dict(sha256=sha(gm.tobytes()),pixels=gm.histogram()[255])
        actual[era]=dict(changedPixels=delta.histogram()[255],facadeChangedPixels=facade_count,
                         groundChangedPixels=ground_stats['changedPixels'],trueGaps=gap_proofs)
        raw=png(final);assert len(raw)<=2*1024**2;outputs[name+'-scene.png']=raw
        prefix=frozen[name+'-scene.svg'].decode().split('<g id="editable_ground">')[0]
        outputs[name+'-scene.svg']=(prefix+'<g id="editable_ground">'+''.join(ground_vectors_base+street_vectors)+
          '</g><g id="editable_qualified_ground_overlay">'+ground_vectors+'</g><g id="editable_noninteractive_scenery" image-rendering="pixelated">'+''.join(vectors)+'</g></svg>\n').encode()
        assets.append(dict(themeId=era,**pin('world/'+VERSION+'/'+name+'-scene.png',raw),width=768,height=1296,mime='image/png'))
    result=copy.deepcopy(layout);result.update(version=VERSION,recipeRevision='central-facades-and-qualified-ground-01',assets=assets)
    result['facadeDerivation']=dict(sourceCommit=SOURCE,sourcePublication=PUBLICATION,allowedBounds=BOUNDS,
                                    componentsByTheme=plans,groundBinding=ground_binding)
    outputs['layout.json']=encode(result)
    provenance=dict(format='MCR_FACADE_COMPOSED_SOURCE_V3',version=VERSION,sourceCommit=SOURCE,sourcePublication=PUBLICATION,
        baselinePins=[pin(str((BASE/n).relative_to(ROOT)),raw) for n,raw in frozen.items()],
        sourceInputs=proof['sourceInputs'],originalCropPins=proof['originalCropPins'],additionCropPins=proof['additionCropPins'],
        helperPin=pin(str((OLD/'build.py').relative_to(ROOT)),(OLD/'build.py').read_bytes()),
        groundBinding=ground_binding,allowedBounds=BOUNDS,componentsByTheme=plans,facadeMaskPins=mask_pins,groundMaskPins=ground_mask_pins,
        actualByTheme=actual,incumbentDecorationsExact=285,protectedLabelRectanglesExact=18,
        frozenV2RecompositionExactEras=3,
        policy='NEW_AUTHORED_FACADE_ONLY_IN_ROOT_ALPHA_SUPPORT_MASK_AND_QUALIFIED_DISJOINT_GROUND',
        originalSourcePolicy='ALL_ORIGINAL_BYTES_UNCHANGED; THREE_PROJECTED_FACADES_ONLY_HAVE_EXPLICIT_NEW_DERIVATIVE_EXCEPTION',
        recipe=pin(str((HERE/'build.py').relative_to(ROOT)),(HERE/'build.py').read_bytes()),
        outputs=[pin(str((HERE/n).relative_to(ROOT)),raw) for n,raw in outputs.items()],
        license=dict(newVectors='Original project-authored masonry, recessed bays and true facade gaps; no external assets or fonts',
                     inherited=proof['license'],ground=ground_binding),
        notRun=['consumer/App adoption','SVG-engine parity','authoritative World/operator release','device/live/physical','whole reference acceptance'])
    outputs['source-provenance.json']=encode(provenance)
    assert sum(map(len,outputs.values()))<10*1024**2
    return outputs,actual

def read_ground(binding,layout,parent):
    prefix='frontend/public/world/map-lawns-derived-v3/'
    assert binding['path']==prefix+'interface.json'
    interface=json.loads(read_pin(binding))
    assert interface['format']=='MCR_GROUND_OVERLAY_V1' and interface['sourceCommit']==SOURCE
    assert interface['dimensions']==list(SIZE) and set(interface['eras'])==set(ERAS)
    expected={str((BASE/n).relative_to(ROOT)):h for n,h in FROZEN.items()}
    assert len(interface['baselinePins'])==10 and {p['path']:hexsha(p['sha256']) for p in interface['baselinePins']}==expected
    for p in interface['baselinePins']:read_pin(p)
    helper=interface['helperPin']
    assert helper['path']==str((OLD/'build.py').relative_to(ROOT)) and hexsha(helper['sha256'])==HELPER_SHA
    assert hexsha(sha((ROOT/helper['path']).read_bytes()))==HELPER_SHA
    provenance=json.loads(read_pin(interface['provenancePin']))
    assert provenance['sourceCommit']==SOURCE
    def validate_closure(value):
        if isinstance(value,dict):
            if {'path','bytes','sha256'}<=set(value):read_pin(value)
            else:
                for child in value.values():validate_closure(child)
        elif isinstance(value,list):
            for child in value:validate_closure(child)
    validate_closure(provenance)
    qualification=interface['qualification'];assert qualification['status']=='PASS'
    receipt=[json.loads(line) for line in read_pin(qualification['receipt']).decode().splitlines() if line.strip()]
    assert receipt[-1]['event']=='FULL' and receipt[-1]['status']=='PASS'
    accepted=next(r for r in receipt if r['event']=='ACCEPT')
    assert accepted['head']==qualification['sourceCommit']
    assert receipt[-1]['tempAbsent'] and receipt[-1]['allChildrenExited']
    def render(era,mask):
        row=interface['eras'][era]
        assert hexsha(row['allowedMaskSha256'])==hexsha(sha(mask.tobytes())) and row['allowedPixels']==mask.histogram()[255]
        for k in ('overlay','svg','operationsPin','allowedMaskPin'):
            assert row[k]['path'].startswith(prefix)
        raw=read_pin(row['overlay'])
        with Image.open(io.BytesIO(raw)) as source:
            assert source.mode=='RGBA' and source.size==SIZE;overlay=source.copy()
        operations=row['operations'];assert operations==json.loads(read_pin(row['operationsPin']))
        mask_raw=read_pin(row['allowedMaskPin'])
        with Image.open(io.BytesIO(mask_raw)) as source:
            assert source.mode=='L' and source.size==SIZE and source.tobytes()==mask.tobytes()
        replay=Image.new('RGBA',SIZE);draw=ImageDraw.Draw(replay);vectors=[]
        with Image.open(BASE/(era.lower()+'-scene.png')) as source:baseline=source.convert('RGB')
        pixels=baseline.load();ids=set()
        for op in operations:
            assert set(op)=={'id','kind','x','y','width','height','fill'} and op['kind']=='rect'
            assert isinstance(op['id'],str) and op['id'] not in ids;ids.add(op['id'])
            r=[op[k] for k in ('x','y','width','height')]
            assert all(type(v) is int for v in r) and fits(mask,r),('Ground footprint outside permission',op['id'])
            fill=op['fill'];assert isinstance(fill,str) and len(fill)==7 and fill.startswith('#')
            color=tuple(bytes.fromhex(fill[1:]));x,y,w,h=r
            assert all(pixels[xx,yy]!=color for yy in range(y,y+h) for xx in range(x,x+w)),('False ground change',op['id'])
            draw.rectangle((x,y,x+w-1,y+h-1),fill=color+(255,))
            vectors.append(f'<rect id="{op["id"]}" x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"/>')
        assert replay.tobytes()==overlay.tobytes(),'Owner overlay does not equal its closed operations'
        assert not ImageChops.subtract(binary(overlay.getchannel('A')),mask).getbbox()
        assert binary(overlay.getchannel('A')).histogram()[255]==row['changedPixels']>0
        # Parse the editable owner SVG. Only ordered rect primitives are valid;
        # the final source uses those verified primitives, never client assembly.
        svg=ET.fromstring(read_pin(row['svg']))
        assert svg.tag.removeprefix('{http://www.w3.org/2000/svg}')=='svg'
        assert set(svg.attrib)<= {'width','height','viewBox','shape-rendering'}
        assert svg.attrib.get('width')=='768' and svg.attrib.get('height')=='1296' and svg.attrib.get('viewBox')=='0 0 768 1296'
        assert svg.attrib.get('shape-rendering','crispEdges')=='crispEdges'
        parsed=[]
        for element in list(svg):
            kind=element.tag.removeprefix('{http://www.w3.org/2000/svg}')
            if kind in ('title','desc'):assert not list(element);continue
            assert kind=='rect' and set(element.attrib)=={'id','x','y','width','height','fill'}
            a=element.attrib
            parsed.append(dict(id=a['id'],kind='rect',x=int(a['x']),y=int(a['y']),width=int(a['width']),height=int(a['height']),fill=a['fill']))
        assert parsed==operations,'Ground SVG primitives differ from actual overlay operations'
        return overlay,''.join(vectors),row
    return render

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--verify',action='store_true');args=parser.parse_args()
    binding=json.loads((HERE/'ground-input.json').read_text())
    outputs,actual=produce(binding)
    expected=set(outputs)|{'build.py','README.md','ground-input.json'}
    assert not ({p.name for p in HERE.iterdir()}-expected)
    if args.verify:
        assert {p.name for p in HERE.iterdir()}==expected
        for n,raw in outputs.items():assert (HERE/n).read_bytes()==raw,n
    else:
        for n,raw in outputs.items():(HERE/n).write_bytes(raw)
    print(json.dumps(dict(status='PASS',mode='verify' if args.verify else 'build',outputs=len(outputs),
                         bytes=sum(map(len,outputs.values())),actualByTheme=actual)))
if __name__=='__main__':main()

