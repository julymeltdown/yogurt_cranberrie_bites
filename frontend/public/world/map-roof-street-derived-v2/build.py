"""New source composition only; immutable density inputs are never regenerated."""
from __future__ import annotations
import argparse
import copy
import hashlib
import importlib.util
import io
import json
import math
from pathlib import Path
from PIL import Image, ImageChops, ImageDraw

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
BASE = ROOT/'frontend/public/world/map-density-derived-v1'
ART06 = ROOT/'frontend/public/world/map-reference-composition-v1'
VERSION = 'map-roof-street-derived-v2'
SOURCE = '7acfac3aed91f1eb06fa48eee63ce3da1431fadc'
CANONICAL_BASE = '7ba6de456949136c33a068bdeee8bebb8e7822fd'
ERAS = ('REVENGE', 'PARADE', 'DANGER')
SIZE = (768, 1296)
FROZEN = {
    'README.md': '96098bb840fb4e047d5149571b2a6e64723c516c5784271042a41d1ff464c7ff',
    'build.py': '4f4f6eede3413d7fb0502103f543a563fb88e69e26600fd177c71648d2ff7539',
    'layout.json': '971c9a94cd1aa01c771f3defa8cede70754c87f43eb4f7fb0ed4cf98921096eb',
    'source-provenance.json': '66b18e7c0f235f46828b87811ba344da3a4dc1c6025ae5622493026444b9d617',
    'revenge-scene.png': '7d657d5cd22ceafa4edf4bff7df5b75843927208ae8a8704747c4a645b51e071',
    'parade-scene.png': 'a8ac1c806adfc3cd36af21c655131b3c19e80034528632c23001eb49adcae9e1',
    'danger-scene.png': 'b54be823fe26b659b7223f4d2baacbdc2a232d8b1b78926540b87e790a05ec1b',
    'revenge-scene.svg': '40b1bbe84a3857861f4079563ef5cc10b80f2e564f1902d008c1bcfef8203aa2',
    'parade-scene.svg': 'b66ed2c2c4ad70a109e3a5f2b4d9bb26c5ed849f05e1b800a1b9ba1e30ef4b7d',
    'danger-scene.svg': '197249431cccf8b50741f494da88f1283140cc28ee4a70141869ce5a35da57b1',
}
HELPER_SHA = '6bf696b7c3edd5adf35e0b5995ca9cd33f915e87898de0e9266f2923cffc0433'

def sha(raw):
    return 'sha256:'+hashlib.sha256(raw).hexdigest()

def encode(value):
    return (json.dumps(value, indent=2, ensure_ascii=True)+'\n').encode()

def descriptor(path, raw):
    return dict(path=path, bytes=len(raw), sha256=sha(raw))

def intersects(a, b):
    return a[0] < b[0]+b[2] and b[0] < a[0]+a[2] and a[1] < b[1]+b[3] and b[1] < a[1]+a[3]

def reserves(layout):
    return [[a['exclusionRect'][k] for k in ('x', 'y', 'width', 'height')] for a in layout['serviceLabelAnchors']]

def changed(a, b):
    channels = ImageChops.difference(a.convert('RGB'), b.convert('RGB')).split()
    return ImageChops.lighter(ImageChops.lighter(channels[0], channels[1]), channels[2]).point(lambda v: 255 if v else 0)

def roof_plan(name, w, h):
    """Unequal connected volumes, authored inside the unchanged illustration slot."""
    seed = sum(name.encode())
    if w >= 170:
        spans = [(0, .43, .16), (.38, .78, 0), (.73, 1, .30)]
    elif w >= 105:
        spans = [(0, .61, 0), (.55, 1, .24)] if seed % 2 else [(0, .46, .22), (.40, 1, 0)]
    else:
        spans = [(0, 1, .06)]
    result = []
    for index, (left, right, top) in enumerate(spans):
        x, y, right = round(w*left), round(h*top), round(w*right)
        result.append(dict(id=name+'_volume_'+str(index), rect=[x, y, right-x, h-y],
                           profile=('hip', 'front_gable', 'saltbox')[(seed+index) % 3]))
    return result

def draw_roof(canvas, vectors, row, rect, palette):
    x, y, w, h = rect
    draw = ImageDraw.Draw(canvas)
    # These are newly authored roof vectors, not recolored source facade pixels.
    body, light, dark = palette['roof'], palette['roofLight'], palette['roofDark']
    def poly(points, fill):
        draw.polygon(points, fill=fill)
        vectors.append('<polygon points="'+' '.join(f'{a},{b}' for a,b in points)+f'" fill="{fill}"/>')
    def line(points, fill, width=1):
        # A thick diagonal can extend beyond its endpoint. Author its centerline
        # inside the original roof slot; never clip source RGBA or widen the slot.
        pad=width//2
        points=[(min(x+w-1-pad,max(x+pad,px)),min(y+h-1-pad,max(y+pad,py))) for px,py in points]
        draw.line(points, fill=fill, width=width)
        vectors.append('<polyline points="'+' '.join(f'{a},{b}' for a,b in points)+f'" fill="none" stroke="{fill}" stroke-width="{width}"/>')
    def box(px, py, rw, rh, fill):
        draw.rectangle((px,py,px+rw-1,py+rh-1), fill=fill)
        vectors.append(f'<rect x="{px}" y="{py}" width="{rw}" height="{rh}" fill="{fill}"/>')
    plans = roof_plan(row['id'], w, h)
    vectors.append(f'<g id="{row["id"]}_new_roof" shape-rendering="crispEdges">')
    for module in plans:
        mx, my, mw, mh = module['rect']
        def p(u,v):
            return (x+mx+min(mw-1,max(0,round((mw-1)*u))), y+my+min(mh-1,max(0,round((mh-1)*v))))
        profile = module['profile']
        if profile == 'front_gable':
            outline = [p(0,1), p(.12,.30), p(.46,0), p(.88,.30), p(1,1)]
            poly(outline, body)
            poly([p(0,1),p(.12,.30),p(.46,0),p(.46,1)], dark)
            line([p(.46,0),p(.88,.30),p(1,1)], light, 2)
            line([p(.46,0),p(.46,1)], light, 2)
        else:
            ridge = (.18,.69,.18) if profile == 'hip' else (.08,.91,.31)
            a,b,c,d=p(.08,.08),p(.92,.08),p(1,1),p(0,1)
            r,s=p(ridge[0],ridge[2]),p(ridge[1],ridge[2])
            for points,fill in [([a,b,s,r],light),([r,s,c,d],body),([a,r,d],dark),([b,c,s],dark)]:
                poly(points,fill)
            line([r,s],light,2)
            line([r,d],dark,2)
            line([s,c],dark,2)
        # Short tile groups live in the front plane's central interior. No alpha mask.
        for ty in range(max(8,mh//2), mh-5, 7):
            for tx in range(8+(ty//7 % 2)*3, mw-9, 10):
                if (tx+ty+len(row['id'])) % 4:
                    box(x+mx+tx,y+my+ty,5,2,light)
        line([p(0,1),p(1,1)],dark,2)
        # One articulated dormer per volume reads at the narrowest source scale.
        cx,cy=p(.55,.58)
        dw,dh=max(6,min(12,mw//5)),max(5,min(10,mh//5))
        if cx+dw < x+mx+mw and cy+dh < y+my+mh:
            box(cx,cy,dw,dh,dark)
            box(cx+2,cy+2,dw-4,dh-3,'#f4d89f')
            line([(cx-2,cy),(cx+dw//2,cy-4),(cx+dw+1,cy)],light,2)
    vectors.append('</g>')
    return plans

def street_relief(canvas, vectors, base, palette):
    """Larger stone faces entirely within incumbent paving; no new route paint."""
    original = canvas.copy()
    pixels = original.load()
    # Include the exact pre-existing small-stone tones, not lawn/edge colors.
    # A larger face may replace incumbent paving detail without crossing paint.
    paving = {}
    for key in ('path','ring'):
        base_color=tuple(bytes.fromhex(palette[key][1:]))
        edge=bytes.fromhex(palette['pathEdge'][1:])
        family={base_color,*(tuple(round(c*(1-f)+d*f) for c,d in zip(base_color,edge)) for f in (.10,.16,.22))}
        for color in family:
            paving[color]=(base_color,family)
    draw = ImageDraw.Draw(canvas)
    boxes=[]
    # The first protected-area run accepted276 faces at18x13 cadence. Sample
    # the same incumbent paint more closely; footprints/exclusions stay exact.
    for y in range(5,SIZE[1]-12,10):
        for x in range(5+(y//10 % 2)*7,SIZE[0]-18,15):
            rect=[x,y,12,7]
            if any(intersects(rect,r) for r in reserves(base)):
                continue
            if any(math.hypot(x+6-p['x'],y+3-p['y']) < 45 for p in base['canonicalDisplayAnchors']):
                continue
            sampled=pixels[x,y][:3]
            if sampled not in paving:
                continue
            color,family=paving[sampled]
            if not all(pixels[x+dx,y+dy][:3] in family for dx in range(12) for dy in range(7)):
                continue
            # Quiet stone face plus one offset joint, preserving pastel ground.
            fill=tuple(round(c*.88+d*.12) for c,d in zip(color,bytes.fromhex(palette['pathEdge'][1:])))
            tone='#'+''.join(f'{v:02x}' for v in fill)
            draw.rectangle((x,y,x+11,y+5),fill=fill)
            draw.line([(x+2,y+6),(x+10,y+6)],fill=palette['pathEdge'])
            vectors.append(f'<rect x="{x}" y="{y}" width="12" height="6" fill="{tone}"/><path d="M{x+2} {y+6}h8" stroke="{palette["pathEdge"]}"/>')
            boxes.append(rect)
    assert len(boxes) >= 350, ('Insufficient visible paving relief',len(boxes))
    return boxes, changed(canvas,original)

def produce():
    frozen,pins={},[]
    for name,expected in FROZEN.items():
        raw=(BASE/name).read_bytes()
        assert sha(raw)=='sha256:'+expected,name
        frozen[name]=raw; pins.append(descriptor(str((BASE/name).relative_to(ROOT)),raw))
    base=json.loads(frozen['layout.json']); old=json.loads(frozen['source-provenance.json'])
    assert len(base['decorations'])==285
    for pin in old['sourceInputs']:
        assert descriptor(pin['path'],(ROOT/pin['path']).read_bytes())==pin
    helper=(ART06/'build.py').read_bytes()
    assert sha(helper)=='sha256:'+HELPER_SHA
    spec=importlib.util.spec_from_file_location('immutable_ground_and_projection',ART06/'build.py')
    parent=importlib.util.module_from_spec(spec); spec.loader.exec_module(parent)
    originals={(p['themeId'],p['frame']):p for p in old['originalCropPins']}
    outputs,assets,stats,plans_by_era={},[],{},{}
    for era in ERAS:
        name=era.lower(); canvas,ground_vectors,substrate,_=parent.surface(era)
        assert substrate==0
        street_vectors=[]
        streets,street_mask=street_relief(canvas,street_vectors,base,parent.PALETTES[era])
        vectors=[]; roof_domains=[]; plans=[]
        atlas_path=ROOT/f'frontend/public/world/reference-scene-v2/map-cozy-v1/{name}/atlas.png'
        with Image.open(atlas_path) as atlas:
            cache={}
            for row in base['decorations']:
                frame=row['frame']; proof=originals[(era,frame)]; f=proof['sourceRect']
                sx,sy,sw,sh=[f[k] for k in ('x','y','width','height')]
                if frame not in cache:
                    crop=atlas.crop((sx,sy,sx+sw,sy+sh))
                    assert sha(crop.tobytes())==proof['rgbaSha256']; cache[frame]=crop
                x,y,w,h=row['rect']; end=math.floor(sh*parent.CANOPY_SOURCE_END) if row['crop_role']=='canopy' else sh
                scale=min(w/sw,h/end); rw,rh=max(1,math.floor(sw*scale+.5)),max(1,math.floor(end*scale+.5))
                dx,dy=x+(w-rw)//2,y+h-rh
                venue=frame in parent.VENUES
                trim=math.floor(sh*parent.FACADE_SOURCE_START) if venue else 0
                fh=max(1,math.floor(rh*(1-parent.FACADE_SOURCE_START)*parent.FACADE_HEIGHT_SCALE+.5)) if venue else rh
                top=rh-fh
                canvas.alpha_composite(cache[frame].crop((0,trim,sw,end)).resize((rw,fh),Image.Resampling.NEAREST),(dx,dy+top))
                vectors.append(f'<svg id="{row["id"]}" x="{dx}" y="{dy+top}" width="{rw}" height="{fh}" viewBox="{sx} {sy+trim} {sw} {end-trim}" preserveAspectRatio="none"><use xlink:href="#atlas"/></svg>')
                if venue:
                    # Retain the complete incumbent structure, including its
                    # existing 2px hip/eave strokes outside the nominal slot.
                    # New architectural planes are authored above that structure;
                    # no crop, pixel restoration, mask or enlarged audit domain.
                    parent.overhead_roof(canvas,vectors,row['id'],dx,dy,rw,rh,parent.PALETTES[era])
                    # Whole protected-adjacent roofs remain unchanged, never clipped/masked.
                    domain=[dx,dy,rw,max(top,math.ceil(rh*.72))]
                    if not any(intersects(domain,r) for r in reserves(base)):
                        volumes=draw_roof(canvas,vectors,row,[dx,dy,rw,top],parent.PALETTES[era])
                        roof_domains.append(domain)
                        plans.append(dict(id=row['id'],renderedRect=[dx,dy,rw,rh],roofDomain=domain,volumes=volumes))
        final=canvas.convert('RGB')
        with Image.open(io.BytesIO(frozen[name+'-scene.png'])) as before:
            delta=changed(final,before)
            for x,y,w,h in reserves(base):
                assert final.crop((x,y,x+w,y+h)).tobytes()==before.crop((x,y,x+w,y+h)).tobytes(), ('Label',era,x,y)
        # These masks only measure permitted differences; they never edit artwork.
        allowed=street_mask.copy(); draw=ImageDraw.Draw(allowed)
        roof_mask=Image.new('L',SIZE); roof_draw=ImageDraw.Draw(roof_mask)
        for x,y,w,h in roof_domains:
            draw.rectangle((max(0,x),max(0,y),min(SIZE[0]-1,x+w-1),min(SIZE[1]-1,y+h-1)),fill=255)
            roof_draw.rectangle((x,y,x+w-1,y+h-1),fill=255)
        outside=ImageChops.subtract(delta,allowed)
        bounds=outside.getbbox()
        if bounds is not None:
            pixels=outside.load()
            first=next((px,py) for py in range(bounds[1],bounds[3]) for px in range(bounds[0],bounds[2]) if pixels[px,py])
            def distance(plan):
                px,py,pw,ph=plan['roofDomain']
                return max(px-first[0],0,first[0]-(px+pw-1))**2+max(py-first[1],0,first[1]-(py+ph-1))**2
            with Image.open(io.BytesIO(frozen[name+'-scene.png'])) as original:
                before_pixel=original.getpixel(first)
            raise AssertionError(('Unowned pixel change',era,dict(bounds=bounds,first=first,before=before_pixel,
                after=final.getpixel(first),nearestRoofs=sorted(plans,key=distance)[:2],pavingFaces=len(streets))))
        count=delta.histogram()[255]
        roof_count=ImageChops.darker(delta,roof_mask).histogram()[255]
        street_count=ImageChops.darker(delta,street_mask).histogram()[255]
        assert len(plans)>=12 and count>15000 and roof_count>5000 and street_count>5000, (era,len(plans),count,roof_count,street_count)
        buffer=io.BytesIO(); final.save(buffer,format='PNG',compress_level=9)
        png=buffer.getvalue(); assert len(png)<2097152
        outputs[name+'-scene.png']=png
        raw=frozen[name+'-scene.svg'].decode()
        start=raw.index('<g id="editable_ground">')
        prefix=raw[:start]
        outputs[name+'-scene.svg']=(prefix+'<g id="editable_ground">'+''.join(ground_vectors+street_vectors)+'</g><g id="editable_noninteractive_scenery" image-rendering="pixelated">'+''.join(vectors)+'</g></svg>\n').encode()
        assets.append(dict(themeId=era,**descriptor('world/'+VERSION+'/'+name+'-scene.png',png),width=768,height=1296,mime='image/png'))
        plans_by_era[era]=plans
        stats[era]=dict(changedPixels=count,roofChangedPixels=roof_count,streetChangedPixels=street_count,
                        roofs=len(plans),volumes=sum(len(p['volumes']) for p in plans),pavingFaces=len(streets),pavingRects=streets)
    layout=copy.deepcopy(base)
    layout.update(version=VERSION,recipeRevision='connected-roofs-and-readable-paving-01',assets=assets)
    layout['roofStreetDerivation']=dict(sourceCommit=SOURCE,canonicalBase=CANONICAL_BASE,baselinePins=pins,
        roofPlansByTheme=plans_by_era,policy='UNCHANGED_285_PLACEMENTS_ORIGINAL_RGBA_NO_WORLD_AUTHORITY')
    outputs['layout.json']=encode(layout)
    proof=dict(format='MCR_MAP_ROOF_STREET_DERIVED_V2',version=VERSION,sourceCommit=SOURCE,canonicalBase=CANONICAL_BASE,
        baselinePins=pins,sourceInputs=old['sourceInputs'],originalCropPins=old['originalCropPins'],
        additionCropPins=old['additionCropPins'],roofPlansByTheme=plans_by_era,actualByTheme=stats,
        recipe=descriptor(str((HERE/'build.py').relative_to(ROOT)),(HERE/'build.py').read_bytes()),
        outputs=[descriptor(str((HERE/n).relative_to(ROOT)),raw) for n,raw in outputs.items()],
        protectedLabelRectanglesExact=18,incumbentDecorationsExact=285,sourceRgbaPolicy='EXACT_ORIGINAL_CROPS_UNCHANGED_PROJECTION',
        helperReuse=dict(path=str((ART06/'build.py').relative_to(ROOT)),sha256='sha256:'+HELPER_SHA,
                         called=['surface','overhead_roof for complete incumbent structural underlays; protected-adjacent roofs remain unchanged'],notCalled=['produce','main']),
        license=dict(newVectors='Original project-authored roof and paving vectors; no external asset/font dependency',
                     inheritedSources='Immutable cozy scenery generated-source/prompt and originalCropPins; no relicensing or human review claim',
                     sourceRecord='frontend/public/world/reference-scene-v2/map-cozy-v1/README.md',
                     inputPins=[descriptor('frontend/public/world/reference-scene-v2/map-cozy-v1/'+n,
                         (ROOT/'frontend/public/world/reference-scene-v2/map-cozy-v1'/n).read_bytes())
                         for n in ('README.md','prompt.txt','source-original.png')]),
        notRun=['consumer/App uptake','SVG engine parity','current operator release admission','device/live/physical','full reference acceptance'])
    outputs['source-provenance.json']=encode(proof)
    assert sum(map(len,outputs.values()))<10*1024**2
    return outputs,stats

def main():
    parser=argparse.ArgumentParser(); parser.add_argument('--verify',action='store_true'); args=parser.parse_args()
    outputs,stats=produce(); expected=set(outputs)|{'build.py','README.md'}
    assert not ({p.name for p in HERE.iterdir()}-expected)
    if args.verify:
        assert {p.name for p in HERE.iterdir()}==expected
        for name,raw in outputs.items(): assert (HERE/name).read_bytes()==raw,name
    else:
        for name,raw in outputs.items(): (HERE/name).write_bytes(raw)
    print(json.dumps(dict(status='PASS',mode='verify' if args.verify else 'build',outputs=len(outputs),bytes=sum(map(len,outputs.values())),
                         actualByTheme={era:{k:v for k,v in s.items() if k!='pavingRects'} for era,s in stats.items()})))

if __name__=='__main__':
    main()
