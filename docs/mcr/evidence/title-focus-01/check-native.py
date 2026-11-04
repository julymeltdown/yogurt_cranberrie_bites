"""Read back a custodian's immutable focus packet; no browser or artifact copy.

Arguments: packet commit, repository-relative packet directory. Execute only in
a fresh named Resource21d slot after the packet is actually committed.
"""
from hashlib import sha256
from io import BytesIO
import json
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
report = json.loads(raw_report)
assert report['errors'] == []
assert report['cleanup'] == {'browserClosed': True, 'serverClosed': True, 'tempRemoved': True}
assert len(report['cases']) == 18
assert {c['status'] for c in report['cases']} == {'CAPTURED_PENDING_OWNER_VISUAL_INSPECTION'}
assert report['sources']['candidate'][CSS]['sha256'] == EXPECTED
assert sha256(blob(report['candidate'], CSS)).hexdigest() == EXPECTED
assert report['minFree'] >= 8 * 1024**3
assert all(report['peaks'][key] <= value for key, value in report['limits'].items())

# Independently hash every declared source/asset pin in one bounded Git batch.
# Duplicate modes pointing to the same immutable blob do not require re-reading.
pins = {}
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


summary = []
for case in report['cases']:
    for kind in ('focus', 'forced'):
        if kind not in case:
            continue
        focus = case[kind]
        image(focus['screenshot'])
        assert focus['focusVisible'] and focus['outlineStyle'] != 'none'
        assert len(focus['samples']) == 4
        if case['mode'] == 'candidate':
            assert all(s['expectedMatches'] for s in focus['samples'])
            assert focus['minimum'] >= 3 and focus['allOutlineSamplesVisible']
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
        assert case['forced']['backgroundImage'] == 'none'
        assert case['forcedMedia']['headingColor'] == 'rgb(0, 0, 0)'
    summary.append({'id': case['id'], 'width': width, 'focusMinimum': case['focus']['minimum'],
                    'focusedGlyphMinimum': case['focusedGlyphs']['minimum'],
                    'unfocusedGlyphMinimum': case['unfocusedGlyphs']['minimum'],
                    'forcedMinimum': case.get('forced', {}).get('minimum')})
assert len(summary) == 6
result = {'task': 'TITLE-FOCUS-01', 'status': 'ACTUAL_PACKET_READBACK_PASS',
          'packetCommit': PACKET, 'prefix': PREFIX, 'reportSha256': sha256(raw_report).hexdigest(),
          'candidate': report['candidate'], 'cssSha256': EXPECTED,
          'uniqueSourceAssetPins': len(pins), 'uniquePngPins': len(pngs), 'titleCases': summary,
          'limits': ['Readback of actual custodian captures, no new rendering or copied packets',
                     'Synthetic transport/default-router qualification only; no live/multiuser/device or broad design/WCAG acceptance',
                     'Meet is separately owned; its source and artifacts are pinned but no Meet acceptance is claimed here',
                     'Owner visual inspection and remote publication readback remain separate']}
output = Path(__file__).with_name('native-check.json')
output.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'status': result['status'], 'sourceAssetPins': len(pins),
                  'pngPins': len(pngs), 'titleCases': len(summary), 'outputBytes': output.stat().st_size}))
