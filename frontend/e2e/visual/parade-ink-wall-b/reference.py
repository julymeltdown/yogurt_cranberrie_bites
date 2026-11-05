"""Analytical reference crops only; never a product rendering substitute."""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image, ImageChops, ImageStat, ImageDraw, ImageFont
p=argparse.ArgumentParser()
p.add_argument('--output',type=Path,required=True)
p.add_argument('--captures',type=Path)
a=p.parse_args()
repo=Path(__file__).resolve().parents[4]
source=repo/'docs/mcr/design/parade-ink-2026-11-05/parade-ink.jpg'
assert hashlib.sha256(source.read_bytes()).hexdigest()=='9e7f17a666e1ecfc738a14b75bb3a166235a05c401f206aa24f26cf7463cbc27'
assert str(a.output.resolve()).startswith(str(repo/'docs/mcr/evidence/parade-ink-wall-b')+'/')
a.output.mkdir(parents=True,exist_ok=True)
rect=[849,280,1256,454]
anchors={'component':rect,'printedViewport':[862,291,1036,443],'pencil':[1050,291,1093,335],'erase':[1050,339,1093,383],'fill':[1050,389,1093,433],'palette':[1105,292,1248,389],'primaryAction':[1105,399,1248,438]}
image=Image.open(source).convert('RGB');crop=image.crop(tuple(rect))
result={'sourceSha256':hashlib.sha256(source.read_bytes()).hexdigest(),'convention':'exclusive right/bottom source pixels; manually bounded visible ink, +/-1 source pixel','sourceAnchors':anchors,'normalizations':[],'adaptations':['Printed viewport is174x152; real128x128 raster uses uniform square fit without changing protected mapping.','Separate44px targets require a two-row horizontally scrolling16-color palette and reachable secondary tools.','The true existing explicit confirm and original predicates replace the sample SAVE wording; no dummy action.']}
for width in [360,390,430]:
 scale=width/(rect[2]-rect[0]);size=(width,round(crop.height*scale));out=a.output/('photo2-wall-'+str(width)+'.png');crop.resize(size,Image.Resampling.LANCZOS).save(out)
 result['normalizations'].append({'width':width,'scale':scale,'dimensions':size,'sha256':hashlib.sha256(out.read_bytes()).hexdigest()})
(a.output/'reference-measurements.json').write_text(json.dumps(result,indent=2)+'\n')

if a.captures:
 captures=json.loads(a.captures.read_text());comparisons=[]
 try:font=ImageFont.load_default(size=14)
 except TypeError:font=ImageFont.load_default()
 for case in captures['cases']:
  bounds=case['workbench']['bounds'];image=Image.open(a.captures.parent/case['screenshot']['path']).convert('RGB')
  box=(round(bounds['x']),round(bounds['y']),round(bounds['right']),round(bounds['bottom']))
  actual=image.crop(box);reference=crop.resize((actual.width,round(crop.height*actual.width/crop.width)),Image.Resampling.LANCZOS)
  high=max(actual.height,reference.height);left=Image.new('RGB',(actual.width,high),'white');right=left.copy();left.paste(reference);right.paste(actual)
  difference=ImageChops.difference(left,right);pair=Image.new('RGB',(actual.width*2+8,high+24),'white');pair.paste(left,(0,24));pair.paste(right,(actual.width+8,24));draw=ImageDraw.Draw(pair);draw.text((2,2),'Photo2 reference',font=font,fill='black');draw.text((actual.width+10,2),'Actual App composite',font=font,fill='black')
  stem='wall-'+str(case['width']);actual.save(a.output/(stem+'-actual-workbench.png'));pair.save(a.output/(stem+'-comparison.png'));difference.save(a.output/(stem+'-raw-diff.png'))
  comparisons.append({'viewportWidth':case['width'],'actualBounds':bounds,'uniformReferenceScale':actual.width/crop.width,'referenceDimensions':reference.size,'actualDimensions':actual.size,'rawMeanAbsoluteRGB':ImageStat.Stat(difference).mean,'status':'DIAGNOSTIC_RAW_NOT_ACCEPTANCE','masking':'NONE; actual mural and server palette values vary; structural bounds remain visible.','actualGeometry':case['workbench']})
 (a.output/'comparison.json').write_text(json.dumps({'reference':result,'comparisons':comparisons,'qualification':'Exact source/native/composite and functional adaptations require owner inspection after resource FULL; raw diff is not a pass.'},indent=2)+'\n')
