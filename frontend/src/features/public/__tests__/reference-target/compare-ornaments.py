"""Compare actual browser foreground silhouettes with the immutable reference.

This measures bounded decorative contours only; it cannot accept characters,
full-page color equality, Auth, real devices, or the whole product.
"""
import json
from pathlib import Path
from PIL import Image
import numpy as np

HERE = Path(__file__).resolve().parent
EVIDENCE = HERE / 'evidence/artwork'
source = np.array(Image.open(HERE / 'evidence/target-full.png').convert('RGB').crop((10, 35, 296, 520)))
actual = np.array(Image.open(EVIDENCE / 'foreground-390.png').convert('RGB').resize((286, 485), Image.Resampling.LANCZOS))
prior_image = Image.open(HERE / 'evidence/fidelity/poster-390.png').convert('RGB')
prior = np.array(prior_image.crop((4, 34, prior_image.width-4, prior_image.height-4)).resize((286, 485), Image.Resampling.LANCZOS))
y, x = np.indices(source.shape[:2]); x += 10; y += 35
regions = {
    'wordmark': ((x >= 40) & (x < 285) & (y >= 53) & (y < 219) & ~((x >= 164) & (y >= 187)), .88),
    'rose': ((x >= 17) & (x < 90) & (y >= 163) & (y < 269), .82),
    'moon_cemetery': ((x >= 17) & (x < 292) & (y >= 219) & (y < 285), .78),
}

def mask(rgb, region, name):
    f = rgb.astype(float)
    if name == 'rose':
        return region & (((f[:, :, 0] > 65) & (f[:, :, 0] > f[:, :, 1] * 1.65)) | ((f[:, :, 1] > 43) & (f[:, :, 1] > f[:, :, 0] * .84) & (f[:, :, 2] < f[:, :, 1] * .8)))
    return region & (f.mean(2) > 110)

report = {'qualification': 'Native routed foreground resampled once to the source coordinate grid, compared against original reference; bounded silhouette IoU, not whole-page acceptance.', 'sourceCrop': [10,35,296,520], 'cases': []}
for name, (region, minimum) in regions.items():
    expected = mask(source, region, name)
    def iou(image):
        observed = mask(image, region, name)
        return float((observed & expected).sum() / (observed | expected).sum())
    score, before = iou(actual), iou(prior)
    report['cases'].append({'region': name, 'priorIoU': before, 'deliveredIoU': score, 'minimum': minimum, 'pass': score >= minimum})
report['result'] = 'PASS' if all(c['pass'] for c in report['cases']) else 'FAIL'
(EVIDENCE / 'contour-comparison.json').write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps(report, indent=2))
assert report['result'] == 'PASS'
