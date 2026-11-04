"""Assess full-panel foreground registration without changing supplied artwork.

Source dimensions and the original declared panel bounds supply a reproducible
mapping onto the NEW JPEG. Unlike pass one, this excludes the inter-panel gutter
and preserves aspect ratio. Any proposed title warp is analytical only; a real
browser capture remains necessary before qualifying a CSS change.
"""
from pathlib import Path
from PIL import Image, ImageFilter, ImageDraw
import numpy as np
import json
import sys

HERE = Path(__file__).resolve().parent
OUT = HERE / 'round-02'
OUT.mkdir(exist_ok=True)
original = Image.open(HERE / 'reference-original.jpg').convert('RGB')
source_size = (1536, 1024)
panel = (6, 4, 296, 524)  # Previously declared original-source panel, x/y/w/h.
sx, sy = original.width / source_size[0], original.height / source_size[1]
box = (panel[0]*sx, panel[1]*sy, (panel[0]+panel[2])*sx, (panel[1]+panel[3])*sy)
baseline = Image.open(HERE / 'candidate-poster-390.png').convert('RGB')
scale = baseline.width / (box[2]-box[0])
target_size = (baseline.width, round((box[3]-box[1])*scale))
reference = original.resize(target_size, Image.Resampling.LANCZOS, box=box)
canvas = Image.new('RGB', baseline.size, '#161413')
canvas.paste(reference, (0, 0))
reference = canvas

regions = {
    'wordmark': (35, 55, 365, 280),
    'copy': (70, 490, 310, 583),
    'cta': (20, 585, 355, 645),
}

def cream(image):
    rgb = np.asarray(image)
    return (rgb[:,:,0] > 165) & (rgb[:,:,1] > 150) & (rgb[:,:,2] > 120)

def restricted(mask, label):
    region = np.zeros(mask.shape, bool)
    left, top, right, bottom = regions[label]
    region[top:bottom,left:right] = True
    if label == 'wordmark':
        # Same source-space moon exclusion as the existing contour qualification.
        left = round((164-panel[0])*baseline.width/panel[2])
        top = round((187-panel[1])*target_size[1]/panel[3])
        region[top:,left:] = False
    return mask & region

def compare(image):
    result = {}
    for label in regions:
        expected = restricted(cream(reference), label)
        observed = restricted(cream(image), label)
        ey, ex = np.where(expected); oy, ox = np.where(observed)
        bounds = lambda x, y: [int(x.min()), int(y.min()), int(x.max()), int(y.max())]
        a, b = bounds(ex,ey), bounds(ox,oy)
        result[label] = {
            'referenceBounds': a, 'observedBounds': b,
            'edgeError': sum(abs(x-y) for x,y in zip(a,b)),
            'maskIoU': float((expected & observed).sum()/(expected | observed).sum()),
        }
    return result

report = {
    'qualification': 'Full-panel geometric registration and coarse cream masks; excludes artwork/background/character acceptance. No production change is certified by an analytical warp.',
    'newPhotoSize': original.size, 'originalSourceSize': source_size,
    'originalPanelXYWH': panel, 'newPhotoPanelLTRB': box,
    'oldFirstPassCropLTRB': [5,3,254,440],
    'oldExtraGutterColumns': 249-(box[2]-box[0]),
    'uniformTargetSize': target_size,
    'firstPass': compare(baseline),
}
# Source-coordinate prediction: fit the existing (10,35,286,485) SVG viewBox
# to its original panel coordinates, without changing paths or gallery geometry.
body_width, body_height = 366, 629.28125
new_left = (10-panel[0])*baseline.width/panel[2]
new_top = (35-panel[1])*target_size[1]/panel[3]
new_width = 286*baseline.width/panel[2]
new_height = 485*target_size[1]/panel[3]
proposal = {
    'leftPercent': (new_left-6)/body_width*100,
    'topPercent': (new_top-34)/body_height*100,
    'widthPercent': new_width/body_width*100,
    'heightPercent': new_height/body_height*100,
}
report['sourceCoordinateWordmarkProposal'] = proposal
final = '--final' in sys.argv
candidate_file = OUT/('final/candidate-poster-390.png' if final else 'candidate-poster-390.png')
if candidate_file.exists():
    candidate = Image.open(candidate_file).convert('RGB')
    report['nativeCandidate'] = compare(candidate)
    images = [reference, baseline, candidate]
    labels = ['NEW photo / corrected panel', 'Published first pass', 'Actual revised component']
else:
    images = [reference, baseline]
    labels = ['NEW photo / corrected panel', 'Published first pass']
comparison = Image.new('RGB',(baseline.width*len(images),baseline.height+28),'white')
draw = ImageDraw.Draw(comparison)
for i,(image,label) in enumerate(zip(images,labels)):
    comparison.paste(image,(i*baseline.width,28));draw.text((i*baseline.width+5,8),label,fill='black')
comparison.save(OUT/('comparison-final-390.png' if final else 'comparison-candidate-390.png' if candidate_file.exists() else 'comparison-assessment-390.png'))
(OUT/('residual-assessment-final.json' if final else 'residual-assessment.json')).write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
