"""Read-only asset/provenance and printed-paper measurements; never edits raster data."""
from pathlib import Path
import hashlib
import json
from PIL import Image, ImageStat
from fontTools.ttLib import TTFont
import fontTools

root = Path(__file__).resolve().parents[8]
source = root / 'frontend/src/features/auth/ui/reference-sign-in'
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
provenance = json.loads((source / 'artwork/provenance.json').read_text())
assert sha(root / provenance['reference']['path']) == provenance['reference']['sha256']
assets = []
for entry in provenance['rasterAssets']:
    path = source / entry['path']
    assert sha(path) == entry['sha256']
    image = Image.open(path)
    assert image.mode == 'RGBA' and image.size == (2172, 724)
    alpha = image.getchannel('A')
    assert alpha.getextrema() == (0, 255)
    # The upper central footer must remain genuinely transparent above the grave silhouette.
    if 'bottom' in path.name:
        region = alpha.crop((435, 0, 1737, 400))
        assert ImageStat.Stat(region).mean[0] < .1
    else:
        region = alpha.crop((650, 550, 1520, 724))
        assert ImageStat.Stat(region).mean[0] < 1
    assets.append({'path': entry['path'], 'sha256': sha(path), 'transparentContentClearance': 'PASS'})
font_sources = json.loads((source / 'typography/sources.json').read_text())
for entry in font_sources:
    assert sha(source / entry['path']) == entry['sha256']
    assert sha(source / entry['license']) == entry['licenseSha256']
    assert sha(source / entry['derivative']['path']) == entry['derivative']['sha256']
font = TTFont(source / 'typography/mcr-auth-print.woff')
assert font['name'].getDebugName(1) == 'MCR Auth Print'
assert font['OS/2'].fsType == 0
for text in ['SIGN IN', 'EMAIL', 'PASSWORD', 'FORGOT PASSWORD?', 'OR CONTINUE WITH', 'NO ACCOUNT? CREATE AN ACCOUNT', 'You’re signed in']:
    assert all(ord(c) in font.getBestCmap() for c in text)
# Explicitly unobstructed paper regions, excluding fields, letters, checkbox and scenery.
def paper(path, box):
    stat = ImageStat.Stat(Image.open(path).convert('L').crop(box))
    return {'mean': stat.mean[0], 'standardDeviation': stat.stddev[0]}
target = paper(root / provenance['reference']['path'], (380, 267, 525, 279))
actual = paper(root / 'frontend/src/features/auth/__tests__/reference-target/fidelity/evidence/login-390.png', (90, 383, 300, 400))
assert abs(target['mean'] - actual['mean']) < 8
assert .5 < actual['standardDeviation'] / target['standardDeviation'] < 2
print(json.dumps({'status': 'PASS', 'referenceSha256': provenance['reference']['sha256'], 'assets': assets, 'font': {'family': 'MCR Auth Print', 'coverage': 'PASS', 'fontTools': fontTools.__version__, 'sha256': sha(source / 'typography/mcr-auth-print.woff')}, 'paperTexture': {'target': target, 'actual': actual, 'scope': 'Regional paper tone/grain only; not a pixel-identity or visual acceptance oracle'}, 'visualRequirement': 'Developer inspection of native comparisons is required separately; static metrics do not certify composition or artwork alone.'}, indent=2))
