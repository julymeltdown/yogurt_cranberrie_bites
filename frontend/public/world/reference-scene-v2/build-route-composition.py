#!/usr/bin/env python3
"""v9 ground and MAP-only north facades, retaining exact v8 world geometry."""
import copy,hashlib,json,shutil,argparse
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('--revision',type=int,choices=[9,10],default=9);args=parser.parse_args();rev=args.revision;geo=4 if rev==9 else 5
HERE=Path(__file__).resolve().parent;OUT=HERE/f'quality-v{rev}';BASE=HERE/'quality-v8'
read=lambda p:json.loads(p.read_text());write=lambda p,v:p.write_text(json.dumps(v,indent=2)+'\n')
sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
pin=lambda p:{'path':str(p.relative_to(OUT)),'bytes':p.stat().st_size,'sha256':sha(p)}
p=read(HERE/f'spatial-v{geo}/geometry-proposal.json')
label_path=HERE/'map-labels-v1/presentation.json'
if rev==10:
 label=read(label_path);label.pop('reference_scene_commit');label.update(version=2,geometry_hash=p['geometry_hash'],source_lineage_commit='ab8e93ba79332df62d5105bb4aa187c8c7f5cd48',publication='Unpublished candidate; this lineage commit does not contain the new geometry or presentation.')
 root=HERE.parents[3]
 def source_pin(file):return {'path':str(file.relative_to(root)),'bytes':file.stat().st_size,'sha256':sha(file)}
 label['base_presentation']=source_pin(label_path)
 label['frozen_inputs']=[source_pin(BASE/'manifest.json'),source_pin(HERE/'spatial-v5/integer-projection.json'),source_pin(HERE/'spatial-v5/geometry-proposal.json')]
 label['derivation']='Explicit spatial-v5 binding of the byte-equivalent v1 drawing values: dimensions, labels, anchors, null/existing bindings, style, icons and font. New geometry requires fresh consumer validation; no prior review, release or server allocation is inherited.'
 label_path=OUT/'map-label-presentation.json';write(label_path,label)
label_relative='../map-label-presentation.json' if rev==10 else '../../map-labels-v1/presentation.json'
ground={'id':f'quality_v{rev}_route_edge_ground','path':f'world/reference-scene-v2/quality-v{rev}/ground.png','sha256':sha(OUT/'ground.png'),'width_px':768,'height_px':1296,'bytes':(OUT/'ground.png').stat().st_size}
reps=copy.deepcopy(read(BASE/'parade/layout.json')['overview_representations'])
for ident,replace,x in [('north_west_facade_representation','northern_hall',-12),('north_east_facade_representation','north_eastern_hall',542)]:
 reps.append({'id':ident,'replaces_prop_id':replace,'frame':'north_hall','x':x,'y':6,'width':210,'height':144,'depth_y':150,'interaction_id':None,
              'kind':'cropped_unbound_map_architecture_not_a_world_solid','reason':'Show the small north venue facade in front of the canopy band in overview scale; replace existing unbound hall illustration, not a new world solid or place.'})
themes=[]
for era in ['revenge','parade','danger']:
 old=BASE/era;folder=OUT/era;folder.mkdir(exist_ok=True)
 for name in ['atlas.png','native-atlas.png']:shutil.copyfile(old/name,folder/name)
 layout=read(old/'layout.json');layout.update(version=rev,geometry_revision=geo,geometry_hash=p['geometry_hash'],ground='../ground.png',overview_representations=reps,
   runtime_derivative='Original editable paving-union edge ground; two corrected MAP-only northern facade representations. Exact spatial-v4 world placements/collision and original v8 atlas bytes. Explicit separate MAP label sidecar.')
 write(folder/'layout.json',layout)
 scenery=read(old/'scenery.json');scenery['ground']=ground;write(folder/'scenery.json',scenery)
 meta=read(old/'map-metadata.json');meta.update(version=rev,geometry_hash=p['geometry_hash'],overview_representations=reps,map_label_presentation={'path':label_relative,'sha256':sha(label_path),'mode':'Explicit MAP-only vector/text composition, not a new C06 scenery PNG or interaction.'});write(folder/'map-metadata.json',meta)
 resources=read(old/'resource-inputs.json');resources.update(version=rev,geometry_hash=p['geometry_hash'],projection=f'../../spatial-v{geo}/integer-projection.json',resources=[ground,*scenery['atlases']],total_bytes=ground['bytes']+scenery['atlases'][0]['bytes']);assert resources['total_bytes']<=2097152;write(folder/'resource-inputs.json',resources)
 provenance={'version':rev,'geometry_hash':p['geometry_hash'],'status':'PROPOSAL_NOT_RELEASE_BOUND','source_pins':[
  {'path':f'../../quality-v8/{era}/atlas.png','sha256':sha(old/'atlas.png')},{'path':'../ground.png','sha256':sha(OUT/'ground.png')},{'path':'../route-edges.json','sha256':sha(OUT/'route-edges.json')},{'path':label_relative,'sha256':sha(label_path)}],
  'atlas':'Byte-identical original v8 atlas/native padding. No source image regeneration or character art.',
  'ground':'Native editable vector union-edge strokes over exact frozen v6 original ground; source/export/lossless RGB proof in ground-native-evidence.json.',
  'geometry':'All34 WORLD objects/footprints and canonical spatial-v4 grids/places/spawns/interactions unchanged. New two MAP facade views replace existing unbound hall images; no collision, place or interaction.',
  'source_review':'No C06 REVIEWED state or physical qualification is conferred. Original source/place provenance remains unchanged.'}
 write(folder/'provenance.json',provenance)
 themes.append({'theme_id':era.upper(),'bytes':resources['total_bytes'],'pixels':resources['total_pixels'],'atlas_sha256':scenery['atlases'][0]['sha256']})
write(OUT/'resource-summary.json',{'version':rev,'geometry_hash':p['geometry_hash'],'ground':ground,'themes':themes})
manifest={'version':rev,'format':f'MCR_REFERENCE_SCENE_SPATIAL{geo}_QUALITY{rev}','status':'CONTRACT_READY_SOURCE_NOT_RELEASE_BOUND','geometry_hash':p['geometry_hash'],
 'geometry':{'path':f'../spatial-v{geo}/geometry-proposal.json','sha256':sha(HERE/f'spatial-v{geo}/geometry-proposal.json')},
 'integer_projection':{'path':f'../spatial-v{geo}/integer-projection.json','sha256':sha(HERE/f'spatial-v{geo}/integer-projection.json')},
 'base_manifest':{'path':'../quality-v8/manifest.json','sha256':sha(BASE/'manifest.json')},
 'ground_sources':[pin(OUT/n) for n in ['ground.png','ground-native.png','ground-layout.svg','ground-editable.json','ground.tsrct','route-edges.json','ground-native-evidence.json']],
 'map_label_sidecar':{'path':'map-label-presentation.json' if rev==10 else '../map-labels-v1/presentation.json','sha256':sha(label_path)},
 'themes':[{'theme_id':era.upper(),'files':[pin(OUT/era/n) for n in ['scenery.json','layout.json','map-metadata.json','resource-inputs.json','provenance.json','atlas.png','native-atlas.png']]} for era in ['revenge','parade','danger']],
 'limits':['No canonical geometry/grid change, new place/interaction or new server revision required by this source change. Actual0448 map revision allocation and release/resource qualification remain with assigned owners.','Oldv8 and prior bindings remain immutable; consumer adoption of new ground/MAP metadata is explicit and separately validated.','No fourth scored panel or asserted target acceptance; stronger geometric route hierarchy is not certified by painted edges.']}
if rev==10:
 manifest['ground_sources'] += [pin(OUT/n) for n in ['material-ground.png','material-ground-native.png','material-ground-editable.json','material-ground.tsrct','material-ground-evidence.json','surface-native.png']]
 manifest['revision_evidence']={'path':'../spatial-v5/revision-evidence.json','sha256':sha(HERE/'spatial-v5/revision-evidence.json')}
 manifest['limits']=['New spatial-v5 terrain/hash requires a distinct canonical Tiled and server/client map revision greater than allocated 5; proposed 6 is not reserved or published. Collision, 34 props, spawns, interactions, places and cameras are unchanged.','Old 0448/map5 and 899/map4 sources/releases remain immutable; no source review or silent reseal.','No fourth critic score, whole-target, physical, app/device/multiuser acceptance.']
 for era in ['revenge','parade','danger']:
  f=OUT/era;l=read(f/'layout.json');l['runtime_derivative']='Continuous southwest diagonal route with explicit spatial-v5 terrain. Frozen 34 WORLD props/collision and original v8 atlas; v9 MAP facades and explicitly rebound label composition.';write(f/'layout.json',l)
  v=read(f/'provenance.json');v['geometry']='New spatial-v5 terrain and geometry hash; exact 34 WORLD props, footprints, collision, places, spawns, interactions and cameras. Eight MAP representations are identical to v9.';v['ground']='Original v6 texture quadrants on 121 native editable material layers clipped to the new paving; then native vector union edges. Source pixels unchanged.';write(f/'provenance.json',v)
 for theme in manifest['themes']:
  for f in theme['files']:
   file=OUT/f['path'];f.update(bytes=file.stat().st_size,sha256=sha(file))
write(OUT/'manifest.json',manifest)
print(json.dumps({'status':'SOURCE_AUTHORED','world_props':34,'map_representations':len(reps),'geometry_hash':p['geometry_hash'],'resources':themes}))
