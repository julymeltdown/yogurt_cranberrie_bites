"""Analytical ROI comparison only; never edits or replaces the product artwork."""
import hashlib
import json
import os
from pathlib import Path
from PIL import Image, ImageChops, ImageDraw, ImageStat

output = Path(os.environ['MCR_VENU_OUTPUT'])
report = json.loads((output / 'native-report.json').read_text())
assert report['status'] == 'PASS', 'Functional browser attempt must finish before comparison'
case = next(case for case in report['cases'] if (case['width'], case['height']) == (390, 844))
originals = Path('/tmp/codex-remote-attachments/01a1a385-9f32-73c2-8715-ff54bd1e0497/5AE18B10-B53F-405C-9CC2-1541ECB4BAFA')
references = [
    ('friends', '9-Photo-9.jpg', '2c9a8a240495467c9b03a0cce445515f3b879c6e2aef4bc0a366f1aa855051a9', (466, 438, 851, 518)),
    ('notes', '10-Photo-10.jpg', '1f0e133be3a4bf999fb7871188cce67086d09106a095c266b74e648a67777f63', (36, 593, 417, 724)),
    ('note-composer', '10-Photo-10.jpg', '1f0e133be3a4bf999fb7871188cce67086d09106a095c266b74e648a67777f63', (447, 378, 830, 748)),
]
comparisons, rows = [], []
for state, filename, digest, roi in references:
    source = originals / filename
    assert hashlib.sha256(source.read_bytes()).hexdigest() == digest
    capture = next(item for item in case['componentCaptures'] if item['state'] == state)
    candidate_path = output / capture['file']
    assert hashlib.sha256(candidate_path.read_bytes()).hexdigest() == capture['sha256']
    reference = Image.open(source).convert('RGB').crop(roi)
    candidate = Image.open(candidate_path).convert('RGB')
    # A single uniform width scale. Height differences are explicitly retained.
    scale = candidate.width / reference.width
    scaled = reference.resize((candidate.width, round(reference.height * scale)), Image.Resampling.LANCZOS)
    height = max(scaled.height, candidate.height)
    left = Image.new('RGB', (candidate.width, height), '#ff00ff')
    right = left.copy()
    left.paste(scaled, (0, 0)); right.paste(candidate, (0, 0))
    difference = ImageChops.difference(left, right)
    overlay = Image.blend(left, right, .5)
    files = {}
    for label, image in [('reference-crop', reference), ('reference-uniform-scale', left), ('candidate-padded', right), ('overlay', overlay), ('diff', difference)]:
        path = output / (state + '-' + label + '.png'); image.save(path)
        files[label] = dict(file=path.name, sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    row = Image.new('RGB', (candidate.width * 4, height + 30), 'white')
    draw = ImageDraw.Draw(row)
    for index, (label, image) in enumerate([('Reference', left), ('Candidate', right), ('50% overlay', overlay), ('Absolute RGB difference', difference)]):
        row.paste(image, (index * candidate.width, 30)); draw.text((index * candidate.width + 4, 8), state + ': ' + label, fill='black')
    rows.append(row)
    comparisons.append(dict(state=state, reference=dict(path=str(source), sha256=digest, roi=list(roi), bounds='left,top,right,bottom; source pixels', status='OWNER_SELECTED_BOUNDS_PENDING_CROP_INSPECTION'),
        candidate=capture, sourceCandidate=report['candidate'], uniformScale=scale, scaledReferenceHeight=scaled.height, candidateHeight=candidate.height,
        padding='Magenta explicitly marks unmatched height; no masks, warp, region suppression or pixel identity claim',
        meanAbsoluteRGB=ImageStat.Stat(difference).mean, pixelClaim='DESCRIPTIVE_DIFFERENCE_ONLY_NOT_A_VISUAL_PASS', files=files,
        ownerDisposition='PENDING_PERSONAL_INSPECTION_AND_MATERIAL_DIFFERENCE_ITERATION'))
sheet = Image.new('RGB', (max(row.width for row in rows), sum(row.height for row in rows)), '#dddddd')
top = 0
for row in rows:
    sheet.paste(row, (0, top)); top += row.height
sheet.save(output / 'reference-comparison-contact-sheet.png')
(output / 'reference-comparisons.json').write_text(json.dumps(dict(candidate=report['candidate'], lease=report['lease'], qualification='CONTROLLED_APP_COMPONENT_COMPARISONS_PENDING_OWNER_INSPECTION', comparisons=comparisons), indent=2) + '\n')
print(json.dumps(dict(comparisons=len(comparisons), ownerInspection='PENDING', masking='NONE', scale='UNIFORM_ONLY')))
