#!/usr/bin/env python3
"""Pack original era sources into versioned, geometry-identical C06 art inputs."""
import copy
import hashlib
import json
import math
from pathlib import Path
from PIL import Image

HERE=Path(__file__).resolve().parent
OUT=HERE/'eras-v5'
read=lambda p:json.loads(p.read_text())
write=lambda p,o:p.write_text(json.dumps(o,indent=2)+'\n')
sha=lambda p:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
nearest=lambda v:math.floor(v+.5) if v>=0 else math.ceil(v-.5)
base=read(HERE/'runtime-v4/parade-layout.json')
base_scenery=read(HERE/'runtime-v4/parade-scenery.json')
base_map=read(HERE/'runtime-v4/parade-map-metadata.json')
names=list(base['frames'])
results=[]
for era in ['revenge','danger','parade']:
    folder=OUT/era;source_path=folder/'source-atlas.png'
    if not source_path.exists():continue
    source=Image.open(source_path)
    assert source.mode=='RGBA' and source.getchannel('A').getextrema()==(0,255)
    # Measure metadata boundaries through transparent inter-object gutters;
    # equal mathematical quarters can clip a generated tree root by one pixel.
    alpha=source.getchannel('A')
    def gutter(target, extent, score):
        candidates=range(max(1,nearest(target)-40),min(extent-1,nearest(target)+40)+1)
        return min(candidates,key=lambda n:(score(n),abs(n-target),n))
    rows=[0]+[gutter(i*source.height/4,source.height,lambda y:sum(alpha.getpixel((x,y))>=32 for x in range(source.width))) for i in [1,2,3]]+[source.height]
    grids=[];frames={}
    for row in range(4):
        top,bottom=rows[row:row+2]
        cols=[0]+[gutter(i*source.width/4,source.width,lambda x:sum(alpha.getpixel((x,y))>=32 for y in range(top,bottom))) for i in [1,2,3]]+[source.width]
        grids.append({'row':row,'y':[top,bottom],'columns':cols})
        for col in range(4):
            i=row*4+col;name=names[i];left,right=cols[col:col+2]
            cell_alpha=alpha.crop((left,top,right,bottom)).point(lambda a:255 if a>=32 else 0)
            bounds=cell_alpha.getbbox();assert bounds,name
            x=max(left,left+bounds[0]-2);y=max(top,top+bounds[1]-2)
            x2=min(right,left+bounds[2]+2);y2=min(bottom,top+bounds[3]+2)
            frames[name]={'x':x,'y':y,'width':x2-x,'height':y2-y,'cell':i}
    for candidate_scale in [.75,.70,.65,.60,.55]:
        atlas=Image.new('RGBA',(1024,1024));packed={};transforms=[]
        scale=min(candidate_scale,244/max(max(f['width'],f['height']) for f in frames.values()))
        for i,(name,f) in enumerate(frames.items()):
            width,height=nearest(f['width']*scale),nearest(f['height']*scale)
            x=(i%4)*256+(256-width)//2;y=(i//4)*256+(256-height)//2
            rect={k:f[k] for k in ['x','y','width','height']}
            crop=source.crop((f['x'],f['y'],f['x']+f['width'],f['y']+f['height'])).resize((width,height),Image.Resampling.NEAREST)
            atlas.paste(crop,(x,y));packed[name]={'x':x,'y':y,'width':width,'height':height}
            transforms.append({'frame':name,'source_rect':rect,'packed_rect':packed[name],'scale':scale,'resampling':'nearest','rounding':'nearest, ties away from zero'})
        atlas.save(folder/'atlas.png',optimize=True,compress_level=9)
        if (folder/'atlas.png').stat().st_size+base_scenery['ground']['bytes']<=2097152:break
    else:raise AssertionError('Era atlas cannot fit incumbent scenery budget')
    native_atlas=Image.new('RGBA',(2048,2048));native_atlas.paste(atlas,(0,0));native_atlas.save(folder/'native-atlas.png',optimize=True,compress_level=9)
    layout=copy.deepcopy(base);layout.update(version=5,theme_id=era.upper(),ground='../../runtime-v4/parade-ground.png',atlas='atlas.png',frames=packed,native_atlas='native-atlas.png',native_atlas_dimensions=[2048,2048])
    layout['skins']='Distinct original three-era variants; geometry/visual destinations use the explicit frozen PARADE v4 integer projection.'
    layout['runtime_derivative']='Original built-in imagegen era source, non-destructive measured 4x4 frames, declared nearest-neighbor packing. Physical geometry and integer destination rectangles are unchanged.'
    write(folder/'layout.json',layout)
    scenery=copy.deepcopy(base_scenery);aid=f'{era}_v5_atlas'
    scenery['atlases']=[{'id':aid,'path':f'world/reference-scene-v2/eras-v5/{era}/atlas.png','sha256':sha(folder/'atlas.png'),'width_px':1024,'height_px':1024,'bytes':(folder/'atlas.png').stat().st_size}]
    for o,p in zip(scenery['objects'],layout['placements']):o.update(atlas_id=aid,frame=packed[p['frame']])
    write(folder/'scenery.json',scenery)
    meta=copy.deepcopy(base_map);meta.update(version=5,world_scenery='scenery.json',layout='layout.json')
    write(folder/'map-metadata.json',meta)
    provenance={'version':5,'theme_id':era.upper(),'status':'PROPOSAL_NOT_RELEASE_BOUND','worker_base':'e3af146ae4757794ca153363fd1408300ffde29e','generation_ref':{'revenge':'exec-d9be6451-d4e8-416e-bf22-3ce4d375704a.png','danger':'exec-e0cf1d5c-b66e-46e4-859b-a96298fcfc7a.png','parade':'exec-aa34a456-783f-441e-9d67-d3e87c3b22dd.png'}[era],'method':'Built-in imagegen original character-free era edit of own source atlas, then explicitly authorized non-generative source-preserving nearest-neighbor packing. No source PNG pixel rewrites.',
        'input_source':({'path':'../revenge/source-atlas.png','sha256':sha(OUT/'revenge/source-atlas.png'),'revision':None,'revision_status':'SAME_V5_HANDOFF'} if era=='parade' else {'path':'../../painted/parade-source-atlas-v3.png','sha256':sha(HERE/'painted/parade-source-atlas-v3.png'),'revision':'8f89d1905a86143d34ff77d2951fc60651ab2497'}),
        'native_packaging':{'path':'native-atlas.png','sha256':sha(folder/'native-atlas.png'),'width':2048,'height':2048,'method':'Identical runtime atlas pixels at0,0 with transparent padding only; native CLI0.4 image-mask surface otherwise clips world-space masks below source height1024. Not a runtime PNG fetch.'},
        'generated_source':{'path':'source-atlas.png','sha256':sha(source_path),'width':source.width,'height':source.height},
        'runtime_atlas':scenery['atlases'][0],'shared_ground':scenery['ground'],'measured_source_grid':grids,'source_frames':frames,'transforms':transforms,
        'geometry_hash':base['geometry_hash'],'integer_projection':'../../runtime-v4/parade-integer-projection.json','prompt':'prompt.md',
        'runtime_resources':2,'combined_pixels':1024*1024+768*1296,'combined_bytes':scenery['ground']['bytes']+scenery['atlases'][0]['bytes'],
        'limits':{'pixels':2097152,'bytes':2097152},'shared_ground_policy':'Intentional shared quiet physical ground across all eras, not a PARADE atlas alias. Each era has a distinct original atlas/hash. Theme swaps change only scenery.',
        'identities':'No new place/operator/interaction IDs; no physical arrival/routing/provenance claim.'}
    write(folder/'provenance.json',provenance)
    write(folder/'resource-inputs.json',{'version':5,'theme_id':era.upper(),'status':'PROPOSAL_NOT_RELEASE_BOUND','geometry_hash':base['geometry_hash'],'scenery':'scenery.json','layout':'layout.json','integer_projection':'../../runtime-v4/parade-integer-projection.json','map_metadata':'map-metadata.json','provenance':'provenance.json','art_review_state':'NOT_ACCEPTED','physical_place_provenance':'UNREVIEWED','resources':[scenery['ground'],scenery['atlases'][0]],'combined_pixels':provenance['combined_pixels'],'combined_bytes':provenance['combined_bytes'],'budget_pass':True,'metadata_pins':[{ 'path':name,'sha256':sha(folder/name),'bytes':(folder/name).stat().st_size} for name in ['scenery.json','layout.json','map-metadata.json','provenance.json','prompt.md']]})
    # Editable vector wrapper samples unchanged sources; each named object is independent.
    for view in ['world','map']:
        props=copy.deepcopy(layout['placements'])
        zone=''
        if view=='map':
            replaced={o.get('replaces_prop_id') for o in layout['overview_representations']}
            props=[o for o in props if o['id'] not in replaced]+layout['overview_representations']
            points=' '.join(f'{x},{y}' for x,y in meta['plaza_parcel'])
            zone=f'<polygon id="semantic-plaza-not-terrain" points="{points}" fill="#7a8742" stroke="#b4a27d" stroke-width="6"/>'
        objects=''
        for o in sorted(props,key=lambda o:o['depth_y']):
            f=packed[o['frame']]
            objects+=f'<svg id="{o["id"]}" x="{o["x"]}" y="{o["y"]}" width="{o["width"]}" height="{o["height"]}" viewBox="{f["x"]} {f["y"]} {f["width"]} {f["height"]}" preserveAspectRatio="none"><image href="atlas.png" width="1024" height="1024"/></svg>'
        (folder/f'{view}-layout.svg').write_text(f'<svg xmlns="http://www.w3.org/2000/svg" width="768" height="1296" viewBox="0 0 768 1296"><title>{era.upper()} {view.upper()} editable original scenery; no characters or baked map labels</title><image id="ground" href="../../runtime-v4/parade-ground.png" width="768" height="1296"/>{zone}{objects}</svg>')
    results.append({'theme_id':era.upper(),'source_sha256':provenance['generated_source']['sha256'],'atlas_sha256':scenery['atlases'][0]['sha256'],'bytes':provenance['combined_bytes'],'pixels':provenance['combined_pixels']})
if len(results)==3:
    index={'format':'MCR_REFERENCE_SCENE_THREE_ERA_ART_INPUTS','version':5,'status':'CONTRACT_READY','visual_acceptance':'IN_PROGRESS_NOT_ACCEPTED','production_admission':'NOT_ADMITTED','physical_place_provenance':'UNREVIEWED',
        'geometry':{'path':'../geometry-proposal.json','sha256':sha(HERE/'geometry-proposal.json'),'geometry_hash':base['geometry_hash'],'source_revision':'8f89d1905a86143d34ff77d2951fc60651ab2497'},
        'integer_projection':{'path':'../runtime-v4/parade-integer-projection.json','sha256':sha(HERE/'runtime-v4/parade-integer-projection.json'),'source_revision':'e3af146ae4757794ca153363fd1408300ffde29e'},
        'themes':[{'theme_id':era.upper(),'files':[{ 'path':era+'/'+name,'sha256':sha(OUT/era/name),'bytes':(OUT/era/name).stat().st_size} for name in ['source-atlas.png','atlas.png','scenery.json','layout.json','map-metadata.json','resource-inputs.json','provenance.json','prompt.md']]} for era in ['revenge','parade','danger']],
        'adoption':'Versioned art inputs only. Coordinate actual server/frontend release revision and pins. Do not alter frozen v4 pins, treat aliases as themes, or infer provenance review from art verification.',
        'static_map_owner':'WORLD developer exclusively owns sibling ../runtime/** canonical static Tiled/descriptor; this art manifest does not produce a competing map.',
        'limitations':['Place/source review and operator bundle unavailable here; all nested places remain byte/value-equivalent UNREVIEWED.', 'Only virtual concept scenery. No physical arrival, safe routing or event-operation claim.', 'No target-fidelity acceptance, application/Phaser/device/multiuser release proof.']}
    write(OUT/'manifest.json',index)
print(json.dumps({'built':results}))
