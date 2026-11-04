"""Asset-only integrity checks. Does not certify composed UI accessibility."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent
before = {p.name: p.read_bytes() for p in ROOT.glob('*.svg')}
manifest_before = (ROOT / 'manifest.json').read_bytes()
subprocess.run([sys.executable, str(ROOT / 'author.py')], check=True, capture_output=True)
assert before == {p.name: p.read_bytes() for p in ROOT.glob('*.svg')}
assert manifest_before == (ROOT / 'manifest.json').read_bytes()
manifest = json.loads(manifest_before)
assert len(manifest['assets']) == 5
counts = {}
for asset in manifest['assets']:
    data = (ROOT / asset['file']).read_bytes()
    assert hashlib.sha256(data).hexdigest() == asset['sha256']
    assert len(data) == asset['bytes']
    root = ET.fromstring(data)
    assert root.attrib['viewBox'] == ' '.join(map(str, asset['viewBox']))
    ids = {e.attrib['id'] for e in root.iter() if 'id' in e.attrib}
    for element in root.iter():
        assert element.tag.split('}')[-1] in {'svg', 'defs', 'g', 'path', 'circle', 'ellipse', 'rect', 'use'}
        for key, value in element.attrib.items():
            assert not key.lower().startswith('on')
            if key == 'href':
                assert value.startswith('#') and value[1:] in ids
    counts[asset['file']] = sum(1 for _ in root.iter())

def luminance(color):
    rgb = [int(color[i:i+2], 16) / 255 for i in (1, 3, 5)]
    linear = [v / 12.92 if v <= .04045 else ((v + .055) / 1.055) ** 2.4 for v in rgb]
    return sum(v * weight for v, weight in zip(linear, [.2126, .7152, .0722]))

def contrast(a, b):
    low, high = sorted([luminance(a), luminance(b)])
    return round((high + .05) / (low + .05), 2)

pairs = [('#eee4cf', background) for background in ['#624250', '#6b4b5b', '#503b4a']]
pairs += [(foreground, background) for foreground in ['#211d19', '#554b3e', '#791d25']
          for background in ['#fbf2df', '#f4dfd9', '#eee4f0', '#e9eddd', '#f3e4d1', '#f0dedd', '#e9e5ed', '#e0e6cf']]
ratios = {a + ' on ' + b: contrast(a, b) for a, b in pairs}
assert min(ratios.values()) >= 4.5
report = {
    'status': 'PASS',
    'scope': 'Static asset integrity and flat palette checks only; rendered composition NOT_RUN',
    'checks': ['Five SVG XML documents parsed', 'Only inert vector elements and local fragment references',
               'Manifest sizes/viewBoxes/SHA256 match', 'Authoring output reproduces byte for byte',
               'No character asset references; zero external dependencies',
               'Existing text sampled against every broad field: contrast >=4.5; illustration zone excluded'],
    'elements': counts, 'contrast_ratios': ratios,
    'contrast_limit': 'Light moon/cloud midzone is illustration-only and not approved for white lettering. Owner must verify composed foreground does not overlap it.',
    'rendered_evidence': 'NOT_RUN', 'publication': 'NOT_RUN',
}
(ROOT / 'static-validation.json').write_text(json.dumps(report, indent=2) + '\n')
print(f'PASS: five deterministic inert SVGs; {sum(a["bytes"] for a in manifest["assets"])} bytes; minimum sampled text contrast {min(ratios.values())}:1')
