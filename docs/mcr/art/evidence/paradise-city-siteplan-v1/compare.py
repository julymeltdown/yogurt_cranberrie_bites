"""Diagnostic source/asset pixel comparison and candidate-anchor annotation.

The product JPEG is read-only. Only separate evidence PNG/JSON files are written.
No art crop, repaint, rescale, palette change, mask or screenshot substitution.
"""
from pathlib import Path
import argparse
import hashlib
import json
from PIL import Image, ImageChops, ImageDraw

ROOT=Path(__file__).resolve().parents[5]
PACK=ROOT/'frontend/public/venue/paradise-city/v1'
EXPECTED='d0713633289f87fc98603927f7cbc02df2054e989f6360332a93476d6c9da4e0'
parser=argparse.ArgumentParser();parser.add_argument('--reference-root',required=True);parser.add_argument('--offer',required=True);parser.add_argument('--attempt',required=True,type=int);args=parser.parse_args()
assert Path(args.offer).name==args.offer and args.attempt>0
source=Path(args.reference_root)/'5-Photo-5.jpg';asset=PACK/'siteplan-photo-5.jpg';raw=source.read_bytes();actual=asset.read_bytes()
assert len(raw)==328720 and hashlib.sha256(raw).hexdigest()==EXPECTED and raw==actual
with Image.open(source) as im:im.load();reference=im.convert('RGB');source_format=im.format
with Image.open(asset) as im:im.load();packaged=im.convert('RGB');asset_format=im.format
assert reference.size==packaged.size==(1024,1280) and source_format==asset_format=='JPEG'
# ROI is measured from both decoded source dimensions, not an exploratory UI box.
roi=[0,0,reference.width,reference.height]
difference=ImageChops.difference(reference,packaged);assert difference.getbbox() is None
interface=json.loads((PACK/'asset-interface.json').read_text())
annotation=Image.new('RGBA',packaged.size,(0,0,0,0));draw=ImageDraw.Draw(annotation)
for item in interface['landmarks']:
    r=item['hitRegion'];x,y,w,h=[r[k] for k in ('x','y','width','height')]
    draw.rectangle((x,y,x+w-1,y+h-1),outline='#00e5ff',width=2)
    px,py=item['anchor']['x'],item['anchor']['y'];draw.ellipse((px-4,py-4,px+4,py+4),fill='#ff295c',outline='#ffffff',width=1)
    draw.text((x+2,y+2),item['id'],fill='#000000',stroke_width=2,stroke_fill='#ffffff')
overlay=packaged.convert('RGBA');overlay.alpha_composite(annotation);overlay=overlay.convert('RGB')
base=Path(__file__).resolve().parent/(args.offer+'-attempt-'+str(args.attempt))
outputs=[]
for suffix,im in [('-anchors.png',overlay),('-anchor-layer.png',annotation),('-unmasked-rgb-diff.png',difference)]:
    path=Path(str(base)+suffix);assert not path.exists();im.save(path,format='PNG',compress_level=6)
    b=path.read_bytes();outputs.append(dict(path=str(path.relative_to(ROOT)),bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
assert sum(p['bytes'] for p in outputs)<=4*1024**2
result=dict(status='PASS',scope='SOURCE_ASSET_AND_PRESENTATION_ANCHORS_ONLY',sourcePhoto='5-Photo-5.jpg',sourceSHA256=EXPECTED,assetSHA256=hashlib.sha256(actual).hexdigest(),sourceROI=roi,assetROI=roi,viewportCSS=[1024,1280],devicePixelRatio=1,uniformScale=1,comparison='UNMASKED_RGB_EXACT',rgbDifferencePixels=0,originalJPEGBytesExact=True,candidates=len(interface['landmarks']),outputs=outputs,annotationIsSeparateTransparentRGBA=True,notAppCapture=True,notGeographicCalibration=True)
Path(str(base)+'-comparison.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
