#!/usr/bin/env python3
"""Register one original cozy environment sheet; no browser or PNG ground export."""
from pathlib import Path
import base64
import hashlib
import io
import json
import sys
import subprocess
from PIL import Image

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parent
ROOT = SOURCE.parents[3]
VERIFY = '--verify' in sys.argv
BASE = '7c25078f118da53dd9a524d0a58e707cca190f0f'
RAW_SHA = 'db77361f6366157de19e32fb73b988e5f04883269c1f0a53d9a1a086a1d0b514'
GEOMETRY = 'sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395'
PINNED = {
 'architecture-v2/parade/layout.json': '1dabdcd979ae3d64ebfdb834ce0319f25869d20f6d354e69f0f2c2e8d68f619d',
 'architecture-v2/parade/atlas.png': 'aab0383b8963f1b681acf81d22d2be88e130744554311e74f0793efbe388e529',
 'quality-v10/ground.png': 'dd70d8fb02e1c6e57e9a9f9e96d69dc7c15999bc076678a7075161281c929887',
 'quality-v10/ground-layout.svg': '25fff0d51f12251b34bf072cf4e32f2c8281f65ce676836c6f7998740da64a24',
 'spatial-v5/geometry-proposal.json': 'a2cdd0e60c1f9cc7c3f093643760000e91a01e1fadffb51f9a76bd2c3a7a3a0f',
}
FRAMES = ['stage','white_merch_booth','white_food_booth','park_plaque',
 'tree_green','tree_yellow','tree_dark','tree_red','meeting_pavilion',
 'giveaway_pavilion','north_hall','west_hall','food_counter','plaza_furniture',
 'cross_banner','shrub_bed']
# Largest connected alpha>=128 component bounds, independently counted16.
# They identify original sprite rectangles, not segmentation/opacity edits.
CROPS = [
 [8,13,330,301],[346,31,610,298],[644,31,914,298],[947,57,1238,295],
 [15,349,311,611],[327,346,611,610],[634,349,928,610],[942,346,1243,610],
 [9,646,306,901],[325,651,603,899],[625,674,930,897],[944,653,1242,899],
 [18,991,291,1205],[324,1004,619,1205],[683,953,854,1228],[938,1003,1238,1201],
]
PRESERVE = {'stage','park_plaque'}
ERA_RECIPES = {
 'REVENGE': {'layout_sha256':'7769f6b6ef754f2c64fccc000ce995a4d0d87d1423e87f9bd8aacc4776b2f043',
  'atlas_sha256':'108d5483711e6bd755b328d08d3535141954ef81adc6eabfd066a88d8fe278aa',
  'rgb_percent_and_offset':[[92,20],[84,28],[87,24]],'palette':'warm strawberry / rose cream'},
 'DANGER': {'layout_sha256':'1dbea3beddbba01f62fae1170b3f626a5b65eae138e1aab2c0171c85888b2b9a',
  'atlas_sha256':'20f218966eb17f50992191f1128b45ca02aac9b00a1c1f39ba71637b68afa29d',
  'rgb_percent_and_offset':[[83,34],[94,16],[98,10]],'palette':'apricot / mint / lavender'},
}
def digest(b): return 'sha256:'+hashlib.sha256(b).hexdigest()
def pin(p):
 b=p.read_bytes()
 return {'path':str(p.relative_to(ROOT)),'bytes':len(b),'sha256':digest(b)}
def write(p,b):
 if isinstance(b,str): b=b.encode()
 if VERIFY: assert p.read_bytes()==b, str(p)
 else:
  p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(b)
def output(name,r): write(HERE/name,json.dumps(r,indent=2)+'\n')
def core(im): return im.getchannel('A').point(lambda a:255 if a>=32 else 0).getbbox()
def resource(p):
 r=pin(p);r['path']=str(p.relative_to(ROOT/'frontend/public'))
 im=Image.open(p);r.update(width_px=im.width,height_px=im.height,mime='image/png')
 return r

if '--derive-eras' in sys.argv:
 # This mode makes new PNGs: callers need the separately recorded derivative
 # allocation and binding scope, never the released first-generation offer.
 assert not VERIFY, 'Use --close --verify after the allocated derivative checks.'
 def argument(flag):
  assert flag in sys.argv and sys.argv.index(flag)+1<len(sys.argv),flag
  return sys.argv[sys.argv.index(flag)+1]
 allocation=argument('--allocation')
 scope_record=argument('--scope-record')
 assert allocation!='ART-COZY-SOURCE-202611040621Z'
 assert len(allocation)>=12 and len(scope_record)>=8
 ground=HERE/'parade/ground.png'
 assert ground.exists() and Image.open(ground).size==(768,1296)
 current=json.loads((HERE/'presentation.json').read_text())
 assert current['status']=='CONTRACT_READY_SOURCE_VALIDATED_NATIVE_NOT_RUN'
 # First PARADE source/ground must already have a committed handoff.
 for name in ['source-original.png','parade/ground.png','handoff.json']:
  relative=str((HERE/name).relative_to(ROOT))
  subprocess.run(['git','cat-file','-e',f'HEAD:{relative}'],cwd=ROOT,check=True)
 assert digest((HERE/'source-original.png').read_bytes())=='sha256:'+RAW_SHA
 sheet=Image.open(HERE/'source-original.png').convert('RGBA')
 eras={}
 for era,recipe in ERA_RECIPES.items():
  folder=SOURCE/'architecture-v2'/era.lower()
  assert hashlib.sha256((folder/'layout.json').read_bytes()).hexdigest()==recipe['layout_sha256']
  assert hashlib.sha256((folder/'atlas.png').read_bytes()).hexdigest()==recipe['atlas_sha256']
  layout=json.loads((folder/'layout.json').read_text())
  assert layout['geometry_hash']==GEOMETRY and list(layout['frames'])==FRAMES
  assert len(layout['placements'])==34 and len(layout['overview_representations'])==8
  old=Image.open(folder/'atlas.png').convert('RGBA');atlas=old.copy();records=[]
  lut=[]
  for percent,offset in recipe['rgb_percent_and_offset']:
   lut += [min(255,(percent*v+100*offset+50)//100)for v in range(256)]
  lut+=list(range(256))
  for name,rect in zip(FRAMES,CROPS):
   f=layout['frames'][name];x,y,w,h=[f[k]for k in ('x','y','width','height')]
   before=old.crop((x,y,x+w,y+h));baseline=core(before)[3]-1
   if name in PRESERVE:
    rendered=before;record={'frame':name,'policy':'EXACT_ORIGINAL_ERA_RGBA'}
   else:
    crop=sheet.crop(tuple(rect));colored=crop.point(lut)
    assert colored.getchannel('A').tobytes()==crop.getchannel('A').tobytes()
    scale=min((w-4)/colored.width,(baseline-1)/colored.height)
    size=(max(1,int(colored.width*scale+0.5)),max(1,int(colored.height*scale+0.5)))
    resized=colored.resize(size,Image.Resampling.NEAREST);bb=core(resized);assert bb
    px=(w-size[0])//2;py=baseline+1-bb[3]
    assert px>=0 and py>=0 and px+size[0]<=w and py+size[1]<=h,name
    rendered=Image.new('RGBA',(w,h));rendered.paste(resized,(px,py))
    assert core(rendered)[3]-1==baseline,name
    record={'frame':name,'policy':'EXPLICIT_RGB_PALETTE_UNCHANGED_ALPHA_NEAREST_FIT',
     'source_crop_xyxy':rect,'resized_dimensions':list(size),'destination_xy':[px,py],
     'alpha_preserved_before_sampling':True,'scale_limit':scale}
   atlas.paste(rendered,(x,y))
   record.update(frame_rect=f,original_core_base_y=baseline,
    output_core_base_y=core(rendered)[3]-1,rgba_sha256=digest(rendered.tobytes()))
   records.append(record)
  for name in PRESERVE:
   f=layout['frames'][name];box=(f['x'],f['y'],f['x']+f['width'],f['y']+f['height'])
   assert atlas.crop(box).tobytes()==old.crop(box).tobytes()
  buf=io.BytesIO();atlas.save(buf,format='PNG',optimize=True)
  assert len(buf.getvalue())+ground.stat().st_size<=2097152
  destination=HERE/era.lower()/'atlas.png';write(destination,buf.getvalue())
  eras[era]={'atlas':dict(resource(destination),id=era.lower()+'_architecture_v2_atlas'),
   'ground':resource(ground),'frame_rects':layout['frames'],'recipe':recipe,
   'original_layout':pin(folder/'layout.json'),'original_atlas':pin(folder/'atlas.png'),
   'frames':records,'geometry_hash':GEOMETRY,'decoded_pixels':2043904,
   'combined_png_bytes':len(buf.getvalue())+ground.stat().st_size,
   'native_visual_qualification':'NOT_RUN'}
 output('era-derivatives.json',{'format':'MCR_COZY_ERA_DERIVATIVES_V1',
  'status':'SOURCE_VALIDATED_NATIVE_NOT_RUN','allocation':allocation,'scope_record':scope_record,
  'source':pin(HERE/'source-original.png'),'shared_ground':resource(ground),
  'recipe_rounding':'min(255,(percent*channel+100*offset+50)//100); alpha identity',
  'eras':eras,'new_generation_calls':0,'character_changes':False,
  'actual_era_distinction_and_cuteness':'NOT_RUN_NATIVE_COMPARISON_REQUIRED'})
 print(json.dumps({'status':'DERIVATIVE_PIXELS_PREPARED_METADATA_CLOSURE_NEXT',
  'eras':list(eras),'new_generation_calls':0,'allocation':allocation}))
 sys.exit(0)

# After the finite source-output allocation is released, closure only reads
# immutable image headers/hashes and writes small JSON. It never repacks pixels.
if '--close' in sys.argv:
 for name,expected in PINNED.items():
  assert hashlib.sha256((SOURCE/name).read_bytes()).hexdigest()==expected,name
 assert hashlib.sha256((HERE/'source-original.png').read_bytes()).hexdigest()==RAW_SHA
 assert digest((HERE/'parade/atlas.png').read_bytes())=='sha256:9c77e42eb377fbab1df485d6c1e8e47221ac33aebbc03e9aecba3cc7bbcded9f'
 assert digest((HERE/'ground.svg').read_bytes())=='sha256:e52f9acdaf4bc491664125585776ae42fe34848a59f5edc0205880095a19de40'
 assert digest((HERE/'source-registration.json').read_bytes())=='sha256:f38f19148b8d9ee80dc7340de2ae1fa4388583977aeff1c0602514b006792121'
 ground=HERE/'parade/ground.png'
 ready=ground.exists()
 if ready:
  assert Image.open(ground).format=='PNG' and Image.open(ground).size==(768,1296)
  assert ground.stat().st_size+(HERE/'parade/atlas.png').stat().st_size<=2097152
 status='CONTRACT_READY_SOURCE_VALIDATED_NATIVE_NOT_RUN'if ready else 'SOURCE_PREPARED_AWAITING_ROOT_GROUND_PNG_EXPORT'
 p=json.loads((HERE/'presentation.json').read_text())
 p.update(status=status,ground=resource(ground)if ready else None)
 p['resource_budget'].update(closed_png_tuple=ready,
  actual_png_bytes=(HERE/'parade/atlas.png').stat().st_size+(ground.stat().st_size if ready else 0))
 output('presentation.json',p)
 h=json.loads((HERE/'handoff.json').read_text())
 names=['README.md','build.py','prompt.txt','source-original.png','source-registration.json',
  'ground.svg','ground-provenance.json','presentation.json','parade/atlas.png']
 if ready:names+=['parade/ground.png']
 if (HERE/'era-derivatives.json').exists():
  assert ready
  derivatives=json.loads((HERE/'era-derivatives.json').read_text())
  assert derivatives['source']==pin(HERE/'source-original.png')
  assert derivatives['shared_ground']==resource(ground)
  p['eras']={'PARADE':{'atlas':p['atlas'],'ground':p['ground'],'frame_rects':p['frame_rects']}}
  for era,record in derivatives['eras'].items():
   destination=HERE/era.lower()/'atlas.png'
   assert record['atlas']==dict(resource(destination),id=era.lower()+'_architecture_v2_atlas')
   assert record['ground']==resource(ground)
   p['eras'][era]={key:record[key]for key in ['atlas','ground','frame_rects']}
   names.append(era.lower()+'/atlas.png')
  names.append('era-derivatives.json')
  p['scope']['eras']=['PARADE','REVENGE','DANGER']
  p['scope']['other_eras']='THREE_SCENERY_SOURCE_TUPLES_ONLY'
  p['validation']['other_eras_cozy']='SOURCE_READY_NATIVE_NOT_RUN'
  p['era_derivatives']=pin(HERE/'era-derivatives.json')
  output('presentation.json',p)
 h.update(status=status,payloads=[pin(HERE/name)for name in names])
 output('handoff.json',h)
 print(json.dumps({'status':status,'metadata_only':True,'pixel_repacking':False,
  'payloads':len(names),'closed_png_tuple':ready}))
 sys.exit(0)

for name,expected in PINNED.items():
 assert hashlib.sha256((SOURCE/name).read_bytes()).hexdigest()==expected,name
layout=json.loads((SOURCE/'architecture-v2/parade/layout.json').read_text())
geometry=json.loads((SOURCE/'spatial-v5/geometry-proposal.json').read_text())
assert layout['geometry_hash']==geometry['geometry_hash']==GEOMETRY
assert list(layout['frames'])==FRAMES and len(layout['placements'])==34
raw=(HERE/'source-original.png').read_bytes()
assert len(raw)==2120690 and hashlib.sha256(raw).hexdigest()==RAW_SHA
sheet=Image.open(io.BytesIO(raw)).convert('RGBA')
assert sheet.size==(1254,1254)
old=Image.open(SOURCE/'architecture-v2/parade/atlas.png').convert('RGBA')
atlas=old.copy()
registrations=[]
for name,rect in zip(FRAMES,CROPS):
 f=layout['frames'][name];x,y,w,h=[f[k]for k in ('x','y','width','height')]
 before=old.crop((x,y,x+w,y+h));baseline=core(before)[3]-1
 if name in PRESERVE:
  rendered=before
  registration={'frame':name,'policy':'EXACT_ORIGINAL_RGBA','source_crop_unused':rect}
 else:
  crop=sheet.crop(tuple(rect))
  scale=min((w-4)/crop.width,(baseline-1)/crop.height)
  size=(max(1,int(crop.width*scale+0.5)),max(1,int(crop.height*scale+0.5)))
  resized=crop.resize(size,Image.Resampling.NEAREST)
  bb=core(resized);assert bb
  px=(w-size[0])//2;py=baseline+1-bb[3]
  assert px>=0 and py>=0 and px+size[0]<=w and py+size[1]<=h,name
  rendered=Image.new('RGBA',(w,h));rendered.paste(resized,(px,py))
  assert core(rendered)[3]-1==baseline,name
  registration={'frame':name,'policy':'SOURCE_RECT_RGBA_NEAREST_UNIFORM_FIT',
   'source_crop_xyxy':rect,'source_component_rule':'largest alpha>=128 connected component bounding rectangle; no alpha threshold applied to pixels',
   'scale_limit':scale,'resized_dimensions':list(size),'destination_xy':[px,py],
   'crop_alpha_1_to_31_pixels':sum(crop.getchannel('A').histogram()[1:32]),
   'alpha_cleanup':False,'repaint':False}
 atlas.paste(rendered,(x,y))
 registration.update(frame_rect=f,original_core_bbox=list(core(before)),
  original_core_base_y=baseline,output_core_bbox=list(core(rendered)),
  output_core_base_y=core(rendered)[3]-1,rgba_sha256=digest(rendered.tobytes()))
 registrations.append(registration)
for name in PRESERVE:
 f=layout['frames'][name];box=(f['x'],f['y'],f['x']+f['width'],f['y']+f['height'])
 assert atlas.crop(box).tobytes()==old.crop(box).tobytes()
buf=io.BytesIO();atlas.save(buf,format='PNG',optimize=True)
assert len(buf.getvalue())<=1550000
write(HERE/'parade/atlas.png',buf.getvalue())
output('source-registration.json',{
 'format':'MCR_COZY_ENVIRONMENT_SOURCE_REGISTRATION_V1','source_base_commit':BASE,
 'raw':pin(HERE/'source-original.png'),'raw_dimensions':[1254,1254],
 'tool':'Built-in image_gen; one image-edit call with existing environment atlas reference',
 'model_version':'NOT_EXPOSED','allocation':'ART-COZY-SOURCE-202611040621Z',
 'preserved_frames':sorted(PRESERVE),'registered_frames':14,'frames':registrations,
 'character_inputs':[],'character_changes':False,
 'limits':['Original sheet retained byte for byte. Rectangle crops omit exterior faint residue.',
  'RGBA inside each crop is unchanged before nearest sampling; faint edge residue can remain.',
  'Integer resize rounding is recorded; no canonical coordinates/feet/camera are changed.',
  'Source packing checks do not prove native occlusion, visual cuteness or human acceptance.']})

ground_bytes=(SOURCE/'quality-v10/ground.png').read_bytes()
uri='data:image/png;base64,'+base64.b64encode(ground_bytes).decode()
polygons=geometry['paving_polygons']
assert len(polygons)==43
poly=''.join('<polygon points="'+' '.join(f'{x},{y}'for x,y in p)+'"/>'for p in polygons)
svg=('<svg xmlns="http://www.w3.org/2000/svg" width="768" height="1296" viewBox="0 0 768 1296" color-interpolation-filters="sRGB" style="image-rendering:pixelated">'
 '<defs><filter id="lawn" x="0" y="0" width="100%" height="100%"><feComponentTransfer>'
 '<feFuncR type="linear" slope=".55" intercept=".32"/><feFuncG type="linear" slope=".60" intercept=".40"/><feFuncB type="linear" slope=".48" intercept=".40"/><feFuncA type="identity"/>'
 '</feComponentTransfer></filter><filter id="paving" x="0" y="0" width="100%" height="100%"><feComponentTransfer>'
 '<feFuncR type="linear" slope=".72" intercept=".27"/><feFuncG type="linear" slope=".65" intercept=".29"/><feFuncB type="linear" slope=".60" intercept=".31"/><feFuncA type="identity"/>'
 '</feComponentTransfer></filter><clipPath id="paving-mask">'+poly+'</clipPath>'
 '<image id="source-ground" href="'+uri+'" width="768" height="1296"/></defs>'
 '<use href="#source-ground" filter="url(#lawn)"/><g clip-path="url(#paving-mask)"><use href="#source-ground" filter="url(#paving)"/></g></svg>\n')
write(HERE/'ground.svg',svg)
output('ground-provenance.json',{
 'format':'MCR_COZY_CODE_NATIVE_GROUND_V1','dimensions':[768,1296],
 'original_ground':pin(SOURCE/'quality-v10/ground.png'),
 'original_edge_source':pin(SOURCE/'quality-v10/ground-layout.svg'),
 'geometry':pin(SOURCE/'spatial-v5/geometry-proposal.json'),'geometry_hash':GEOMETRY,
 'paving_polygon_count':43,'exact_unrounded_polygon_coordinates':polygons,
 'svg_coordinate_serialization':'Python round-trip float strings; exact source geometry numbers, no coordinate rounding',
 'method':'Embedded original PNG with two SVG color-transfer filters; original paving polygons only select palette.',
 'immutable':['original material pixels','terrain topology','all paths/coordinates in source geometry','object placements/feet','camera','labels/intents','all character inputs'],
 'lawn_rgb_slope_intercept':[[.55,.32],[.60,.40],[.48,.40]],
 'paving_rgb_slope_intercept':[[.72,.27],[.65,.29],[.60,.31]],
 'png_export_owner':'Root153 ONLY, separately allocated; build.py never writes parade/ground.png',
 'limits':'Color-source proposal, not a freshly qualified native screenshot. No spatial warp, new path, fake object or recolored character.'})

ground=HERE/'parade/ground.png'
ready=ground.exists()
if ready:
 assert Image.open(ground).format=='PNG' and Image.open(ground).size==(768,1296)
 assert ground.stat().st_size+(HERE/'parade/atlas.png').stat().st_size<=2097152
status='CONTRACT_READY_SOURCE_VALIDATED_NATIVE_NOT_RUN'if ready else 'SOURCE_PREPARED_AWAITING_ROOT_GROUND_PNG_EXPORT'
output('presentation.json',{
 'format':'MCR_COZY_SCENERY_SOURCE_V1','version':1,'status':status,
 'source_base_commit':BASE,'geometry_hash':GEOMETRY,
 'scope':{'views':['WORLD','MAP'],'eras':['PARADE'],'other_eras':'UNCHANGED',
  'characters':'ALL_ORIGINAL_BYTES_UV_RENDERERS_CROPS_ANCHORS_SIZING_FROZEN',
  'foreground':'ALL_TEXT_TYPOGRAPHY_CONTROLS_LAYOUT_LABELS_INTENTS_FROZEN'},
 'before':{'candidate':'6ca52f8851473090479f1c13a33e879b6cd099d0',
  'captured_at':'2026-11-04T06:05:14.291358Z','map_dimensions':[390,844],
  'map_bytes':410326,'map_sha256':'sha256:87ab843422292a7701e2a3fe39a4419ef95e52721c6c2b052b617c6c2d135b01'},
 'reference':{'sha256':'sha256:2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23',
  'meaning':'Human requires visibly CUTER backgrounds than both this unchanged collage and current product; dark pixel parity superseded.'},
 'atlas':dict(resource(HERE/'parade/atlas.png'),id='parade_architecture_v2_atlas'),
 'ground':resource(ground)if ready else None,'ground_svg':pin(HERE/'ground.svg'),
 'frame_rects':layout['frames'],'registration':pin(HERE/'source-registration.json'),
 'immutable_inputs':[pin(SOURCE/p)for p in PINNED],
 'retained_placements':{'world':34,'map':51,'labels':6,'coordinates_depth_ids_unchanged':True},
 'style':'Dominant cream/peach/lavender buildings and rounded sage/mint/butter-yellow/blush canopies; mint lawns and honey-peach paving.',
 'resource_budget':{'max_png_bytes':2097152,'decoded_pixels':2043904,
  'actual_png_bytes':(HERE/'parade/atlas.png').stat().st_size+(ground.stat().st_size if ready else 0),
  'closed_png_tuple':ready},
 'consumer':'Root-only minimal descriptor tuple adapter; same PARADE source serves World+Map. No renderer/loader/Phaser/canonical mutation.',
 'validation':{'registered_frames':14,'original_frames_byte_equal':2,'frame_rects_exact':16,
  'frame_core_base_y_exact':16,'character_assets_used':0,'new_browser_native_runs':0,
  'actual_390_world_map_after':'NOT_RUN','affected_360_430_after':'NOT_RUN',
  'other_eras_cozy':'NOT_RUN_UNCHANGED','visual_acceptance':'OPEN_FIRST_GLANCE_CUTER_REQUIRED'}})
names=['README.md','build.py','prompt.txt','source-original.png','source-registration.json',
 'ground.svg','ground-provenance.json','presentation.json','parade/atlas.png']
if ready:names+=['parade/ground.png']
output('handoff.json',{'format':'MCR_COZY_SCENERY_HANDOFF_V1','status':status,
 'payloads':[pin(HERE/name)for name in names],
 'root_export_ownership':'Root153 produces parade/ground.png from pinned ground.svg, then explicitly stops/releases; ART subsequently closes hashes without modifying PNG.',
 'limits':'Source-only. No actual warm native World/Map AFTER or ten-screen visual acceptance is claimed.'})
print(json.dumps({'status':status,'atlas_bytes':(HERE/'parade/atlas.png').stat().st_size,
 'ground_svg_bytes':(HERE/'ground.svg').stat().st_size,'frames':16,'preserved':2,
 'registered':14,'closed_png_tuple':ready}))
