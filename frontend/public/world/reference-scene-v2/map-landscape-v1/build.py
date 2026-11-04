#!/usr/bin/env python3
"""Editable MAP-only planting illustrations using the frozen original v8 atlas."""
import copy,hashlib,json,shutil,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent;SOURCE=HERE.parent;ROOT=SOURCE.parents[3]
CLI=str(Path.home()/'Library/Application Support/Tesseract/bin/tsrct')
read=lambda p:json.loads(p.read_text());write=lambda p,v:p.write_text(json.dumps(v,indent=2)+'\n')
sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
pin=lambda p:{'path':str(p.relative_to(ROOT)),'bytes':p.stat().st_size,'sha256':sha(p)}
p=read(SOURCE/'spatial-v5/geometry-proposal.json');labels=read(SOURCE/'quality-v10/map-label-presentation.json')
assert p['geometry_hash']==labels['geometry_hash']=='sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395'
rects=[('plaza_north_west',265,388,54,32),('plaza_north_east',363,384,56,33),('plaza_east',443,434,32,20),('plaza_south_west',305,551,62,35),('plaza_south_east',384,548,36,23),('meeting_north',406,727,64,38),('meeting_east',500,884,52,31),('meeting_west',218,936,30,22),('giveaway_west',300,1202,62,37),('giveaway_south_west',240,1242,72,42)]
decorations=[{'id':'map_garden_'+name,'frame':'shrub_bed','x':x,'y':y,'width':w,'height':h,'depth_y':y+h,'interaction_id':None,'view':'MAP_ONLY','surface':'semantic_plaza' if name.startswith('plaza_') else 'canonical_lawn','kind':'unbound_map_planting_illustration_not_a_world_solid'} for name,x,y,w,h in rects]
variants=[];evidence=[]
def run(args):
 r=subprocess.run([CLI]+args,capture_output=True,text=True);evidence.append({'command':['tsrct']+[a.replace(str(SOURCE)+'/', '') for a in args],'exit_code':r.returncode})
 if r.returncode:raise RuntimeError(r.stdout+r.stderr)
for era in ['parade','revenge','danger']:
 base=SOURCE/f'quality-v10/{era}';layout=read(base/'layout.json');doc=read(base/f'{era}-map-editable.json');before=copy.deepcopy(doc['composition']['layers']);scene=read(base/'scenery.json')
 project=HERE/f'{era}-map.tsrct'
 if not project.exists():shutil.copyfile(base/f'{era}-map.tsrct',project)
 run(['project','checkout','--project',str(project),'--output',str(HERE/f'{era}-map-editable.json')])
 # Retain the frozen label overlay and scene layers exactly; add independent
 # atlas-backed illustrations in the original stable depth ordering.
 ground=next(l for l in before if l['type']=='Image' and l['name']=='Original material ground')
 zone=next(l for l in before if l['name']=='Semantic green Plaza zone - not terrain authority')
 overlay=[l for l in before if l['type']!='Image' and l is not zone]
 images=[l for l in before if l['type']=='Image' and l is not ground]
 replaced={o.get('replaces_prop_id') for o in layout['overview_representations']}
 props=[o for o in layout['placements'] if o['id'] not in replaced]+layout['overview_representations']
 depth={o['id']:o['depth_y'] for o in props};atlas_id=images[0]['source']['assetId'];frame=layout['frames']['shrub_bed']
 next_id=max(l['id'] for l in before)+1
 for index,o in enumerate(decorations):
  x,y,w,h=[o[k] for k in ['x','y','width','height']];sx,sy=w/frame['width'],h/frame['height'];layer_id=next_id+index
  mask=[{'type':'moveTo','x':x,'y':y},{'type':'lineTo','x':x+w,'y':y},{'type':'lineTo','x':x+w,'y':y+h},{'type':'lineTo','x':x,'y':y+h},{'type':'close'}]
  images.append({'id':layer_id,'name':o['id'],'type':'Image','blendMode':'normal','activeRange':{'start':0,'duration':3000},'transform':{'anchorPoint':[0,0],'position':[0,0],'scale':[100,100],'rotation':0,'opacity':100},'masks':[{'id':100000+layer_id,'mode':'add','opacity':100,'path':{'commands':mask}}],'source':{'assetId':atlas_id,'fit':{'custom':{'scale':[sx,sy],'contentCenter':[x+(1024-frame['x'])*sx,y+(1024-frame['y'])*sy]}}}})
  depth[o['id']]=o['depth_y']
 doc['composition']['name']=f'{era.upper()} MAP with original unbound planting illustrations'
 doc['composition']['layers']=overlay+sorted(images,key=lambda l:depth[l['name']],reverse=True)+[zone,ground]
 assert [l for l in doc['composition']['layers'] if not l['name'].startswith('map_garden_')]==before
 editable=HERE/f'{era}-map-editable.json';write(editable,doc);run(['project','commit','--project',str(project),'--file',str(editable)]);run(['preview','--project',str(project),'--time','1','--output',str(HERE/f'{era}-map-native.png')])
 provenance_file=SOURCE/f'quality-v8/{era}/provenance.json';transform=next(t for t in read(provenance_file)['transforms'] if t['frame']=='shrub_bed');original=(provenance_file.parent/transform['source']).resolve()
 variants.append({'source_provenance':pin(provenance_file),'original_source':pin(original),'atlas_transform':copy.deepcopy(transform),'theme_id':era.upper(),'geometry_hash':p['geometry_hash'],'base_scene':pin(base/'scenery.json'),'layout':pin(base/'layout.json'),'atlas':scene['atlases'][0],'native_atlas':pin(base/'native-atlas.png'),'frame':copy.deepcopy(frame),'runtime_pngs':2,'runtime_bytes':scene['ground']['bytes']+scene['atlases'][0]['bytes'],'runtime_pixels':2043904})
write(HERE/'presentation.json',{'version':1,'status':'CONTRACT_READY_MAP_SOURCE_NOT_RELEASE_BOUND','geometry_hash':p['geometry_hash'],'view':'MAP_ONLY','dimensions':{'width':768,'height':1296},'base_source_published_commit':'06562ff1962be525ed929ef431a54bb589860373','source_execution_base':'93ef0d895dcc4b247b727e2cccba0bb84576b262','frozen_inputs':[pin(SOURCE/'quality-v10/manifest.json'),pin(SOURCE/'quality-v10/map-label-presentation.json'),pin(SOURCE/'spatial-v5/geometry-proposal.json'),pin(SOURCE/'spatial-v5/integer-projection.json')],'decorations':decorations,'themes':variants,'composition':'Combine existing MAP props and these original atlas illustrations in stable ascending depth_y order; paint the frozen labels afterward, then actual runtime markers. WORLD ignores this sidecar. No additional PNG or font.','authority':'Unbound MAP illustrations only: never emit WORLD objects, collision cells, place/destination identities, interaction targets, physical eligibility or selected location from this sidecar. Existing canonical geometry and source review/release state remain unchanged.','provenance':'All decorations reuse the original independently authored shrub_bed frame from the exact frozen era atlas; no reference, selected-character or legacy raster sampling, generation, recoloring or alpha resealing.'})
write(HERE/'native-evidence.json',{'commands':evidence,'native_viewer':'Open requested near start; UI queued is not a render proof. Actual CLI PNG exports are checked separately.','source_integrity':'Existing base native layers and IDs stay exact when the ten added illustration layers are removed. Packaged atlas/ground/font bytes are unchanged.','renderer':'Installed Tesseract0.4.0; full2048 native atlas plus explicit rectangular crop masks, matching the existing padding workaround.'})
print(json.dumps({'status':'AUTHORED_MAP_SOURCE_ONLY','variants':3,'decorations':len(decorations),'new_runtime_textures':0,'geometry_hash':p['geometry_hash']}))
