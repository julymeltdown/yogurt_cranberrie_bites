"""Read back a custodian's immutable focus packet; no browser or artifact copy.

Arguments: packet commit, repository-relative packet directory. Execute only in
a fresh named Resource21d slot after the packet is actually committed.
"""
from hashlib import sha256
from io import BytesIO
import json
from math import ceil
from pathlib import Path
import subprocess
import sys

from PIL import Image

ROOT = Path(__file__).resolve().parents[4]
CSS = 'frontend/src/app/shell-chrome/shell-chrome.css'
EXPECTED = '5c9e194e62eaf438f8b98436b4e8d7315702ead3307e60684fe4f61800d067c4'
PACKET, PREFIX = sys.argv[1:]
assert len(PACKET) == 40 and all(c in '0123456789abcdef' for c in PACKET)
assert PREFIX.startswith('docs/mcr/evidence/') and '..' not in PREFIX.split('/')
PREFIX = PREFIX.rstrip('/') + '/'


def blob(revision, path):
    return subprocess.check_output(['git', 'show', f'{revision}:{path}'], cwd=ROOT)


raw_report = blob(PACKET, PREFIX + 'report.json')
assert sha256(raw_report).hexdigest() == '795dadfdaecfafccea1a50242cbb2f14fe290d5b9e7d117224731b1dbf8aeb21'
report = json.loads(raw_report)
expected_errors = [{'id': name, 'mode': 'candidate', 'width': 390,
                    'error': 'Candidate forced focus contrast/clipping unresolved'} for name in ('map', 'me')]
assert report['status'] == 'OPEN_FAILURES' and report['errors'] == expected_errors
assert report['cleanup'] == {'browserClosed': True, 'serverClosed': True, 'tempRemoved': True}
assert len(report['cases']) == 18
for case in report['cases']:
    failed = case['mode'] == 'candidate' and case['width'] == 390 and case['id'] in ('map', 'me')
    assert case['status'] == ('FAIL' if failed else 'CAPTURED_PENDING_OWNER_VISUAL_INSPECTION')
assert report['sources']['candidate'][CSS]['sha256'] == EXPECTED
assert sha256(blob(report['candidate'], CSS)).hexdigest() == EXPECTED
assert report['minFree'] >= 8 * 1024**3
assert all(report['peaks'][key] <= value for key, value in report['limits'].items())
packet_bytes = blob(PACKET, PREFIX + 'capture-packet.json')
assert sha256(packet_bytes).hexdigest() == 'c489b856143bc728bb02ed48ef59e2f16fc64d33ed030662bce8daa8ceed768d'
packet = json.loads(packet_bytes)
assert sha256(blob(report['candidate'], packet['driver'])).hexdigest() == packet['driverSha256']
assert packet['driverSha256'] == 'f3d7db08efcce14a3f507113b6c6f3f556b0eb35385b2525515e78244b2bcc2d'
artifacts = json.loads(blob(PACKET, PREFIX + 'artifacts.json'))['artifacts']
assert len(artifacts) == 53
for path, pin in artifacts.items():
    data = blob(PACKET, PREFIX + path)
    assert len(data) == pin['bytes'] and sha256(data).hexdigest() == pin['sha256'], path
guard = json.loads(blob(PACKET, PREFIX + 'resource/guard.json'))
assert [c['exit_code'] for c in guard['commands']] == [0, 0, 0, 0, 1]
assert guard['failure'] == 'command-5-exit-1' and guard['temp_absent'] and guard['owned_link_absent']

# Independently hash every declared source/asset pin in one bounded Git batch.
# Duplicate modes pointing to the same immutable blob do not require re-reading.
pins = {}
declared_pins = sum(len(s) for s in report['sources'].values()) + len(report['assets'])
assert declared_pins == 688
for mode, sources in report['sources'].items():
    for path, pin in sources.items():
        key = pin['revision'] + ':' + path
        assert key not in pins or pins[key] == (pin['bytes'], pin['sha256'])
        pins[key] = pin['bytes'], pin['sha256']
for key, pin in report['assets'].items():
    path = key.split(':', 1)[1]
    name = pin['revision'] + ':' + path
    assert name not in pins or pins[name] == (pin['bytes'], pin['sha256'])
    pins[name] = pin['bytes'], pin['sha256']
process = subprocess.run(['git', 'cat-file', '--batch'],
                         input=''.join(name + '\n' for name in pins).encode(),
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=ROOT, check=True)
payload, offset = process.stdout, 0
for name, (size, digest) in pins.items():
    end = payload.index(b'\n', offset)
    object_id, kind, actual_size = payload[offset:end].split()
    assert kind == b'blob' and int(actual_size) == size, name
    offset = end + 1
    assert sha256(payload[offset:offset + size]).hexdigest() == digest, name
    offset += size
    assert payload[offset:offset + 1] == b'\n'
    offset += 1
assert offset == len(payload)
del payload, process

pngs = {}


def image(pin):
    data = blob(PACKET, PREFIX + pin['path'])
    assert len(data) == pin['bytes'] and sha256(data).hexdigest() == pin['sha256']
    pngs[pin['path']] = pin['sha256']
    return Image.open(BytesIO(data)).convert('RGB')


def luminance(rgb):
    values = [v / 255 for v in rgb[:3]]
    return sum((v / 12.92 if v <= .04045 else ((v + .055) / 1.055)**2.4) * weight
               for v, weight in zip(values, (.2126, .7152, .0722)))


def contrast(a, b):
    low, high = sorted((luminance(a), luminance(b)))
    return (high + .05) / (low + .05)


def forced_qualification(case, baseline):
    info = case['forced']
    actual = image(info['screenshot'])
    before = image(baseline['forced']['screenshot'])
    probe_path = info['screenshot']['path'].removesuffix('.png') + '-backing-probe.png'
    probe_bytes = blob(PACKET, PREFIX + probe_path)
    assert sha256(probe_bytes).hexdigest() == info['backingProbeSha256']
    probe = Image.open(BytesIO(probe_bytes)).convert('RGB')
    assert actual.crop((9, 7, 381, 57)).tobytes() == before.crop((9, 7, 381, 57)).tobytes()
    assert info['samples'] == baseline['forced']['samples']
    assert actual.getpixel((21, 32)) == (4, 0, 58) and probe.getpixel((21, 32)) == (0, 0, 0)
    assert info['samples'][2]['contrast'] < 3  # Retain the actual low same-pixel measurement.
    r = info['rect']
    left, top = ceil(r['x']), ceil(r['y'])
    right, bottom = ceil(r['x'] + r['width']) - 1, ceil(r['y'] + r['height']) - 1
    changed = qualifying = 0
    for y in range(top, bottom + 1):
        for x in range(left, right + 1):
            a, b = actual.getpixel((x, y)), probe.getpixel((x, y))
            if a != b:
                changed += 1
                qualifying += contrast(a, b) >= 3
    # W3C's rectangle area is4w+4h. Also exceed the more conservative complete
    # 2px exterior band's area, without counting the low-contrast glyph overlap.
    required = 4 * (r['width'] + r['height'])
    conservative = (r['width'] + 4) * (r['height'] + 4) - r['width'] * r['height']
    assert qualifying >= conservative >= required
    adjacent = []
    for y in range(top, bottom + 1):
        for x, outside in ((left, left - 1), (right, right + 1)):
            assert actual.getpixel((outside, y)) == (255, 255, 255)
            adjacent.append(contrast(actual.getpixel((x, y)), actual.getpixel((outside, y))))
    for x in range(left, right + 1):
        for y, outside in ((top, top - 1), (bottom, bottom + 1)):
            assert actual.getpixel((x, outside)) == (255, 255, 255)
            adjacent.append(contrast(actual.getpixel((x, y)), actual.getpixel((x, outside))))
    assert min(adjacent) >= 3
    return {'rawFailedPoint': info['samples'][2], 'headerRgbaIdenticalToBaseline': True,
            'changedStrokePixels': changed, 'strokePixelsAtLeast3AgainstSuppressedProbe': qualifying,
            'twoPixelPerimeterArea': required, 'conservativeExteriorBandArea': conservative,
            'allFourOuterEdgesMinimumAgainstAdjacentCanvas': min(adjacent),
            'areaMethodLimit': 'Stroke-suppressed focused probe, not a separately captured forced-unfocused state; no full AAA conformance claim'}


summary = []
for case in report['cases']:
    for kind in ('focus', 'forced', 'profileLinkFocus'):
        if kind not in case:
            continue
        focus = case[kind]
        image(focus['screenshot'])
        assert focus['focusVisible'] and focus['outlineStyle'] != 'none'
        assert len(focus['samples']) == 4
        if case['mode'] == 'candidate':
            assert all(s['expectedMatches'] for s in focus['samples'])
            assert focus['allOutlineSamplesVisible']
            if not (kind == 'forced' and case['id'] in ('map', 'me')):
                assert focus['minimum'] >= 3
        if case['mode'] == 'candidate' and case['width'] == 390:
            path = focus['screenshot']['path'].removesuffix('.png') + '-backing-probe.png'
            data = blob(PACKET, PREFIX + path)
            assert sha256(data).hexdigest() == focus['backingProbeSha256']
            pngs[path] = focus['backingProbeSha256']
    if 'unfocusedScreenshot' in case:
        image(case['unfocusedScreenshot'])
    if case['mode'] != 'candidate':
        continue
    baseline = next(c for c in report['cases'] if c['mode'] == 'baseline'
                    and c['id'] == case['id'] and c['width'] == case['width'])
    assert case['geometry'] == case['afterGeometry'] == baseline['geometry']
    assert case['characters'] == baseline['characters']
    assert not case['geometry']['overflow']
    assert case['state']['canvases'] == 1 and case['state']['socketPeak'] <= 1
    assert case['errors'] == []
    if case['id'] == 'meet':
        continue  # Independently owned Meet patch is outside this task's claim.
    for key in ('rect', 'font', 'color', 'outlineColor', 'outlineWidth', 'outlineOffset', 'outlineStyle'):
        assert case['focus'][key] == baseline['focus'][key]
    assert case['unfocusedHeader'] == baseline['unfocusedHeader']
    assert case['unfocusedHeader']['state'] is False
    assert case['unfocusedHeader']['background'] == 'none'
    assert case['unfocusedHeader']['color'] == 'rgb(37, 16, 23)'
    assert case['focusedGlyphs']['minimum'] >= 4.5
    assert case['unfocusedGlyphs']['minimum'] >= 4.5
    assert case['focus']['outlineColor'] == 'rgb(162, 35, 43)'
    normal = image(case['focus']['screenshot'])
    # Retained native frame coordinates; these pixels must now be visibly paper,
    # not merely paper hidden behind an opaque red stroke.
    width = case['width']
    for x in range(19, width - 145):
        assert normal.getpixel((x, 20)) == (233, 225, 212)
        assert normal.getpixel((x, 44)) == (233, 225, 212)
    for y in range(20, 45):
        assert normal.getpixel((19, y)) == (233, 225, 212)
        assert normal.getpixel((width - 146, y)) == (233, 225, 212)
    assert normal.getpixel((21, 24)) == (162, 35, 43)
    if case['id'] == 'me':
        assert image(case['unfocusedScreenshot']).getpixel((21, 24)) == (242, 234, 223)
    if 'forced' in case:
        assert case['forcedMedia']['forced'] and case['forcedMedia']['reduced']
        assert all(layer.strip() == 'none' for layer in case['forced']['backgroundImage'].split(','))
        assert case['forcedMedia']['headingColor'] == 'rgb(0, 0, 0)'
        qualification = forced_qualification(case, baseline)
    else:
        qualification = None
    summary.append({'id': case['id'], 'width': width, 'focusMinimum': case['focus']['minimum'],
                    'focusedGlyphMinimum': case['focusedGlyphs']['minimum'],
                    'unfocusedGlyphMinimum': case['unfocusedGlyphs']['minimum'],
                    'forcedRawMinimum': case.get('forced', {}).get('minimum'),
                    'forcedQualification': qualification,
                    'meProfileLinkMinimum': case.get('profileLinkFocus', {}).get('minimum')})
assert len(summary) == 6
assert len(pngs) == 43
result = {'task': 'TITLE-FOCUS-01', 'status': 'OWNER_QUALIFIED_WITH_RAW_NATIVE_EXIT1_PRESERVED',
          'packetCommit': PACKET, 'prefix': PREFIX, 'reportSha256': sha256(raw_report).hexdigest(),
          'candidate': report['candidate'], 'cssSha256': EXPECTED,
          'rawNativeExit': 1, 'rawNativeErrors': report['errors'], 'artifactPins': len(artifacts),
          'declaredSourceAssetPins': declared_pins, 'uniqueSourceAssetPins': len(pins),
          'uniquePngPins': len(pngs), 'titleCases': summary,
          'limits': ['Readback of actual custodian captures, no new rendering or copied packets',
                     'Synthetic transport/default-router qualification only; no live/multiuser/device or broad design/WCAG acceptance',
                     'Meet is separately owned; its source and artifacts are pinned but no Meet acceptance is claimed here',
                     'No CSS repair or recapture is justified by the inherited black-glyph point alone',
                     'Owner visual inspection and remote publication readback remain separate']}
output = Path(__file__).with_name('native-check.json')
output.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'status': result['status'], 'sourceAssetPins': len(pins),
                  'pngPins': len(pngs), 'titleCases': len(summary), 'outputBytes': output.stat().st_size}))
