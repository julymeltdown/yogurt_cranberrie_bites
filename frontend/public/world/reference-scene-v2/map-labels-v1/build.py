#!/usr/bin/env python3
"""Original vector destination labels over the immutable v8 MAP composition."""
import copy
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont
from PIL import ImageFont

HERE = Path(__file__).resolve().parent
ART = HERE.parent
ROOT = ART.parents[3]
CLI = str(Path.home() / 'Library/Application Support/Tesseract/bin/tsrct')
SOURCE = ROOT / 'frontend/src/features/auth/ui/reference-sign-in/typography'
ERAS = ['parade', 'revenge', 'danger']
write = lambda p, v: p.write_text(json.dumps(v, indent=2) + '\n')
read = lambda p: json.loads(p.read_text())
sha = lambda p: 'sha256:' + hashlib.sha256(p.read_bytes()).hexdigest()
pins = lambda p: {'path': str(p.relative_to(ROOT)), 'bytes': p.stat().st_size, 'sha256': sha(p)}

# OFL source already present in the repository. No font download or package upgrade.
font_path = HERE / 'map-labels-bold.ttf'
font = instantiateVariableFont(TTFont(SOURCE / 'RobotoCondensed-variable.ttf'), {'wght': 800}, inplace=True)
names = {1: 'MCR Map Labels', 2: 'Bold', 3: 'MCR Map Labels Bold v1', 4: 'MCR Map Labels Bold', 6: 'MCRMapLabels-Bold', 16: 'MCR Map Labels', 17: 'Bold'}
for rec in font['name'].names:
    if rec.nameID in names:
        rec.string = names[rec.nameID].encode(rec.getEncoding())
font.recalcTimestamp = False
font.save(font_path)
shutil.copyfile(SOURCE / 'robotocondensed-OFL.txt', HERE / 'OFL.txt')

# Each glyph is authored here on its own 24x24 grid; polygon interiors are editable.
# Filled polygons avoid browser/native stroke-cap differences at mobile sizes.
rect = lambda x, y, w, h: [[x,y],[x+w,y],[x+w,y+h],[x,y+h]]
icons = {
    'stage': [rect(2,3,20,3), rect(3,6,3,13), rect(18,6,3,13), rect(2,19,20,3),
              [[6,6],[10,6],[8,12],[6,14]], [[14,6],[18,6],[18,14],[16,12]], rect(10,7,4,2)],
    'merch': [[[4,8],[20,8],[22,22],[2,22]], [[7,8],[7,3],[9,1],[15,1],[17,3],[17,8],[14,8],[14,4],[10,4],[10,8]]],
    'food': [rect(3,2,2,7),rect(7,2,2,7),rect(11,2,2,7),rect(3,8,10,3),rect(7,10,3,13),
             [[19,1],[22,4],[22,11],[20,13],[20,23],[17,23],[17,13],[15,11],[15,4]]],
    'people': [[[9,1],[15,1],[17,3],[17,7],[15,9],[9,9],[7,7],[7,3]],
               [[2,6],[5,6],[7,8],[7,12],[5,14],[2,14],[0,12],[0,8]],
               [[19,6],[22,6],[24,8],[24,12],[22,14],[19,14],[17,12],[17,8]],
               [[7,12],[17,12],[20,16],[20,23],[4,23],[4,16]], rect(0,16,3,7),rect(21,16,3,7)],
    'gift': [rect(2,9,20,4),rect(4,14,16,9),rect(11,9,3,14),
             [[11,9],[4,8],[2,5],[3,2],[7,1],[11,4],[13,9],[10,7],[7,4],[5,4],[5,6],[10,8]],
             [[13,9],[20,8],[22,5],[21,2],[17,1],[13,4],[11,9],[14,7],[17,4],[19,4],[19,6],[14,8]]],
}
cutouts = {'merch': [rect(5,11,14,8)]}
base_meta = read(ART / 'quality-v8/parade/map-metadata.json')
style = {'font_family': 'MCR Map Labels', 'font_style': 'Bold', 'font_size': 40, 'line_height': 42,
         'text_fill': '#211e20', 'paper_fill': '#f3ead8', 'border_fill': '#211e20', 'border_width': 3,
         'icon_fill': '#211e20', 'icon_paper': '#d9b49c', 'icon_size': 55, 'icon_cell': 70, 'padding': 14,
         'single_height': 78, 'double_height': 108, 'edge_margin': 5, 'text_metric_allowance': 4}
pilfont = ImageFont.truetype(str(font_path), style['font_size'])
labels = []
for source, icon, lines in zip(base_meta['map_labels'], ['stage',None,'merch','food','people','gift'],
                               [['MAIN STAGE'],['PLAZA'],['MERCH'],['FOOD'],['MEETUP','ZONE'],['GIVEAWAY','BOOTH']]):
    width = int(max(pilfont.getlength(line) for line in lines) + .999) + style['text_metric_allowance'] + 2 * style['padding'] + (style['icon_cell'] if icon else 0)
    height = style['double_height'] if len(lines)==2 else style['single_height']
    x = max(style['edge_margin'], min(768-width-style['edge_margin'], round(source['x']-width/2)))
    y = max(style['edge_margin'], min(1296-height-style['edge_margin'], round(source['y']-height/2)))
    labels.append({**copy.deepcopy(source), 'lines': lines, 'icon': icon, 'rect': {'x':x,'y':y,'width':width,'height':height},
                   'binding_policy': 'Existing interaction only' if source['interaction_id'] else 'Decorative, no hit target or invented place'})

presentation = {'version':1,'status':'CONTRACT_READY_SOURCE_PRESENTATION_NOT_RELEASE_BOUND',
    'geometry_hash':base_meta['geometry_hash'],'reference_scene_commit':'492160b15465818eca172f4017f3816d58c989a9',
    'dimensions':{'width':768,'height':1296},'view':'MAP_ONLY','style':style,'icons':icons,'icon_cutouts':cutouts,'labels':labels,
    'font':pins(font_path), 'font_license':pins(HERE/'OFL.txt'),
    'frozen_inputs':[pins(ART/'quality-v8/manifest.json'),pins(ART/'spatial-v4/integer-projection.json'),pins(ART/'spatial-v4/geometry-proposal.json')],
    'composition':'Draw labels after scenery in MAP coordinates; scale with the scene, not fixed CSS pixels. Do not bake them into ground/atlas. WORLD has no destination overlay. No pin marker without actual selected state.',
    'authority':'These vectors and labels add no place, destination, collision, route, physical eligibility, source-review state, interaction or release binding. Existing null bindings remain null.',
    'runtime_resources':'No additional PNG or changed v8 texture. This exact OFL font is separately pinned; import/reuse it deliberately in an assigned consumer patch.',
    'provenance':{'icons':'Original hand-authored filled polygons in build.py; no reference raster extraction or legacy icons.',
                  'font_source':pins(SOURCE/'RobotoCondensed-variable.ttf'),'font_source_license':pins(SOURCE/'robotocondensed-OFL.txt'),
                  'font_upstream':read(SOURCE/'sources.json')[0]['upstream'],
                  'font_transform':'fontTools instantiateVariableFont wght800; unique MCR Map Labels names; fixed source timestamps; no glyph edits.',
                  'label_text_anchors_bindings':'Exact immutable quality-v8 MAP metadata; visible rectangle centers clamp to5px scene edges, authored anchors unchanged.'}}
write(HERE/'presentation.json',presentation)

def rgba(color):
    return [int(color[i:i+2],16)/255 for i in (1,3,5)]+[1]

def primitives(label):
    r=label['rect']; x,y,w,h=(r[k] for k in ['x','y','width','height']); b=style['border_width']; items=[]
    items.append(('symmetric outer outline', rect(x,y,w,h),style['border_fill']))
    items.append(('cream label face',rect(x+b,y+b,w-2*b,h-2*b),style['paper_fill']))
    if label['icon']:
        ix=x+style['padding']; iy=y+(h-style['icon_size'])/2
        items.append(('semantic icon square',rect(ix-3,iy-3,style['icon_size']+6,style['icon_size']+6),style['icon_paper']))
        scale=style['icon_size']/24
        for n,poly in enumerate(icons[label['icon']]):
            items.append((f"{label['icon']} glyph part {n+1}",[[ix+px*scale,iy+py*scale] for px,py in poly],style['icon_fill']))
        # The bag interior is a genuine cutout, not a second icon or raster.
        for poly in cutouts.get(label['icon'],[]):
            items.append(('semantic icon cutout',[[ix+px*scale,iy+py*scale] for px,py in poly],style['icon_paper']))
    return items

def svg():
    parts=[]
    for label in labels:
        for name,poly,color in primitives(label):
            parts.append(f'<polygon data-part="{name}" points="'+ ' '.join(f'{x},{y}' for x,y in poly)+f'" fill="{color}"/>')
        r=label['rect']; tx=r['x']+style['padding']+(style['icon_cell'] if label['icon'] else 0)
        first_y=r['y']+(r['height']-len(label['lines'])*style['line_height'])/2
        for i,line in enumerate(label['lines']):
            parts.append(f'<text x="{tx}" y="{first_y+32+i*style["line_height"]}" fill="{style["text_fill"]}" font-size="{style["font_size"]}">{line}</text>')
    return '<svg xmlns="http://www.w3.org/2000/svg" width="768" height="1296"><style>@font-face{font-family:MapLabels;src:url(map-labels-bold.ttf)}text{font-family:MapLabels}</style>'+''.join(parts)+'</svg>'
(HERE/'labels.svg').write_text(svg()+'\n')

if '--native' in sys.argv:
    commands=[]
    def run(args):
        result=subprocess.run([CLI]+args,capture_output=True,text=True)
        commands.append({'command':['tsrct']+[a.replace(str(ROOT)+'/', '') for a in args], 'exit_code':result.returncode})
        if result.returncode: raise RuntimeError(result.stdout+result.stderr)
        if args[0:2]==['project','import-font']:
            return json.loads(result.stdout)
        return None
    for era in ERAS:
        project=HERE/f'{era}-map-labels.tsrct'; editable=HERE/f'{era}-map-labels-editable.json'
        if not project.exists():
            shutil.copyfile(ART/f'quality-v8/{era}/{era}-map.tsrct',project)
            imported=run(['project','import-font','--project',str(project),'--file',str(font_path)])
            imported['project']=str(project.relative_to(ROOT))
            write(HERE/f'{era}-font-import.json',imported)
        run(['project','checkout','--project',str(project),'--output',str(editable)])
        doc=read(editable)
        imported=read(HERE/f'{era}-font-import.json')
        # Import returns its actual family/style; do not guess from filenames.
        font_result=imported.get('fonts',[imported])[0]
        if 'fontFamily' not in font_result:
            print(json.dumps(imported,indent=2));raise RuntimeError('Inspect actual import result before text authoring')
        original_layers=read(ART/f'quality-v8/{era}/{era}-map-editable.json')['composition']['layers']
        layers=copy.deepcopy(original_layers); next_id=max(o['id'] for o in layers)+1; overlay=[]
        def common(name,kind):
            global next_id
            v={'id':next_id,'name':name,'type':kind,'blendMode':'normal','activeRange':{'start':0,'duration':3000},
               'transform':{'anchorPoint':[0,0],'position':[0,0],'scale':[100,100],'rotation':0,'opacity':100}}
            next_id+=1;return v
        for label in labels:
            for name,poly,color in primitives(label):
                l=common(label['label']+' / '+name,'Shape')
                l['shape']={'path':{'commands':[{'type':'moveTo' if i==0 else 'lineTo','x':x,'y':y} for i,(x,y) in enumerate(poly)]+[{'type':'close'}]},
                            'fills':[{'paint':{'type':'solid','color':rgba(color)},'fillRule':'nonZeroWinding','blendMode':'normal','opacity':100}]}
                overlay.append(l)
            r=label['rect']; tx=r['x']+style['padding']+(style['icon_cell'] if label['icon'] else 0)
            first_y=r['y']+(r['height']-len(label['lines'])*style['line_height'])/2
            for i,line in enumerate(label['lines']):
                l=common(label['label']+' / editable text '+str(i+1),'Text');l['transform']['position']=[tx,first_y+32+i*style['line_height']]
                l['sourceText']={'text':line,'fontFamily':font_result['fontFamily'],'fontStyle':font_result['fontStyle'],
                                 'fontSize':style['font_size'],'fillColor':rgba(style['text_fill']),'strokeWidth':0,'justification':'left'}
                overlay.append(l)
        doc['composition']['name']=era.upper()+' MAP with original editable destination labels'
        doc['composition']['layers']=list(reversed(overlay))+layers
        write(editable,doc)
        run(['project','commit','--project',str(project),'--file',str(editable)])
        run(['preview','--project',str(project),'--time','1','--output',str(HERE/f'{era}-map-labels-native.png')])
    write(HERE/'native-evidence.json',{'commands':commands,'dimensions':[768,1296],
          'description':'Fresh MAP variants from immutable v8 native project asset pairs; exact old scenery layers preserved with original native shape and text overlays. No changes to v8 projects.'})
print(json.dumps({'labels':len(labels),'icons':len(icons),'font_bytes':font_path.stat().st_size,'status':'SOURCE_AUTHORED'}))
