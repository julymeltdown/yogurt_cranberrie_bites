"""Actual facade mechanics only. No lawn input, final pack or visual acceptance."""
import importlib.util
import json
from pathlib import Path
import sys
from PIL import Image, ImageChops

ROOT=Path(__file__).resolve().parents[5]
PACK=ROOT/'frontend/public/world/map-facade-derived-v3'
spec=importlib.util.spec_from_file_location('facade_foundation_builder',PACK/'build.py')
b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)
out=Path(sys.argv[1]).resolve()
assert out.is_dir() and out.name=='previews'  # Existing supervisor-owned TMP child.
frozen={n:(b.BASE/n).read_bytes() for n in b.FROZEN}
assert all(b.sha(raw)=='sha256:'+b.FROZEN[n] for n,raw in frozen.items())
layout=json.loads(frozen['layout.json']);proof=json.loads(frozen['source-provenance.json'])
for pin in proof['sourceInputs']:b.read_pin(pin)
parent=b.load_module(b.OLD/'build.py','foundation_ground',b.HELPER_SHA)
v2=b.load_module(b.BASE/'build.py','foundation_roofs',b.FROZEN['build.py'])
result=dict(format='FACADE_FOUNDATION_ONLY',sourceCommit=b.SOURCE,eras={},
            notRun=['qualified lawn composition','final pack','combined qualifier 14 cases',
                    'source image assessment','consumer/App','whole reference acceptance'])
for era in b.ERAS:
    layers,_=b.layers_for(era,layout,proof,parent,v2)
    masks,_,roofs=b.masks_for(layout,proof,era,layers)
    baseline=Image.open(b.BASE/(era.lower()+'-scene.png')).convert('RGB')
    surface,_,substrate,_=parent.surface(era);assert substrate==0
    v2.street_relief(surface,[],layout,parent.PALETTES[era])
    unchanged=surface.copy()
    for layer in layers:
        unchanged.alpha_composite(layer['local'],layer['position'])
        if layer['roof'] is not None:unchanged.alpha_composite(layer['roof'],layer['roofPosition'])
    assert not b.changed(unchanged,baseline).getbbox(),('Frozen reconstruction',era)
    del unchanged
    canvas=surface;plans={};gaps=[]
    for layer in layers:
        key=layer['row']['id'];local=layer['local']
        if key in b.BOUNDS:
            components=b.facade_components(key,masks[key],era);plans[key]=components
            local=b.apply_facade(layer,components,masks[key])
            for c in components:
                assert b.fits(masks[key],c['rect'])
                if c['role']=='negative_gap':
                    x,y,w,h=c['rect'];dx,dy=layer['position']
                    assert not local.crop((x-dx,y-dy,x-dx+w,y-dy+h)).getchannel('A').getbbox()
                    gaps.append(dict(id=c['id'],rect=c['rect'],
                        underlyingRgbSha256=b.sha(canvas.convert('RGB').crop((x,y,x+w,y+h)).tobytes())))
            # Whole-footprint rejection is required; never clip an oversized op.
            bad=[dict(id='outside_grant',role='negative_gap',rect=[0,0,2,2],
                      operations=[dict(kind='clear',rect=[0,0,2,2])])]
            try:b.apply_facade(layer,bad,masks[key])
            except AssertionError:pass
            else:raise AssertionError('Outside component was accepted')
        canvas.alpha_composite(local,layer['position'])
        if layer['roof'] is not None:canvas.alpha_composite(layer['roof'],layer['roofPosition'])
    final=canvas.convert('RGB');delta=b.changed(final,baseline);union=Image.new('L',b.SIZE)
    for mask in masks.values():union=ImageChops.lighter(union,mask)
    assert not ImageChops.subtract(delta,union).getbbox(),('Unowned delta',era)
    assert not ImageChops.darker(delta,roofs).getbbox(),('Roof delta',era)
    for x,y,w,h in b.labels(layout):
        assert final.crop((x,y,x+w,y+h)).tobytes()==baseline.crop((x,y,x+w,y+h)).tobytes()
    for gap in gaps:
        x,y,w,h=gap['rect']
        assert b.sha(final.crop((x,y,x+w,y+h)).tobytes())==gap['underlyingRgbSha256']
    changed=delta.histogram()[255];assert changed>=1000 and len(gaps)>=3
    raw=b.png(final);filename=era.lower()+'-foundation.png';(out/filename).write_bytes(raw)
    result['eras'][era]=dict(file=filename,bytes=len(raw),sha256=b.sha(raw),
        frozenRecompositionExact=True,changedPixels=changed,components=plans,trueGaps=gaps,
        masks={k:dict(bytes=len(m.tobytes()),sha256=b.sha(m.tobytes()),pixels=m.histogram()[255])
               for k,m in masks.items()})
assert all(result['eras'][e]['components']==result['eras']['PARADE']['components'] for e in b.ERAS)
(out/'foundation.json').write_bytes(b.encode(result))
assert sum(p.stat().st_size for p in out.iterdir())<3*1024**2
print(json.dumps(result))
