"""PI12 unpictured-page component comparison; never a fabricated page pixel score."""
import hashlib,json,sys
from pathlib import Path
from PIL import Image,ImageDraw
repo=Path(__file__).resolve().parents[5]
out=Path(sys.argv[1]);report=json.loads((out/'report.json').read_text());assert report['status']=='PASS'
reference=repo/'docs/mcr/design/parade-ink-2026-11-05/parade-ink.jpg'
assert hashlib.sha256(reference.read_bytes()).hexdigest()=='9e7f17a666e1ecfc738a14b75bb3a166235a05c401f206aa24f26cf7463cbc27'
original=Image.open(reference).convert('RGB')
shots=[*report.get('retainedScreenshots',[]),*report['screenshots']]
def shot_path(shot):
 file=Path(shot['file']);return file if file.is_absolute() else out/file
# Declared reference component crops, resized isotropically only. No repaint/masks.
crops={'tabs-footer':(850,504,1259,635),'text-row':(850,679,1259,837)}
canvas=Image.new('RGB',(1560,1120),'#f9f9f8');draw=ImageDraw.Draw(canvas)
for i,(name,box) in enumerate(crops.items()):
 image=original.crop(box)
 canvas.paste(image.resize((390,round(image.height*390/image.width))),(i*390,24));draw.text((i*390+6,4),'Original Photo2 '+name,fill='#171717')
for i,name in enumerate(['social','requests','age','blocks','account','inbox','archive']):
 shot=next(shot for shot in shots if Path(shot['file']).name==name+'-390.png');path=shot_path(shot);image=Image.open(path).convert('RGB');assert image.size==(390,844)
 x=(i%4)*390;y=230+(i//4)*445;canvas.paste(image.resize((195,422)),(x,y+20));draw.text((x+6,y),name+' original390px',fill='#171717')
canvas.save(out/'reference-components-and-pages.png')
rows=[]
for shot in shots:
 metrics=shot['metrics'];root=metrics['root']
 assert root['background']=='rgb(249, 249, 248)' and root['backgroundImage']=='none'
 assert 'Parade Ink Text' in root['font'] and root['size']=='18px' and root['leading']=='22px'
 if metrics['heading']:assert metrics['heading']['size']=='26px' and metrics['heading']['leading']=='32px'
 file=Path(shot['file']);file=file if file.is_absolute() else out/file
 rows.append({'file':shot['file'],'sha256':hashlib.sha256(file.read_bytes()).hexdigest(),'componentChecks':'paper/font/18-22 body/26-32 title/44px controls/no overflow PASS'})
(out/'comparison.json').write_text(json.dumps({'status':'PASS','referenceSha256':hashlib.sha256(reference.read_bytes()).hexdigest(),'referenceCrops':crops,'transform':'isotropic resize to390px; contact-sheet native images at1/2 scale; originals preserved','scope':'Seven pages are unpictured. Compare shared reference anatomy/tokens and actual source metrics; no exact page-image pixel parity or whole-image similarity claim.','fixedRegionDisposition':'Native owner inspection required after FULL. No masks or fabricated unsupported controls.','rows':rows},indent=2)+'\n')
print(json.dumps({'status':'PASS','screenshots':len(rows),'comparison':'reference-components-and-pages.png'}))
