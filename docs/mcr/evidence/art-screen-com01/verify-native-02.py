"""Read-only audit of Root's retained packet; never captures or copies images."""
import argparse
import hashlib
import json
import pathlib
import struct
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--repository', required=True)
parser.add_argument('--packet-root', required=True)
args = parser.parse_args()
repo = pathlib.Path(args.repository)
packet_root = pathlib.Path(args.packet_root)
artifact = '5186f11017185a040e4ccde1c59589e140563ef0'
production = '8972b0b634035055eb023dbc2f786e4463b172e5'
prefix = 'docs/mcr/evidence/cozy-backgrounds-v1/native-02/'
expected_report = '7f34e10279212d5f5d01017cbd644f02a33bd8939393b30818326ab2a489395b'
css_path = 'frontend/src/shared/styles/cozy-community-backgrounds-v1.css'
css_sha = 'd26dd8fbee928539f2689ccd9bb6269e4cb964c58b78e8cc86afbd894f228504'
owned_routes = ['/feed', '/meet', '/gifts', '/wall']

def digest(data):
    return hashlib.sha256(data).hexdigest()

def blob(revision, path):
    return subprocess.check_output(['git', 'show', revision + ':' + path], cwd=repo)

raw = (packet_root / 'report.json').read_bytes()
assert digest(raw) == expected_report
assert raw == blob(artifact, prefix + 'report.json')
report = json.loads(raw)
packet_raw = (packet_root / 'capture-packet.json').read_bytes()
assert packet_raw == blob(artifact, prefix + 'capture-packet.json')
packet = json.loads(packet_raw)
assert packet['raw_report_sha256'] == expected_report
assert report['candidate'] == packet['production_revision'] == production
assert report['errors'] == []
assert all(report['cleanup'].values())
artifacts = {entry['path']: entry for entry in packet['artifacts']}
images = []
for case in report['cases']:
    if case['route'] not in owned_routes:
        continue
    name = case['screenshot']
    path = prefix + name
    data = (packet_root / name).read_bytes()
    assert data == blob(artifact, path), name
    pin = {'bytes': len(data), 'sha256': digest(data)}
    assert pin == case['screenshotPin'], name
    assert all(artifacts[path][key] == value for key, value in pin.items())
    assert data[:8] == b'\x89PNG\r\n\x1a\n'
    dimensions = struct.unpack('>II', data[16:24])
    assert dimensions == (case['width'], 844), name
    assert not case['errors'] and not case['metrics']['overflow'], name
    images.append({'path': path, **pin, 'dimensions': list(dimensions)})
assert len(images) == 24
pairs = []
for route in owned_routes:
    for width in [390, 360, 430]:
        before, after = [next(case for case in report['cases'] if case['route'] == route
                             and case['width'] == width and case['mode'] == mode)
                         for mode in ['baseline', 'candidate']]
        for field in ['controls', 'text', 'images']:
            assert before['metrics'][field] == after['metrics'][field], (route, width, field)
        assert before['characters'] == after['characters'], (route, width, 'characters')
        assert all(after[field] for field in ['controlsPreserved', 'textPreserved',
                                              'imagesPreserved', 'characterPixelsPreserved'])
        controls = after['metrics']['controls']
        small = [control for control in controls if min(control['rect']['width'], control['rect']['height']) < 44]
        assert not small, (route, width, small)
        assert after['focus']['outline'] == 'solid'
        assert before['screenshotPin']['sha256'] != after['screenshotPin']['sha256']
        pairs.append({'route': route, 'width': width, 'controls': len(controls),
                      'minimumControlWidth': min(control['rect']['width'] for control in controls),
                      'minimumControlHeight': min(control['rect']['height'] for control in controls),
                      'canvasCount': after['characters']['canvasCount'],
                      'spanCount': after['characters']['spanCount'],
                      'htmlImageCount': len(after['metrics']['images']),
                      'characters': after['characters'],
                      'controlsTextImagesCharactersIdentical': True,
                      'overflow': False, 'errors': [], 'firstTabFocus': after['focus']})
css = blob(production, css_path)
assert digest(css) == css_sha
workspace = pathlib.Path(__file__).resolve().parents[4]
assert (workspace / css_path).read_bytes() == css
asset_pins = {}
for path, pin in report['assets'].items():
    if '/cute-community-v1/' not in path and not path.endswith('/cute-wall-me-v1/wall-stationery.svg'):
        continue
    data = blob(production, path)
    assert len(data) == pin['bytes'] and digest(data) == pin['sha256']
    asset_pins[path] = pin
assert len(asset_pins) == 4
print(json.dumps({'task': 'ART-SCREEN-COM01', 'kind': 'RETAINED_PACKET_OWNER_AUDIT',
                  'artifactCommit': artifact, 'production': production,
                  'runner': packet['runner_revision'], 'driverSha256': packet['driver_sha256'],
                  'job': report['job'], 'rawReportSha256': expected_report,
                  'capturePacketSha256': digest(packet_raw),
                  'qualification': report['qualification'], 'baselinePolicy': report['baselinePolicy'],
                  'startedAt': report['startedAt'], 'finishedAt': report['finishedAt'],
                  'browser': report['browser'], 'cleanup': report['cleanup'],
                  'cssPath': css_path, 'cssSha256': css_sha, 'assets': asset_pins,
                  'images': images, 'pairs': pairs,
                  'result': 'PASS: bytes, pins, paired geometry/content/characters, recorded targets and overflow',
                  'limits': [
                      'This read-only audit does not generate browser, image or server evidence.',
                      'Visual disposition is recorded separately by the owner after viewing all 24 original PNGs.',
                      'Controls list excludes details/summary elements; it does not establish every target in every state.',
                      'Own routes record first Tab outlineStyle only, not focus color/contrast/clipping/geometry.',
                      'paints.color stores backgroundColor; body foreground computed contrast was not captured.',
                      'Forced-colors evidence belongs to Me only, not these four routes.',
                      'All core contexts use reducedMotion=reduce; normal-motion equivalence was not run.',
                      'Zero HTML images on these fixtures does not qualify real post/gift media.',
                      'Wall has zero character canvases and remains unavailable; mural editor pixels are not exercised.',
                      'The nested community CSS is not a separate key in report.sources; exact production bytes, recorded URL paint and visual evidence are checked instead.',
                      'Only controlled synthetic HTTP/C05/local admission, not live services or physical devices.'
                  ], 'commandExitCode': 0}, indent=2))
