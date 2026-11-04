"""Audit retained Root evidence in place. Does not run or alter the application."""
import argparse
import hashlib
import json
import math
import pathlib
import re
import subprocess
from PIL import Image

parser = argparse.ArgumentParser()
parser.add_argument('--repository', required=True)
parser.add_argument('--packet-root', required=True)
args = parser.parse_args()
repo = pathlib.Path(args.repository)
packet_root = pathlib.Path(args.packet_root)
artifact = '1722a81734947764996590b94a63aa001501df28'
prefix = 'docs/mcr/evidence/cozy-backgrounds-v1/wall-com-03/'
expected_pins = {
    'report.json': '8056cea82655a3ae95e6520952a351890a3eb921953262378ebf260ca89b4c5f',
    'capture-packet.json': '4b1b8bf2d7a29678d072f6073c37e0a43ce646beb3a0cb99766275b2c952e61e',
    'qualification.json': 'f0ac612f201d43863086039a0284917f0cd9a6677fad5fe0b62a107d258c8301',
}

def sha(data):
    return hashlib.sha256(data).hexdigest()

def blob(revision, path):
    return subprocess.check_output(['git', 'show', revision + ':' + path], cwd=repo)

for name, pin in expected_pins.items():
    data = (packet_root / name).read_bytes()
    assert data == blob(artifact, prefix + name)
    assert sha(data) == pin
report = json.loads((packet_root / 'report.json').read_bytes())
packet = json.loads((packet_root / 'capture-packet.json').read_bytes())
qualification = json.loads((packet_root / 'qualification.json').read_bytes())
guard = json.loads((packet_root / 'guard.json').read_bytes())
assert report['candidate'] == packet['candidate'] == qualification['candidate'] == 'cc20832a897f18cd83482dcd21b6c2fc1d226942'
assert report['status'] == 'OPEN_FAILURES'
assert qualification['rawExit'] == guard['exit_code'] == 1
assert len(report['errors']) == 4
assert len(packet['artifacts']) == 32
pngs = {}
artifacts = []
for pin in packet['artifacts']:
    name = pathlib.PurePosixPath(pin['path']).name
    data = (packet_root / name).read_bytes()
    assert data == blob(artifact, pin['path']), name
    assert len(data) == pin['bytes'] and sha(data) == pin['sha256'], name
    artifacts.append(pin)
    if name.endswith('.png'):
        pngs[name] = Image.open(packet_root / name).convert('RGBA')
        assert pngs[name].height == 844
assert len(pngs) == 25
assert guard['temp_absent'] and guard['owned_link_absent']
assert all(report['cleanup'].values())
assert sha((packet_root / 'driver.mjs.snapshot').read_bytes()) == guard['driver_sha256']

# Only relevant production, fixture, CSS and avatar/background pins are checked;
# the Root-reported 725 total is not claimed as an independent full-source audit.
source_checks = []
relevant = [
    'frontend/src/app/__tests__/cozy-backgrounds-v1/preview.tsx',
    'frontend/src/app/router.tsx',
    'frontend/e2e/reference-integrated/whole/data.ts',
    'frontend/src/app/social-composition/__tests__/fixtures.ts',
    'frontend/src/features/event-feed/EventFeed.tsx',
    'frontend/src/features/party-discovery/ui/PartyDiscovery.tsx',
    'frontend/src/features/gift-discovery/ui/reference-presentation.tsx',
]
for mode in ['baseline', 'candidate']:
    for path in relevant:
        pin = report['sources'][mode][path]
        data = blob(pin['revision'], path)
        assert len(data) == pin['bytes'] and sha(data) == pin['sha256']
        assert pin == report['sources']['candidate'][path]
        source_checks.append({'kind': 'loaded-source', 'mode': mode, 'path': path, **pin})
css_paths = [
    'frontend/src/shared/styles/cozy-community-backgrounds-v1.css',
    'frontend/src/features/event-feed/feed.css',
    'frontend/src/features/party-discovery/ui/party-discovery.css',
    'frontend/src/features/gift-discovery/ui/reference-gifts.css',
    'frontend/src/app/shell.css',
    'frontend/src/app/shell-chrome/shell-chrome.css',
]
for path in css_paths:
    pin = next(pin for pin in report['cssCheckout'] if pin['path'] == path)
    data = blob(pin['revision'], path)
    assert sha(data) == pin['sha256'] and len(data) == pin['bytes']
    source_checks.append({'kind': 'physical-clean-checkout-not-individual-nested-load', **pin})
css_path = css_paths[0]
css_sha = 'd26dd8fbee928539f2689ccd9bb6269e4cb964c58b78e8cc86afbd894f228504'
workspace = pathlib.Path(__file__).resolve().parents[4]
assert sha((workspace / css_path).read_bytes()) == css_sha
assert sha(blob(report['candidate'], css_path)) == css_sha
for key, pin in report['assets'].items():
    mode, path = key.split(':', 1)
    if not any(part in path for part in ['/backgrounds/cute-community-v1/', '/backgrounds/cute-wall-me-v1/wall-stationery.svg', '/avatars/derived-v1/']):
        continue
    data = blob(pin['revision'], path)
    assert sha(data) == pin['sha256'] and len(data) == pin['bytes']
    source_checks.append({'kind': 'loaded-asset', 'mode': mode, 'path': path, **pin})
wall_path = 'frontend/public/backgrounds/cute-wall-me-v1/wall-stationery.svg'
assert report['assets']['baseline:' + wall_path]['sha256'] == '1929fda18afe73000f79a44a7cb37c5504bee42d450bff343a562baa85e2e227'
assert report['assets']['candidate:' + wall_path]['sha256'] == '22e2de672776682693bca71c57f501f2450bc9a28bc6314df44c26d6d193efc7'
assert blob(report['candidate'], wall_path) == blob('18e6e4db2f2e6384c796453d6e63caedc11875e8', wall_path)

# Native-02 retains the prior actual character-pixel evidence for unchanged
# Community routes; compare records without creating another paired matrix.
old_report = json.loads(blob('5186f11017185a040e4ccde1c59589e140563ef0',
                            'docs/mcr/evidence/cozy-backgrounds-v1/native-02/report.json'))
character_continuity = []
for route in ['feed', 'meet', 'gifts']:
    current = next(c for c in report['cases'] if c['id'] == route and c['mode'] == 'candidate')
    prior = next(c for c in old_report['cases'] if c['id'] == route and c['mode'] == 'candidate' and c['width'] == 390)
    assert current['characters'] == prior['characters']['canvases']
    assert prior['characters']['spans'] == []
    character_continuity.append({'route': route, 'priorPacket': '5186f11017185a040e4ccde1c59589e140563ef0',
                                 'sameActualCanvasRgbaRectDimensions': True,
                                 'canvasCount': len(current['characters']),
                                 'characters': current['characters']})

wall_pairs = []
for width in [390, 360, 430]:
    before, after = [next(c for c in report['cases'] if c['id'] == 'wall' and c['width'] == width and c['mode'] == mode) for mode in ['baseline', 'candidate']]
    assert before['metrics'] == after['metrics']
    assert before['characters'] == after['characters'] == []
    assert not before['errors'] and not after['errors'] and not after['metrics']['overflow']
    assert after['foregroundPreserved']
    assert 'Wall unavailable.' in after['metrics']['text']
    wall_pairs.append({'width': width, 'body': after['metrics']['body'], 'metricsAndCharactersIdentical': True,
                       'guardPreserved': True, 'baseline': before['screenshot'], 'candidate': after['screenshot']})

def luminance(rgb):
    values = [channel / 255 for channel in rgb[:3]]
    linear = [v / 12.92 if v <= .04045 else ((v + .055) / 1.055) ** 2.4 for v in values]
    return sum(v * w for v, w in zip(linear, [.2126, .7152, .0722]))

def ratio(a, b):
    low, high = sorted([luminance(a), luminance(b)])
    return (high + .05) / (low + .05)

def pixel(name, x, y):
    return list(pngs[name].getpixel((x, y)))

accessibility = []
for c in report['cases']:
    if c['mode'] != 'candidate' or c['width'] != 390:
        continue
    route = c['id']
    q = next(row for row in qualification['accessibility'] if row['route'] == route)
    assert c['metrics'] == c['metricsAfter'] and q['geometryStateRestored']
    assert not c['metrics']['overflow'] and not c['errors']
    assert c['forcedMedia']['forcedColors'] and c['forcedMedia']['reducedMotion']
    text_rows = []
    for target in c['text']['targets']:
        assert all(float(a['opacity']) == 1 and a['filter'] == 'none' and a['mixBlendMode'] == 'normal' for a in target['ancestors'])
        im = pngs[c['text']['backing']['path']]
        foreground = target['foreground']
        ratios = [ratio(foreground, im.getpixel((x, y)))
                  for box in target['boxes']
                  for y in range(math.ceil(box['y']), math.floor(box['y'] + box['height']))
                  for x in range(math.ceil(box['x']), math.floor(box['x'] + box['width']))]
        assert len(ratios) == target['pixels']
        minimum = min(ratios)
        assert abs(minimum - target['minimum']) < 1e-9 and minimum >= 4.5
        text_rows.append({'selector': target['selector'], 'index': target['index'], 'computedColor': target['color'],
                          'minimum': minimum, 'pixels': len(ratios), 'boxes': target['boxes']})
    assert abs(min(t['minimum'] for t in text_rows) - q['normalTextMin']) < 1e-9
    focus_rows = {}
    for field in ['focus', 'forcedFocus']:
        focus = c[field]
        assert focus['active'] and focus['focusVisible'] and focus['style'] == 'solid'
        assert min(focus['rect']['width'], focus['rect']['height']) >= 44
        color = [float(v) for v in re.findall(r'[\d.]+', focus['color'])]
        alpha = color[3] if len(color) == 4 else 1
        samples = []
        for sample in focus['samples']:
            actual = pixel(focus['screenshot']['path'], sample['x'], sample['y'])
            assert actual == sample['paint']
            expected = [math.floor(color[i] * alpha + sample['backing'][i] * (1 - alpha) + .5) for i in range(3)]
            matches = all(abs(expected[i] - actual[i]) <= 1 for i in range(3))
            contrast = ratio(sample['backing'], actual)
            samples.append({**sample, 'expectedAlphaCompositeRgb': expected, 'matchesWithOneChannelTolerance': matches, 'contrast': contrast})
        matches = [sample['matchesWithOneChannelTolerance'] for sample in samples]
        if route == 'meet':
            assert matches == [True, False, False, True]
            assert samples[1]['contrast'] == samples[2]['contrast'] == 1
        else:
            assert all(matches) and min(sample['contrast'] for sample in samples) >= 3
        focus_rows[field] = {'selector': focus['selector'], 'rect': focus['rect'], 'color': focus['color'],
                             'width': focus['width'], 'offset': focus['offset'], 'samples': samples,
                             'allFourSamplesVisible': all(matches), 'minimum': min(sample['contrast'] for sample in samples)}
    # Validate actual unmasked forced-color spots, not the invalid fill-mask.
    forced_spots = []
    for target in q['forcedTextActualUnmaskedRasterSpots']:
        assert target['computedColor'] == 'rgb(0, 0, 0)'
        for fragment in target['fragments']:
            for key in ['whiteBackplateSample', 'darkGlyphCoreSample']:
                spot = fragment[key]
                assert pixel(c['forcedFocus']['screenshot']['path'], spot['x'], spot['y']) == spot['rgba']
            white, black = fragment['whiteBackplateSample']['rgba'], fragment['darkGlyphCoreSample']['rgba']
            assert white == [255, 255, 255, 255] and black == [0, 0, 0, 255]
            assert ratio(white, black) == fragment['sampleContrast'] == 21
            forced_spots.append({'selector': target['selector'], **fragment})
    assert not c['forcedText']['passes'] and not c['forcedFocus']['passes']
    accessibility.append({'route': route, 'normalTextRecalculated': text_rows, **focus_rows,
                          'forcedTextUnmaskedSpotsVerified': forced_spots,
                          'rawForcedTextMaskIsNotQualified': True,
                          'rawFocusRGBComparisonIgnoredAlpha': True,
                          'geometryStateRestored': True,
                          'disposition': 'INHERITED_MEET_BOTTOM_LEFT_CLIP_OPEN' if route == 'meet' else 'REPRESENTATIVE_SAMPLES_QUALIFIED'})

inherited_sources = []
for row in qualification['meetInheritedSourceComparison']:
    before = blob(report['base'], row['path'])
    after = blob(report['candidate'], row['path'])
    assert before == after and sha(after) == row['sha256']
    inherited_sources.append(row)
meet = next(c for c in report['cases'] if c['id'] == 'meet' and c['mode'] == 'candidate')
clip = next(a for a in meet['focus']['ancestors'] if a['class'] == 'meet-discovery')
assert clip['overflowX'] == clip['overflowY'] == 'auto'
print(json.dumps({'task': 'ART-SCREEN-COM01', 'kind': 'READ_ONLY_QUALIFIED_OWNER_AUDIT',
                  'artifactCommit': artifact, 'candidate': report['candidate'], 'pins': expected_pins,
                  'job': report['job'], 'startedAt': report['startedAt'], 'finishedAt': report['finishedAt'],
                  'rawExit': 1, 'rawStatus': report['status'], 'rawErrors': report['errors'],
                  'qualificationDisposition': qualification['disposition'],
                  'qualification': report['qualification'], 'baselinePolicy': report['baselinePolicy'],
                  'artifactsVerified': artifacts, 'pngsDirectlyInspected': len(pngs),
                  'relevantSourcePinsVerified': source_checks,
                  'wallPairs': wall_pairs, 'accessibility': accessibility,
                  'actualCharacterContinuityFromNative02': character_continuity,
                  'inheritedMeetSourceComparison': inherited_sources, 'inheritedMeetClipAncestor': clip,
                  'localCssSha256': css_sha, 'localCssUnchanged': True,
                  'resourceRelease': guard, 'cleanup': report['cleanup'],
                  'auditCommandExitCode': 0,
                  'result': 'BOUNDED_BACKGROUND_EVIDENCE_QUALIFIED_WITH_REAL_INHERITED_MEET_FOCUS_LIMIT',
                  'limits': [
                      'Audit exit0 verifies retained evidence arithmetic/provenance; the original browser job remains exit1/OPEN_FAILURES.',
                      'Normal text covers only selected visible fragments; focus covers four perimeter points per representative target at390.',
                      'Forced text-fill masking is invalid under forced colors; actual black glyph/white backplate spots are local evidence only.',
                      'Alpha-composited focus samples qualify Wall/Feed/Gifts locally; Meet remains clipped and is not passed.',
                      'Physical CSS pins are clean-checkout evidence, not per-import Vite load receipts.',
                      'No global WCAG, all-state, real-media, live-service, physical-device or mural-editor acceptance.',
                      'Historical native02 oldWallNEEDS_CORRECTION remains frozen; this packet alone qualifies the tall correction.'
                  ]}, indent=2))
