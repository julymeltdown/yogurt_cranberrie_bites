"""Read-only source compatibility audit; never renders or changes Root files."""
import argparse
import copy
import hashlib
import json
import pathlib
import re
import subprocess
import xml.etree.ElementTree as ET

parser = argparse.ArgumentParser()
parser.add_argument('--repository', required=True)
args = parser.parse_args()
repo = pathlib.Path(args.repository)
revision = '18e6e4db2f2e6384c796453d6e63caedc11875e8'
old_revision = '8972b0b634035055eb023dbc2f786e4463b172e5'
path = 'frontend/public/backgrounds/cute-wall-me-v1/wall-stationery.svg'
expected_sha = '22e2de672776682693bca71c57f501f2450bc9a28bc6314df44c26d6d193efc7'
css_path = 'frontend/src/shared/styles/cozy-community-backgrounds-v1.css'
css_sha = 'd26dd8fbee928539f2689ccd9bb6269e4cb964c58b78e8cc86afbd894f228504'

def blob(rev, name):
    return subprocess.check_output(['git', 'show', rev + ':' + name], cwd=repo)

def sha(data):
    return hashlib.sha256(data).hexdigest()

source = blob(revision, path)
old_source = blob(old_revision, path)
assert len(source) == 4500 and sha(source) == expected_sha
assert sha(old_source) == '1929fda18afe73000f79a44a7cb37c5504bee42d450bff343a562baa85e2e227'
svg = ET.fromstring(source)
old_svg = ET.fromstring(old_source)
assert svg.attrib == {'width': '390', 'height': '780', 'viewBox': '0 0 390 780', 'fill': 'none'}
allowed_tags = {'svg', 'title', 'desc', 'defs', 'pattern', 'circle', 'path', 'g', 'use', 'rect', 'ellipse'}
ids = {node.attrib['id'] for node in svg.iter() if 'id' in node.attrib}
local_references = []
for node in svg.iter():
    assert node.tag.rsplit('}', 1)[-1] in allowed_tags, node.tag
    for key, value in node.attrib.items():
        assert not key.lower().startswith('on'), key
        if key.rsplit('}', 1)[-1] == 'href':
            assert value.startswith('#') and value[1:] in ids
            local_references.append(value[1:])
        for ref in re.findall(r'url\(([^)]+)\)', value):
            assert ref.startswith('#') and ref[1:] in ids
            local_references.append(ref[1:])
        assert not any(token in value.lower() for token in ['https:', 'http:', 'data:', 'javascript:'])
assert b'<!DOCTYPE' not in source and b'<!ENTITY' not in source

# Ignore insignificant XML text/tail whitespace only. Original child order,
# attributes, geometry, paints and local definitions remain exactly comparable.
def signature(node):
    return [node.tag, dict(node.attrib), (node.text or '').strip(),
            [signature(child) for child in node]]

original_children = list(old_svg)
new_children = list(svg)
assert len(new_children) > len(original_children)
for index, old in enumerate(original_children):
    if old.tag.rsplit('}', 1)[-1] == 'desc':
        continue
    candidate = copy.deepcopy(new_children[index])
    if index in [3, 4]:
        assert old.attrib['d'] == 'M0 0h390v180H0z'
        assert candidate.attrib['d'] == 'M0 0h390v780H0z'
        candidate.attrib['d'] = old.attrib['d']
    assert signature(candidate) == signature(old), ('original top changed', index)
assert new_children[len(original_children)].attrib['d'].startswith('M390 258')

colors = sorted(set(re.findall(r'#[0-9a-fA-F]{6}', source.decode())))
expected_colors = '#c6b2d0,#c7d6c4,#c9a58b,#cbbad6,#ccd8c4,#d4c6dc,#d7c6dd,#dbbd99,#dca9a4,#dce3cd,#ddc7dd,#e4c9d9,#e4e9d8,#e5b1a8,#e5d8e8,#edc1bc,#f1d6c2,#f3c9bd,#f3e7ef,#f6d7d2,#f6ded4,#fbe5db,#fff0de,#fff8ec,#fffaf1'.split(',')
assert colors == expected_colors

def rgb(color):
    return [int(color[index:index + 2], 16) for index in [1, 3, 5]]

def luminance(color):
    values = [channel / 255 for channel in rgb(color)]
    linear = [v / 12.92 if v <= .04045 else ((v + .055) / 1.055) ** 2.4 for v in values]
    return sum(v * weight for v, weight in zip(linear, [.2126, .7152, .0722]))

def ratio(a, b):
    low, high = sorted([luminance(a), luminance(b)])
    return (high + .05) / (low + .05)

envelope = '#' + ''.join(f'{min(rgb(color)[index] for color in colors):02x}' for index in range(3))
contrast = {}
for label, foreground in [('closedDefaultBody', '#201d1d'), ('artistBodyAssumption', '#211d19'), ('closedRootFocusAssumption', '#a2232b')]:
    worst = min(colors, key=lambda background: ratio(foreground, background))
    contrast[label] = {'foregroundAssumption': foreground, 'minimumIndividualPaint': worst,
                       'minimumIndividualPaintRatio': round(ratio(foreground, worst), 3),
                       'conservativeEnvelope': envelope,
                       'minimumEnvelopeRatio': round(ratio(foreground, envelope), 3)}
assert contrast['closedDefaultBody']['minimumEnvelopeRatio'] >= 4.5
assert contrast['closedRootFocusAssumption']['minimumEnvelopeRatio'] >= 3

workspace = pathlib.Path(__file__).resolve().parents[4]
css = (workspace / css_path).read_bytes()
assert sha(css) == css_sha
rule = re.search(r":root \.shell-panel\[data-mcr-panel='wall'\] > \.shell-panel-body\s*\{([^}]+)\}", css.decode())
assert rule
actual = {key.strip(): value.strip() for key, value in re.findall(r'([\w-]+)\s*:\s*([^;]+);', rule.group(1))}
expected = {'background-color': '#fff0de', 'background-image': "url('/backgrounds/cute-wall-me-v1/wall-stationery.svg')",
            'background-position': 'center top', 'background-repeat': 'no-repeat', 'background-size': '100% auto'}
assert actual == expected

# Native-02 geometry is a frozen measurement used for arithmetic, not proof
# that this new source has actually rendered at these dimensions.
projection = []
for viewport, body_width in [(360, 342), (390, 372), (430, 412)]:
    scale = body_width / 390
    projection.append({'viewport': viewport, 'bodyWidthFromNative02': body_width,
                       'native02BodyTop': 57, 'native02BodyHeight': 780,
                       'projectedIntrinsicHeight': round(780 * scale, 3),
                       'lowerRibbonCenterY': round(57 + 569 * scale, 3),
                       'lowerRibbonCenterWithinNative02Body': 57 < 57 + 569 * scale < 837,
                       'newBroadFormsBeginY': round(57 + 258 * scale, 3)})
assert all(row['lowerRibbonCenterWithinNative02Body'] for row in projection)
print(json.dumps({'task': 'ART-SCREEN-COM01', 'kind': 'SOURCE_ONLY_NOT_RENDERED',
                  'sourceCommit': revision, 'assetPath': path,
                  'bytes': len(source), 'sha256': sha(source),
                  'dimensions': [390, 780], 'viewBox': '0 0 390 780',
                  'replacesOldSource': {'production': old_revision, 'sha256': sha(old_source),
                                        'dimensions': [390, 180], 'packet': '5186f11017185a040e4ccde1c59589e140563ef0'},
                  'topGeometryPaintsDefinitionsRetained': True,
                  'localReferences': sorted(set(local_references)), 'paintColors': colors,
                  'cssPath': css_path, 'cssSha256': css_sha, 'cssUnchanged': True,
                  'mapping': actual, 'sourceContrast': contrast,
                  'native02GeometryProjectionNotRuntime': projection,
                  'result': 'PASS_SOURCE_COMPATIBILITY_ONLY', 'commandExitCode': 0,
                  'limits': ['No browser, screenshot, image export or packet write was performed.',
                             'Native-02 report, image pins and original source audit remain frozen to the old 180px artwork.',
                             'Projection uses old measured geometry with unchanged CSS; fresh Root rendering must confirm actual fit, broad coverage and guard readability.',
                             'Foreground and focus vectors are source assumptions, not actual computed body/ring contrast.',
                             'Envelope bounds source paint mixtures; actual ancestor/filter/opacity/forced-color behavior remains unmeasured.',
                             'Fresh Wall390 then360/430, relevant accessibility, owner inspection and Root publication ancestry remain required.']}, indent=2))
