"""Read retained Git pixels and calculate the TITLE-FOCUS-01 constraint proof.

No browser, render, source mutation, artifact copies, or dependencies installed.
Run only in the coordinator's allocated source-check slot.
"""
import hashlib
from io import BytesIO
import json
from pathlib import Path
import subprocess

from PIL import Image

ROOT = Path(__file__).resolve().parents[4]
BASE = '9e7952fcd89360f7b4fe43c69f22060b9aa55c74'
CSS = 'frontend/src/app/shell-chrome/shell-chrome.css'
CSS_SHA = '08614a9b3263d7cbb27af4d59aaabc0ee4db07943812faa0e04bdba7f720291d'
MAP_PACKET = '4600542ba8f5d16210075c93847fa60960809540'
ME_PACKET = '5186f11017185a040e4ccde1c59589e140563ef0'
MAP_PREFIX = 'docs/mcr/evidence/reference-scene-v5/map-cozy-v1/native-03/'
ME_PREFIX = 'docs/mcr/evidence/cozy-backgrounds-v1/native-02/'
RING = (162, 35, 43)
TEXT = (242, 234, 223)
DARK = (37, 16, 23)
LIGHT = (233, 225, 212)


def git_bytes(revision, path):
    return subprocess.check_output(['git', 'show', f'{revision}:{path}'], cwd=ROOT)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def luminance(rgb):
    channels = [x / 255 for x in rgb]
    linear = [x / 12.92 if x <= .04045 else ((x + .055) / 1.055) ** 2.4 for x in channels]
    return sum(x * w for x, w in zip(linear, (.2126, .7152, .0722)))


def contrast(a, b):
    low, high = sorted((luminance(a), luminance(b)))
    return (high + .05) / (low + .05)


assert sha(git_bytes(BASE, CSS)) == CSS_SHA
assert (ROOT / CSS).read_bytes() == git_bytes(BASE, CSS), 'Application CSS must remain unchanged'
assert git_bytes('8972b0b634035055eb023dbc2f786e4463b172e5', CSS) == git_bytes(BASE, CSS)
ring_l, text_l = luminance(RING), luminance(TEXT)
light_minimum = 3 * (ring_l + .05) - .05
text_maximum = (text_l + .05) / 4.5 - .05
assert (ring_l + .05) / .05 < 3, 'A darker backing cannot meet the ring requirement'
assert light_minimum > text_maximum, 'Same-pixel backing intervals must be disjoint'

raw_report = git_bytes(MAP_PACKET, MAP_PREFIX + 'report.json')
report = json.loads(raw_report)
map_cases = []
for case in report['cases']:
    if case['mode'] != 'candidate':
        continue
    width = case['width']
    pin = next(s for s in case['screens'] if s['id'] == 'map')
    data = git_bytes(MAP_PACKET, MAP_PREFIX + pin['path'])
    assert len(data) == pin['bytes'] and sha(data) == pin['sha256']
    image = Image.open(BytesIO(data)).convert('RGB')
    ring_pixels, glyph_pixels = [], []
    for y in range(21, 44):
        for x in range(20, width - 146):
            rgb = image.getpixel((x, y))
            if rgb == RING:
                ring_pixels.append((x, y))
            elif rgb != DARK:
                glyph_pixels.append((x, y))
    assert image.getpixel((21, 24)) == RING
    assert len(glyph_pixels) == 556

    # A deliberately rejected snapshot-only spatial model. This symmetric frame
    # separates the ring from the glyphs still visible while the outline covers
    # the first glyph columns. It cannot preserve legitimate unfocused text.
    def snapshot_backing(x, y):
        rx, ry, padding_width = x - 12, y - 10, width - 24
        if rx >= padding_width - 134 or ry < 11 or ry >= 34:
            return DARK
        if ry < 14 or ry >= 31:
            return LIGHT
        if 8 <= rx < 11 or padding_width - 137 <= rx < padding_width - 134:
            return LIGHT
        return DARK

    assert all(snapshot_backing(x, y) == LIGHT for x, y in ring_pixels)
    assert all(snapshot_backing(x, y) == DARK for x, y in glyph_pixels)
    map_cases.append({'era': case['era'], 'width': width, 'path': MAP_PREFIX + pin['path'],
                      'sha256': pin['sha256'], 'bytes': pin['bytes'],
                      'ringPixels': len(ring_pixels), 'visibleGlyphPixelsIncludingAA': len(glyph_pixels),
                      'pixel21_24': RING, 'snapshotOnlyFrameFitsVisiblePixels': True})
assert len(map_cases) == 9

me_name = 'candidate-me-focus-0-390.png'
me_report = json.loads(git_bytes(ME_PACKET, ME_PREFIX + 'report.json'))
me_case = next(c for c in me_report['cases'] if c['id'] == 'me' and c['mode'] == 'candidate' and c['width'] == 390)
me_pin = me_case['meIconFocus'][0]
assert me_pin['file'] == me_name and me_pin['label'] == 'Edit profile'
me_data = git_bytes(ME_PACKET, ME_PREFIX + me_name)
assert len(me_data) == me_pin['bytes'] and sha(me_data) == me_pin['sha256']
me_image = Image.open(BytesIO(me_data)).convert('RGB')
assert me_image.getpixel((21, 24)) == TEXT
assert me_image.getpixel((22, 24)) == TEXT
assert all(me_image.getpixel((21, y)) == TEXT for y in range(24, 41))

result = {
    'task': 'TITLE-FOCUS-01', 'disposition': 'NO_PATCH_UNDER_CURRENT_CONSTRAINTS',
    'implementationStatus': 'NOT_STARTED',
    'base': BASE, 'checkout': str(ROOT), 'branch': 'codex/mcr-title-focus-01',
    'cssPath': CSS, 'unchangedCssSha256': CSS_SHA,
    'grantedProperties': ['background-color', 'background-image', 'background-size'],
    'frozenRingRGB': RING, 'frozenTextRGB': TEXT,
    'ringLuminance': ring_l, 'textLuminance': text_l,
    'ringMaximumAgainstBlack': (ring_l + .05) / .05,
    'ring3RequiresBackingLuminanceAtLeast': light_minimum,
    'text4_5RequiresBackingLuminanceAtMost': text_maximum,
    'intervalsDisjoint': True,
    'currentRingOnDarkContrast': contrast(RING, DARK),
    'currentCreamOnDarkContrast': contrast(TEXT, DARK),
    'rejectedSnapshotFrameRingOnPaperContrast': contrast(RING, LIGHT),
    'mapPacketCommit': MAP_PACKET, 'mapRawReportSha256': sha(raw_report),
    'nineActualFocusedMapCases': map_cases,
    'unfocusedSharedTitleCounterexample': {
        'packetCommit': ME_PACKET, 'path': ME_PREFIX + me_name,
        'bytes': len(me_data), 'sha256': sha(me_data),
        'state': 'Edit profile link focused; shared Me panel title no longer focused',
        'actualCreamCoreAt': [[21, 24], [22, 24]], 'rgb': TEXT,
        'creamCoreAtX21ForAllRows24Through40': True,
        'sharedPanelHeaderSource': 'frontend/src/app/shell-chrome/index.tsx',
        'sameFrozenHeaderCssAt8972AndBase': True
    },
    'proof': 'At the same shared-header coordinate (21,24), legitimate focused Map requires backing L>=0.3717359127, while actual unfocused Me cream glyph requires L<=0.1456679906. The allowed background properties have no focus-state-dependent branch. A static spatial paint cannot satisfy both. One valid 390px counterexample disproves an all-state/all-width shared-rule fix.',
    'applicationPatchAuthored': False,
    'limits': [
        'Source/pixel feasibility calculation only; no new browser/render/native/build output or design acceptance',
        'The nine retained Map screenshots cover three eras at360/390/430; unfocused glyph counterexample is an actual390 Me capture',
        'Does not claim an actual unfocused Map capture or unfocused360/430 captures',
        'Pixel frame is a rejected analytical model, not generated product artwork or a proposed implementation',
        'A product decision changing the allowed state/foreground/geometry constraints is needed before a compliant implementation can proceed'
    ]
}
(Path(__file__).parent / 'feasibility.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'task': result['task'], 'disposition': result['disposition'],
                  'nineMapPinsAndPixels': 'PASS', 'actualUnfocusedCoreCounterexample': 'PASS',
                  'cssUnchanged': True, 'disjointBackingIntervals': [text_maximum, light_minimum]}))
