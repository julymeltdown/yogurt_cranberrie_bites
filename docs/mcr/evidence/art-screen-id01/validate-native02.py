"""Audit the existing immutable native02 packet; no browser or artifact copies."""
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess
import sys

from PIL import Image

ROOT = Path(__file__).resolve().parents[4]
PACKET = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(
    '/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish/docs/mcr/evidence/cozy-backgrounds-v1/native-02')
ARTIFACT = '5186f11017185a040e4ccde1c59589e140563ef0'
CANDIDATE = '8972b0b634035055eb023dbc2f786e4463b172e5'
REPORT_HASH = '7f34e10279212d5f5d01017cbd644f02a33bd8939393b30818326ab2a489395b'
ATTESTATION = '2579ac15cb1d934de3f7a081cc7f2fb96498b5ac'
PREFIX = 'docs/mcr/evidence/cozy-backgrounds-v1/native-02/'
OWNED = ('landing', 'login', 'customize', 'me')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def git_bytes(revision, path):
    return subprocess.check_output(['git', 'show', f'{revision}:{path}'], cwd=ROOT)


def luminance(rgb):
    values = [v / 255 for v in rgb[:3]]
    values = [v / 12.92 if v <= .04045 else ((v + .055) / 1.055) ** 2.4 for v in values]
    return sum(v * w for v, w in zip(values, (.2126, .7152, .0722)))


def contrast(a, b):
    low, high = sorted((luminance(a), luminance(b)))
    return (high + .05) / (low + .05)


raw = (PACKET / 'report.json').read_bytes()
assert digest(raw) == REPORT_HASH
assert raw == git_bytes(ARTIFACT, PREFIX + 'report.json')
report = json.loads(raw)
manifest_raw = (PACKET / 'capture-packet.json').read_bytes()
assert manifest_raw == git_bytes(ARTIFACT, PREFIX + 'capture-packet.json')
manifest = json.loads(manifest_raw)
assert report['candidate'] == manifest['production_revision'] == CANDIDATE
assert manifest['raw_report_sha256'] == REPORT_HASH
assert not report['errors']
assert report['cleanup'] == {'browserClosed': True, 'serverClosed': True, 'tempRemoved': True}
artifacts = {}
for item in manifest['artifacts']:
    name = Path(item['path']).name
    data = (PACKET / name).read_bytes()
    assert len(data) == item['bytes'] and digest(data) == item['sha256'], name
    assert data == git_bytes(ARTIFACT, item['path']), name
    artifacts[name] = item
assert sum(name.endswith('.png') for name in artifacts) == 60

source_count = 0
for mode, sources in report['sources'].items():
    for path, pin in sources.items():
        expected_revision = report['base'] if mode == 'baseline' and path in report['restored'] else CANDIDATE
        assert pin['revision'] == expected_revision, (mode, path)
        data = git_bytes(pin['revision'], path)
        assert len(data) == pin['bytes'] and digest(data) == pin['sha256'], (mode, path)
        source_count += 1
for path, pin in report['assets'].items():
    assert pin['revision'] == CANDIDATE
    data = git_bytes(pin['revision'], path)
    assert len(data) == pin['bytes'] and digest(data) == pin['sha256'], path
    source_count += 1
assert source_count == manifest['loaded_source_pins_verified'] == 729
css_path = 'frontend/src/shared/styles/cozy-identity-backgrounds-v1.css'
assert (ROOT / css_path).read_bytes() == git_bytes(CANDIDATE, css_path)
closed = json.loads((Path(__file__).parent / 'source-validation.json').read_text())
for item in closed['closedAssets']:
    pin = report['assets']['frontend/public' + item['url']]
    assert pin['sha256'] == item['sha256'] and pin['bytes'] == item['bytes']

# PostCSS reads nested CSS from disk rather than the recorded Vite load hook.
# Preserve the distinction between clean-checkout inference and a runtime read receipt.
attestation_raw = (PACKET / 'nested-css-attestation.json').read_bytes()
assert attestation_raw == git_bytes(ATTESTATION, PREFIX + 'nested-css-attestation.json')
attestation = json.loads(attestation_raw)
for leaf in attestation['leaves']:
    runner = git_bytes(manifest['runner_revision'], leaf['path'])
    frozen = git_bytes(CANDIDATE, leaf['path'])
    assert runner == frozen and len(runner) == leaf['bytes']
    assert digest(runner) == leaf['sha256']
    for revision, field in ((manifest['runner_revision'], 'runner_blob'), (CANDIDATE, 'frozen_blob')):
        blob = subprocess.check_output(['git', 'rev-parse', f"{revision}:{leaf['path']}"], cwd=ROOT, text=True).strip()
        assert blob == leaf[field]
guard_raw = (PACKET / 'guard.json').read_bytes()
assert guard_raw == git_bytes(ARTIFACT, PREFIX + 'guard.json')
guard = json.loads(guard_raw)
assert guard['candidate'] == manifest['runner_revision']
assert guard['exit_code'] == 0 and guard['failure'] is None
driver = (PACKET / 'driver.mjs.snapshot').read_bytes()
assert driver == git_bytes(ARTIFACT, PREFIX + 'driver.mjs.snapshot')
assert digest(driver) == guard['driver_sha256'] == manifest['driver_sha256']
guard_source = (PACKET / 'guard.py.snapshot').read_bytes()
assert guard_source == git_bytes(ARTIFACT, PREFIX + 'guard.py.snapshot')
assert "['git','status','--porcelain']" in guard_source.decode()
assert "['git','rev-parse','HEAD']" in guard_source.decode()

cases = {(c['id'], c['width'], c['mode']): c for c in report['cases']}
summaries = []
viewed = []
for width in (390, 360, 430):
    for route in OWNED:
        before, after = [cases[(route, width, mode)] for mode in ('baseline', 'candidate')]
        for c in (before, after):
            assert not c['errors'] and not c['metrics']['overflow']
            assert all(x['rect']['width'] >= 44 and x['rect']['height'] >= 44
                       for x in c['metrics']['controls'])
            assert artifacts[c['screenshot']]['sha256'] == c['screenshotPin']['sha256']
            assert artifacts[c['screenshot']]['bytes'] == c['screenshotPin']['bytes']
            viewed.append(c['screenshot'])
        for key in ('controls', 'images', 'text'):
            assert before['metrics'][key] == after['metrics'][key], (route, width, key)
        assert before['characters'] == after['characters'], (route, width, 'characters')
        chars = after['characters']
        expected = {'landing': (0, 4), 'login': (0, 0), 'customize': (4, 0), 'me': (1, 0)}[route]
        assert (chars['canvasCount'], chars['spanCount']) == expected
        for canvas in chars['canvases']:
            assert re.fullmatch('[0-9a-f]{64}', canvas['rgba'])
            assert all(v > 0 for v in canvas['dimensions'])
        for span in chars['spans']:
            assert span['sourceSha256'] == 'e124c88815b22220df2f2dc59e2f26b3ceb845e57b77674fd8e55383116d7efe'
            assert span['sourceBytes'] == 36030 and span['rendering'] == 'pixelated'
        assert after['focus']['outline'] == 'solid'
        summaries.append({'route': route, 'width': width, 'controls': len(after['metrics']['controls']),
                          'canvasCount': expected[0], 'spanCount': expected[1],
                          'pairedControlsTextImagesCharacters': 'PASS',
                          'characters': chars, 'overflow': False, 'errors': []})

focus = []
for mode in ('baseline', 'candidate'):
    c = cases[('me', 390, mode)]
    backing = Image.open(PACKET / c['screenshot']).convert('RGBA')
    for item in c['meIconFocus']:
        assert artifacts[item['file']]['sha256'] == item['sha256']
        assert artifacts[item['file']]['bytes'] == item['bytes']
        painted = Image.open(PACKET / item['file']).convert('RGBA')
        ratios = []
        for sample in item['samples']:
            xy = (sample['x'], sample['y'])
            assert list(backing.getpixel(xy)) == sample['backing']
            assert list(painted.getpixel(xy)) == sample['paint']
            ratios.append(contrast(sample['backing'], sample['paint']))
        assert max(abs(a - b) for a, b in zip(ratios, item['ratios'])) < 1e-10
        assert abs(min(ratios) - item['minimum']) < 1e-10
        assert (item['width'], item['offset'], item['style']) == (3, 3, 'solid')
        focus.append({'mode': mode, 'label': item['label'], 'minimum': min(ratios),
                      'samples': item['samples'], 'screenshot': item['file']})
        viewed.append(item['file'])
for i in range(4):
    assert focus[i + 4]['minimum'] >= focus[i]['minimum']
backing_only = []
for width in (360, 430):
    for mode in ('baseline', 'candidate'):
        c = cases[('me', width, mode)]
        image = Image.open(PACKET / c['screenshot']).convert('RGBA')
        labels = ('Edit profile', 'Dress up', 'Manage private social links', 'Sharing requests')
        for label in labels:
            rect = next(x['rect'] for x in c['metrics']['controls'] if x['text'] == label)
            x, y, w, h = (rect[k] for k in ('x', 'y', 'width', 'height'))
            points = [(x + w / 2, y - 4.5), (x + w / 2, y + h + 4.5),
                      (x - 4.5, y + h / 2), (x + w + 4.5, y + h / 2)]
            samples = []
            for px, py in points:
                xy = (math.floor(px), math.floor(py))
                color = image.getpixel(xy)
                samples.append({'xy': xy, 'backingRGBA': color,
                                'contrastToUnchangedRingRGB161_33_44': contrast(color, (161, 33, 44))})
            backing_only.append({'width': width, 'mode': mode, 'label': label,
                                 'actualFocusedCapture': False, 'samples': samples,
                                 'minimumBackingOnlyContrast': min(s['contrastToUnchangedRingRGB161_33_44'] for s in samples)})
for width in (360, 430):
    for label in labels:
        before, after = [next(x for x in backing_only if x['width'] == width and x['label'] == label and x['mode'] == mode)
                         for mode in ('baseline', 'candidate')]
        assert after['minimumBackingOnlyContrast'] >= before['minimumBackingOnlyContrast']
forced = cases[('me', 390, 'candidate')]
assert forced['forcedColors'] == {'active': True, 'motion': True, 'outline': 'solid'}
forced_pin = forced['forcedColorsScreenshot']
assert all(artifacts[forced_pin['file']][key] == forced_pin[key] for key in ('bytes', 'sha256'))
viewed.append(forced['forcedColorsScreenshot']['file'])
auth = []
for state in report['authStates']:
    assert all(artifacts[state['screenshot']['path']][key] == state['screenshot'][key] for key in ('bytes', 'sha256'))
    assert not state['errors'] and state['focus']['style'] == 'solid'
    assert all(c['width'] >= 44 and c['height'] >= 44 for c in state['controls'])
    auth.append({'state': state['authMode'], 'errors': [], 'recordedTargets44px': True,
                 'focus': state['focus'], 'screenshot': state['screenshot']})
    viewed.append(state['screenshot']['path'])
assert len(viewed) == len(set(viewed)) == 36

spot_specs = [
    ('candidate-landing-390.png', 'tagline', (120, 517, 274, 555), (248, 247, 245), [(82, 61, 76)]),
    ('candidate-login-390.png', 'heading', (143, 174, 250, 203), (25, 24, 23), [(251, 242, 223)]),
    ('candidate-me-390.png', 'profile name', (175, 78, 366, 98), (245, 236, 220), [(87, 51, 68)]),
    ('candidate-me-390.png', 'bio', (182, 142, 359, 171), (33, 29, 25), [(255, 240, 231)]),
    ('candidate-customize-390.png', 'Random button', (49, 528, 104, 547), (29, 26, 23), [(241, 232, 216)]),
    ('candidate-customize-390.png', 'disabled Save', (297, 528, 336, 547), (100, 91, 80), [(222, 211, 192)]),
    ('candidate-login-pending-390.png', 'pending status', (55, 220, 345, 238), (121, 29, 37), [(244, 223, 217), (251, 242, 223), (238, 228, 240)]),
    ('candidate-login-error-390.png', 'error status', (55, 220, 345, 255), (121, 29, 37), [(244, 223, 217), (251, 242, 223), (238, 228, 240)]),
    ('candidate-login-options-unavailable-390.png', 'unavailable status', (55, 220, 345, 236), (25, 24, 23), [(244, 223, 217), (251, 242, 223), (238, 228, 240)])
]
spots = []
for filename, label, region, core, backings in spot_specs:
    image = Image.open(PACKET / filename).convert('RGB')
    def first_pixel(color):
        return next([x, y] for y in range(region[1], region[3]) for x in range(region[0], region[2])
                    if image.getpixel((x, y)) == color)
    spots.append({'file': filename, 'label': label, 'regionXYXY': region,
                  'coreRGB': core, 'coreXY': first_pixel(core),
                  'observedBackingSamples': [{'rgb': color, 'xy': first_pixel(color),
                                              'contrast': contrast(core, color)} for color in backings],
                  'minimumSampledContrast': min(contrast(core, color) for color in backings)})

output = {
    'task': 'ART-SCREEN-ID01', 'status': 'IN_PROGRESS',
    'packetCommit': ARTIFACT, 'candidate': CANDIDATE, 'base': report['base'],
    'runner': manifest['runner_revision'], 'driverSha256': manifest['driver_sha256'],
    'job': report['job'], 'rawReportSha256': REPORT_HASH,
    'manifestSha256': digest(manifest_raw), 'startedAt': report['startedAt'],
    'finishedAt': report['finishedAt'], 'browser': report['browser'],
    'baselinePolicy': report['baselinePolicy'], 'restored': report['restored'],
    'all60ArtifactsMatchManifestAndImmutableBlobs': True,
    'all729RecordedPinsMatchGitObjects': True, 'ownCssMatchesFrozenCandidate': True,
    'all7ClosedAssetsMatchCapturedAssetPins': True,
    'nestedCssProvenance': {'attestationCommit': ATTESTATION,
                            'attestationSha256': digest(attestation_raw),
                            'guardSha256': digest(guard_raw),
                            'basis': attestation['basis'], 'verifiedLeaves': attestation['leaves']},
    'ownedPairedCases': summaries, 'meFocus390': focus, 'authStates390': auth,
    'meBackingOnly360430': backing_only,
    'forcedColorsMe390': forced['forcedColors'], 'directlyViewed36Pngs': viewed,
    'boundedContrastSpots390': spots,
    'cleanup': report['cleanup'],
    'limits': [
        'Controlled synthetic HTTP/C05/local admission; no live flow, physical device, Profile save or Me504 proof',
        'Only four paint integration files restored for baseline; other candidate and fixture source identical',
        'Actual four-link focused and forced-colors captures are 390px only; 360/430 core geometry captured',
        'Core focus check is one Tab; full keyboard focus return and all product states not certified',
        'Nested CSS cut identity is inferred from retained clean-runner guard, identical tracked blobs and observed paint; not an individual Vite load-hook receipt',
        'Contrast spots use selected observed glyph cores and region backings; not every glyph, antialias edge, state or a global accessibility certification',
        'Character invariance uses direct canvas RGBA and four actual Landing CSS span records, not empty document.images arrays',
        'Visual disposition and publication receipt recorded separately in owner documentation'
    ]
}
(Path(__file__).parent / 'owner-native-02.json').write_text(json.dumps(output, indent=2) + '\n')
print(json.dumps({'artifacts': 60, 'recordedPins': source_count, 'ownedPairedCases': 12,
                  'ownedViewedPngs': len(viewed), 'actualMeFocusMinimum': min(x['minimum'] for x in focus[4:]),
                  'checks': 'PASS', 'limits': output['limits']}))
