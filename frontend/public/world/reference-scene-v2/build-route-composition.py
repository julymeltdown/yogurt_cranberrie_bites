#!/usr/bin/env python3
"""v9 ground and MAP-only north facades, retaining exact v8 world geometry."""
import copy,hashlib,json,shutil
from pathlib import Path
HERE=Path(__file__).resolve().parent;OUT=HERE/'quality-v9';BASE=HERE/'quality-v8'
read=lambda p:json.loads(p.read_text());write=lambda p,v:p.write_text(json.dumps(v,indent=2)+'\n')
sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
pin=lambda p:{'path':str(p.relative_to(OUT)),'bytes':p.stat().st_size,'sha256':sha(p)}
p=read(HERE/'spatial-v4/geometry-proposal.json')
ground={'id':'quality_v9_route_edge_ground','path':'world/reference-scene-v2/quality-v9/ground.png','sha256':sha(OUT/'ground.png'),'width_px':768,'height_px':1296,'bytes':(OUT/'ground.png').stat().st_size}
reps=copy.deepcopy(read(BASE/'parade/layout.json')['overview_representations'])
for ident,replace,x in [('north_west_facade_representation','northern_hall',-12),('north_east_facade_representation','north_eastern_hall',542)]:
 reps.append({'id':ident,'replaces_prop_id':replace,'frame':'north_hall','x':x,'y':6,'width':210,'height':144,'depth_y':150,'interaction_id':None,
              'kind':'cropped_unbound_map_architecture_not_a_world_solid','reason':'Show the small north venue facade in front of the canopy band in overview scale; replace existing unbound hall illustration, not a new world solid or place.'})
themes=[]
for era in ['revenge','parade','danger']:
 old=BASE/era;folder=OUT/era;folder.mkdir(exist_ok=True)
 for name in ['atlas.png','native-atlas.png']:shutil.copyfile(old/name,folder/name)
 layout=read(old/'layout.json');layout.update(version=9,ground='../ground.png',overview_representations=reps,
   runtime_derivative='Original editable paving-union edge ground; two corrected MAP-only northern facade representations. Exact spatial-v4 world placements/collision and original v8 atlas bytes. Explicit separate MAP label sidecar.')
 write(folder/'layout.json',layout)
 scenery=read(old/'scenery.json');scenery['ground']=ground;write(folder/'scenery.json',scenery)
 meta=read(old/'map-metadata.json');meta.update(version=9,overview_representations=reps,map_label_presentation={'path':'../../map-labels-v1/presentation.json','sha256':sha(HERE/'map-labels-v1/presentation.json'),'mode':'Explicit MAP-only vector/text composition, not a new C06 scenery PNG or interaction.'});write(folder/'map-metadata.json',meta)
 resources=read(old/'resource-inputs.json');resources.update(version=9,resources=[ground,*scenery['atlases']],total_bytes=ground['bytes']+scenery['atlases'][0]['bytes']);assert resources['total_bytes']<=2097152;write(folder/'resource-inputs.json',resources)
 provenance={'version':9,'geometry_hash':p['geometry_hash'],'status':'PROPOSAL_NOT_RELEASE_BOUND','source_pins':[
  {'path':f'../../quality-v8/{era}/atlas.png','sha256':sha(old/'atlas.png')},{'path':'../ground.png','sha256':sha(OUT/'ground.png')},{'path':'../route-edges.json','sha256':sha(OUT/'route-edges.json')},{'path':'../../map-labels-v1/presentation.json','sha256':sha(HERE/'map-labels-v1/presentation.json')}],
  'atlas':'Byte-identical original v8 atlas/native padding. No source image regeneration or character art.',
  'ground':'Native editable vector union-edge strokes over exact frozen v6 original ground; source/export/lossless RGB proof in ground-native-evidence.json.',
  'geometry':'All34 WORLD objects/footprints and canonical spatial-v4 grids/places/spawns/interactions unchanged. New two MAP facade views replace existing unbound hall images; no collision, place or interaction.',
  'source_review':'No C06 REVIEWED state or physical qualification is conferred. Original source/place provenance remains unchanged.'}
 write(folder/'provenance.json',provenance)
 themes.append({'theme_id':era.upper(),'bytes':resources['total_bytes'],'pixels':resources['total_pixels'],'atlas_sha256':scenery['atlases'][0]['sha256']})
write(OUT/'resource-summary.json',{'version':9,'geometry_hash':p['geometry_hash'],'ground':ground,'themes':themes})
manifest={'version':9,'format':'MCR_REFERENCE_SCENE_SPATIAL4_QUALITY9','status':'CONTRACT_READY_SOURCE_NOT_RELEASE_BOUND','geometry_hash':p['geometry_hash'],
 'geometry':{'path':'../spatial-v4/geometry-proposal.json','sha256':sha(HERE/'spatial-v4/geometry-proposal.json')},
 'integer_projection':{'path':'../spatial-v4/integer-projection.json','sha256':sha(HERE/'spatial-v4/integer-projection.json')},
 'base_manifest':{'path':'../quality-v8/manifest.json','sha256':sha(BASE/'manifest.json')},
 'ground_sources':[pin(OUT/n) for n in ['ground.png','ground-native.png','ground-layout.svg','ground-editable.json','ground.tsrct','route-edges.json','ground-native-evidence.json']],
 'map_label_sidecar':{'path':'../map-labels-v1/presentation.json','sha256':sha(HERE/'map-labels-v1/presentation.json')},
 'themes':[{'theme_id':era.upper(),'files':[pin(OUT/era/n) for n in ['scenery.json','layout.json','map-metadata.json','resource-inputs.json','provenance.json','atlas.png','native-atlas.png']]} for era in ['revenge','parade','danger']],
 'limits':['No canonical geometry/grid change, new place/interaction or new server revision required by this source change. Actual0448 map revision allocation and release/resource qualification remain with assigned owners.','Oldv8 and prior bindings remain immutable; consumer adoption of new ground/MAP metadata is explicit and separately validated.','No fourth scored panel or asserted target acceptance; stronger geometric route hierarchy is not certified by painted edges.']}
write(OUT/'manifest.json',manifest)
print(json.dumps({'status':'SOURCE_AUTHORED','world_props':34,'map_representations':len(reps),'geometry_hash':p['geometry_hash'],'resources':themes}))
