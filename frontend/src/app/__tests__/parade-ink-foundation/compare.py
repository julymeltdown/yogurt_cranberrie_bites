"""Owner-side fixed-region analysis after an admitted native round; no acceptance score."""
import hashlib
import json
from pathlib import Path
import sys
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parents[5]
evidence = root / 'docs/mcr/evidence/parade-ink-foundation'
round_path = Path(sys.argv[1]).resolve()
assert round_path.is_relative_to(evidence)
report = json.loads((round_path / 'native.json').read_text())
measurements = json.loads((evidence / 'reference-measurements.json').read_text())
reference = Image.open(evidence / 'feed-frame-reference-390.png').convert('RGB')
actual = Image.open(round_path / 'app-feed-390.png').convert('RGB')
# Preserve both originals and native scale; pad only outside each displayed image.
comparison = Image.new('RGB', (800, max(reference.height, actual.height) + 30), '#ffffff')
draw = ImageDraw.Draw(comparison)
draw.text((4, 8), 'PHOTO 2 REFERENCE / normalized 390', fill='#171717')
draw.text((404, 8), 'ACTUAL APP / 390 / deterministic test data', fill='#171717')
comparison.paste(reference, (0, 30)); comparison.paste(actual, (400, 30))
comparison.save(round_path / 'reference-app-390.png')
scale = measurements['primaryNormalize']['scale']
anchors = measurements['sourceAnchors']
base_x, base_y = anchors['outerFrame'][:2]
normalized = {name: {'x':(a[0]-base_x)*scale,'y':(a[1]-base_y)*scale,'width':(a[2]-a[0])*scale,'height':(a[3]-a[1])*scale} for name,a in anchors.items()}
app = next(case for case in report['cases'] if case['width'] == 390)['metrics']
comparison_data = {
 'qualification':'Fixed-region geometry and owner image analysis, not a visual acceptance score',
 'reference':measurements['reference'],
 'nativeReportSha256':hashlib.sha256((round_path/'native.json').read_bytes()).hexdigest(),
 'normalizedReference':normalized,
 'actual390':app,
 'headerDeltaCssPx':{key:app['header'][key]-normalized['header'][key] for key in ['x','y','width','height']},
 'referenceViewportHeight':reference.height,'actualViewportHeight':actual.height,
 'declaredAdaptations':measurements['declaredAdaptations'],
 'openMismatchList':[
  'Fixture post count/text/time/media and authorized portrait availability differ from the reference; no private or invented product content is substituted.',
  'Source JPEG has no editable font metadata. Oswald/Roboto are measured choices; text rasterization and glyph shapes may differ.',
  'The reference frame normalizes to about777px high; the actual844px viewport keeps fixed insets and anchors controls to the available height.',
  'Three separate44px window targets and44px tabs expand the smaller printed control spacing and tab height.',
  'World screenshots use explicitly synthetic geometric test terrain; they do not certify World artwork parity or real service/device behavior.'
 ],
 'ownerImageInspection':'PENDING actual image inspection; this script does not mark acceptance'
}
(round_path/'comparison.json').write_text(json.dumps(comparison_data,indent=2)+'\n')
print(json.dumps({'headerDeltaCssPx':comparison_data['headerDeltaCssPx'],'comparisonImage':str(round_path/'reference-app-390.png')}))
