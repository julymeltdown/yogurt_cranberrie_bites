#!/usr/bin/env python3
"""Versioned source-only urban venue correction on frozen spatial-v3/v6 inputs."""
import copy
import hashlib
import json
import math
from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent
BASE = HERE / 'quality-v6'
OUT = HERE / 'quality-v7'
ERAS = ['revenge', 'parade', 'danger']
VENUES = ['meeting_pavilion', 'giveaway_pavilion', 'north_hall', 'west_hall']
read = lambda p: json.loads(p.read_text())
write = lambda p, v: p.write_text(json.dumps(v, indent=2) + '\n')
sha = lambda p: 'sha256:' + hashlib.sha256(p.read_bytes()).hexdigest()
nearest = lambda n: math.floor(n + .5)

source_path = OUT / 'sources/venue-sheet.png'
source = Image.open(source_path).convert('RGBA')
alpha = source.getchannel('A')
assert alpha.getextrema() == (0, 255)
def gutter(target, extent, score):
    return min(range(max(1, nearest(target)-35), min(extent-1, nearest(target)+35)+1), key=lambda n: (score(n), abs(n-target), n))
rows = [0] + [gutter(i*source.height/4, source.height, lambda y: sum(alpha.getpixel((x,y)) >= 32 for x in range(source.width))) for i in [1,2,3]] + [source.height]
crops = {}
grids = []
for row, name in enumerate(VENUES):
    top, bottom = rows[row:row+2]
    cols = [0] + [gutter(i*source.width/3, source.width, lambda x: sum(alpha.getpixel((x,y)) >= 32 for y in range(top,bottom))) for i in [1,2]] + [source.width]
    grids.append({'row': row, 'y': [top,bottom], 'columns': cols})
    for col, era in enumerate(ERAS):
        left, right = cols[col:col+2]
        b = alpha.crop((left,top,right,bottom)).point(lambda a: 255 if a >= 32 else 0).getbbox()
        assert b
        x, y = max(left,left+b[0]-2), max(top,top+b[1]-2)
        x2, y2 = min(right,left+b[2]+2), min(bottom,top+b[3]+2)
        crops[(era,name)] = {'x':x,'y':y,'width':x2-x,'height':y2-y}

geometry = read(BASE/'manifest.json')['geometry']
results = []
for era in ERAS:
    old = BASE/era
    folder = OUT/era
    folder.mkdir(exist_ok=True)
    layout = read(old/'layout.json')
    atlas = Image.open(old/'atlas.png').convert('RGBA')
    transforms = []
    old_provenance = read(old/'provenance.json')
    for t in old_provenance['transforms']:
        t = copy.deepcopy(t)
        if t['frame'] not in VENUES:
            # Relative paths with a source-sheet parent need the frozen v6 prefix.
            if t['source'].startswith('../sources/'):
                t['source'] = '../../quality-v6/' + t['source'][3:]
            transforms.append(t)
            continue
        name = t['frame']
        previous = layout['frames'][name]
        slot_x, slot_y = previous['x']//256*256, previous['y']//256*256
        atlas.paste((0,0,0,0), (slot_x,slot_y,slot_x+256,slot_y+256))
        # Bottom-aligned transparent letterboxing preserves the low source facade
        # instead of stretching its roof into the old cottage-shaped image box.
        frame = {'x':slot_x+6,'y':slot_y+38,'width':244,'height':180}
        rect = crops[(era,name)]
        scale = min(240/rect['width'],176/rect['height'])
        w,h = nearest(rect['width']*scale), nearest(rect['height']*scale)
        packed = {'x':frame['x']+(244-w)//2,'y':frame['y']+180-h-2,'width':w,'height':h}
        crop = source.crop((rect['x'],rect['y'],rect['x']+rect['width'],rect['y']+rect['height'])).resize((w,h),Image.Resampling.NEAREST)
        atlas.paste(crop,(packed['x'],packed['y']))
        layout['frames'][name] = frame
        transforms.append({'frame':name,'source':'../sources/venue-sheet.png','source_rect':rect,'packed_rect':packed,'frame_rect':frame,'scale':scale,'resampling':'nearest','rounding':'nearest ties away from zero','padding':'Transparent, horizontally centered, bottom aligned with 2px base margin; no source repaint or nonuniform source scaling.'})
    atlas.save(folder/'atlas.png',optimize=True,compress_level=9)
    native = Image.new('RGBA',(2048,2048)); native.paste(atlas,(0,0)); native.save(folder/'native-atlas.png',optimize=True,compress_level=9)
    layout.update(version=7,ground='../../quality-v6/ground.png',runtime_derivative='Four original broad brick venue identities replace cottage source frames; uniform nearest crops with bottom-aligned transparent frame padding. Other twelve frames, spatial-v3 destinations, footprints, cameras and cells are frozen v6 inputs.')
    write(folder/'layout.json',layout)
    scenery = read(old/'scenery.json')
    aid = era+'_quality_v7_atlas'
    descriptor = {'id':aid,'path':f'world/reference-scene-v2/quality-v7/{era}/atlas.png','sha256':sha(folder/'atlas.png'),'width_px':1024,'height_px':1024,'bytes':(folder/'atlas.png').stat().st_size}
    scenery['atlases'] = [descriptor]
    for o,p in zip(scenery['objects'],layout['placements']):
        o.update(atlas_id=aid,frame=layout['frames'][p['frame']])
    write(folder/'scenery.json',scenery)
    meta = read(old/'map-metadata.json'); meta['version'] = 7
    write(folder/'map-metadata.json',meta)
    provenance = {'version':7,'status':'PROPOSAL_NOT_RELEASE_BOUND','theme_id':era.upper(),'base_revision':'bc731ed868ec30b33ae5dd1e6f13e97e68ecabd9','geometry_hash':layout['geometry_hash'],'source_pins':[{'path':'../sources/venue-sheet.png','sha256':sha(source_path)},{'path':'../../quality-v6/'+era+'/atlas.png','sha256':sha(old/'atlas.png')},{'path':'../../quality-v6/'+era+'/layout.json','sha256':sha(old/'layout.json')},{'path':'../../quality-v6/manifest.json','sha256':sha(BASE/'manifest.json')}], 'transforms':transforms,'native_packaging':{'path':'native-atlas.png','sha256':sha(folder/'native-atlas.png'),'width':2048,'height':2048,'method':'Unchanged runtime atlas pixels at0,0, transparent native-only padding.'},'fidelity':'Concrete urban-architecture correction, unscored and not reference accepted. Perimeter density remains required.','physical_provenance':'All four truthful virtual-concept place records unchanged; source-art qualification is separate from physical-destination authorization.'}
    write(folder/'provenance.json',provenance)
    resources = read(old/'resource-inputs.json')
    resources.update(version=7,resources=[scenery['ground'],descriptor],total_bytes=scenery['ground']['bytes']+descriptor['bytes'])
    assert resources['total_bytes'] <= resources['limits']['bytes']
    write(folder/'resource-inputs.json',resources)
    points = lambda ps: ' '.join(f'{x},{y}' for x,y in ps)
    for view in ['world','map']:
        props = copy.deepcopy(layout['placements']); zone = ''
        if view == 'map':
            replaced = {o.get('replaces_prop_id') for o in layout['overview_representations']}
            props = [o for o in props if o['id'] not in replaced]+layout['overview_representations']
            zone = f'<polygon id="semantic-plaza" points="{points(meta["plaza_parcel"])}" fill="#7a8742" stroke="#b4a27d" stroke-width="6"/>'
        objects = ''.join(f'<svg id="{o["id"]}" x="{o["x"]}" y="{o["y"]}" width="{o["width"]}" height="{o["height"]}" viewBox="{layout["frames"][o["frame"]]["x"]} {layout["frames"][o["frame"]]["y"]} {layout["frames"][o["frame"]]["width"]} {layout["frames"][o["frame"]]["height"]}" preserveAspectRatio="none"><image href="atlas.png" width="1024" height="1024"/></svg>' for o in sorted(props,key=lambda o:o['depth_y']))
        (folder/f'{view}-layout.svg').write_text(f'<svg xmlns="http://www.w3.org/2000/svg" width="768" height="1296"><title>v7 {era} {view}: editable original brick venues, frozen spatial-v3</title><image href="../../quality-v6/ground.png" width="768" height="1296"/>{zone}{objects}</svg>')
    results.append({'theme_id':era.upper(),'bytes':resources['total_bytes'],'pixels':resources['total_pixels'],'atlas_sha256':descriptor['sha256']})

source_metadata = {'version':7,'status':'ORIGINAL_ART_NOT_REFERENCE_ACCEPTED','source_sheet':{'path':'sources/venue-sheet.png','sha256':sha(source_path),'bytes':source_path.stat().st_size,'width':source.width,'height':source.height},'source_grid':grids,'venue_frames':[{'era':e,'frame':f,'rect':r} for (e,f),r in crops.items()],'prompt':'sources/prompt.md','base_manifest':{'path':'../quality-v6/manifest.json','sha256':sha(BASE/'manifest.json')},'geometry_hash':geometry['geometry_hash'],'unchanged':'v6 ground,12 other atlas frames,physical coordinates/footprints/collision/spawns/interactions/cameras/MAP representations/labels/places.'}
write(OUT/'source-metadata.json',source_metadata)
write(OUT/'resource-summary.json',{'version':7,'status':'PROPOSAL_NOT_RELEASE_BOUND','geometry_hash':geometry['geometry_hash'],'themes':results,'ground':read(BASE/'resource-summary.json')['ground']})
write(OUT/'manifest.json',{'format':'MCR_REFERENCE_SCENE_SPATIAL3_QUALITY7','version':7,'status':'CONTRACT_READY_CANDIDATE_NOT_RELEASE_BOUND','visual_acceptance':'IN_PROGRESS_NOT_ACCEPTED','geometry_revision':3,'geometry':geometry,'integer_projection':read(BASE/'manifest.json')['integer_projection'],'source_metadata':{'path':'source-metadata.json','sha256':sha(OUT/'source-metadata.json')},'sources':[{'path':name,'sha256':sha(OUT/name),'bytes':(OUT/name).stat().st_size} for name in ['sources/venue-sheet.png','sources/prompt.md']],'themes':[{'theme_id':era.upper(),'files':[{'path':era+'/'+name,'sha256':sha(OUT/era/name),'bytes':(OUT/era/name).stat().st_size} for name in ['scenery.json','layout.json','map-metadata.json','resource-inputs.json','provenance.json','atlas.png']]} for era in ERAS],'base_revision':'bc731ed868ec30b33ae5dd1e6f13e97e68ecabd9','base_manifest':{'path':'../quality-v6/manifest.json','sha256':sha(BASE/'manifest.json')},'adoption':'Source-only alternative for the exact spatial-v3 geometry. Does not change the published map4/v6 producer or any release binding; explicit new theme resource publication and matching consumer pins are required. No new Tiled or collision revision.','provenance_scope':'Existing virtual-concept place data unchanged. No physical source qualification or release approval claimed.'})
print(json.dumps(results))
