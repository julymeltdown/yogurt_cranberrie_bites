#!/usr/bin/env python3
"""Package independently editable atlas-backed image layers in native projects."""
import copy
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

HERE=Path(__file__).resolve().parent
parser=argparse.ArgumentParser();parser.add_argument('era',choices=['revenge','danger','parade']);era=parser.parse_args().era
OUT=HERE/'eras-v5'/era
CLI=os.environ.get('MCR_TSRCT') or shutil.which('tsrct') or str(Path.home()/'Library/Application Support/Tesseract/bin/tsrct')
layout=json.loads((OUT/'layout.json').read_text());proposal=json.loads((HERE/'geometry-proposal.json').read_text())
evidence=[]
def run(args):
    result=subprocess.run([CLI]+args,capture_output=True,text=True)
    evidence.append({'command':['tsrct']+[a.replace(str(HERE)+'/', '') for a in args],'exit_code':result.returncode})
    if result.returncode:raise RuntimeError(result.stdout+result.stderr)

for view in ['world','map']:
    project=OUT/f'{era}-{view}.tsrct';editable=OUT/f'{era}-{view}-editable.json'
    cache_file=OUT/f'{era}-{view}-asset-ids.json'
    cached=json.loads(cache_file.read_text()) if cache_file.exists() and project.exists() else []
    if not project.exists():run(['project','create','--project',str(project)])
    ids={}
    for name,asset in [('scenery',layout['native_atlas']),('ground',layout['ground'])]:
        asset_id=name+'-'+hashlib.sha256((OUT/asset).read_bytes()).hexdigest()[:16];ids[name]=asset_id
        if asset_id not in cached:
            run(['project','import-asset','--project',str(project),'--file',str(OUT/asset),'--asset-id',asset_id,'--kind','image'])
            cached.append(asset_id);cache_file.write_text(json.dumps(cached,indent=2)+'\n')
    run(['project','checkout','--project',str(project),'--output',str(editable)])
    doc=json.loads(editable.read_text());doc['dimensions']={'width':768,'height':1296};doc['duration']=3
    layers=[]
    def image(name,asset,rect,x,y,width,height):
        sx,sy=width/rect['width'],height/rect['height']
        source_width,source_height=(768,1296) if asset=='ground' else tuple(layout['native_atlas_dimensions'])
        # CLI0.4 accepts sourceRect but its renderer leaves neighboring atlas
        # pixels visible. Explicit editable masks and full-image placement give
        # the same non-destructive crop without relying on that render behavior.
        center=[x+(source_width/2-rect['x'])*sx,y+(source_height/2-rect['y'])*sy]
        mask=[{'type':'moveTo','x':x,'y':y},{'type':'lineTo','x':x+width,'y':y},{'type':'lineTo','x':x+width,'y':y+height},{'type':'lineTo','x':x,'y':y+height},{'type':'close'}]
        layers.append({'id':len(layers)+1,'name':name,'type':'Image','blendMode':'normal','activeRange':{'start':0,'duration':3000},
                       'transform':{'anchorPoint':[0,0],'position':[0,0],'scale':[100,100],'rotation':0,'opacity':100},
                       'masks':[{'id':10000+len(layers),'mode':'add','opacity':100,'path':{'commands':mask}}],
                       'source':{'assetId':ids[asset],'fit':{'custom':{'scale':[sx,sy],'contentCenter':center}}}})
    image('Original material ground','ground',{'x':0,'y':0,'width':768,'height':1296},0,0,768,1296)
    props=copy.deepcopy(layout['placements'])
    if view=='map':
        commands=[{'type':'moveTo' if i==0 else 'lineTo','x':x,'y':y} for i,(x,y) in enumerate(proposal['plaza_parcel'])]+[{'type':'close'}]
        layers.append({'id':len(layers)+1,'name':'Semantic green Plaza zone - not terrain authority','type':'Shape','blendMode':'normal','activeRange':{'start':0,'duration':3000},'transform':{'anchorPoint':[0,0],'position':[0,0],'scale':[100,100],'rotation':0,'opacity':100},'shape':{'path':{'commands':commands},'fills':[{'paint':{'type':'solid','color':[.48,.53,.26,1]},'fillRule':'nonZeroWinding','blendMode':'normal','opacity':100}]}})
        omit={o.get('replaces_prop_id') for o in layout['overview_representations']}
        props=[o for o in props if o['id'] not in omit]+layout['overview_representations']
    for o in sorted(props,key=lambda o:o['depth_y']):
        f=layout['frames'][o['frame']];rect={k:f[k] for k in ['x','y','width','height']}
        image(o['id'],'scenery',rect,o['x'],o['y'],o['width'],o['height'])
    doc['composition']={'id':'main','name':f'{era.upper()} {view.upper()} editable scenery, no characters or baked map labels','layers':list(reversed(layers))}
    editable.write_text(json.dumps(doc,indent=2)+'\n')
    run(['project','commit','--project',str(project),'--file',str(editable)])
    output=OUT/f'{era}-{view}-native.png';output.unlink(missing_ok=True)
    run(['preview','--project',str(project),'--time','1','--output',str(output)])
(OUT/'native-evidence.json').write_text(json.dumps({'commands':evidence,'map_limit':'Native overview uses approved solid green fill; SVG/runtime also outline it. Identical physical geometry. No app/server authority.','assets':'Exactly two packaged sources per project (one transparently padded era derivative atlas and shared ground); named independently editable image layers with explicit rectangular crop masks. Source pixels unchanged.','renderer_workaround':'InstalledCLI0.4 accepted sourceRect but neighboring atlas pixels remained visible; explicit mask/full-source transform renders the intended crop. Asset IDs are content hashes to prevent stale imports.'},indent=2)+'\n')
print(json.dumps({'projects':2,'status':'PASS','dimensions':[768,1296]}))
