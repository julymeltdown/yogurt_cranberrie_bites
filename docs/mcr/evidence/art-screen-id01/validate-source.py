"""Check granted CSS boundaries and closed asset source bytes; no rendering."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[4]
CSS = ROOT / 'frontend/src/shared/styles/cozy-identity-backgrounds-v1.css'
ENTRY = '9a962ea6f8c20fe59846cbcf4f468b902e3f280e'
ME = '77d8e2608a28286bcce18df000eeea4069c826d1'

def git_bytes(revision, path):
    return subprocess.check_output(['git', 'show', f'{revision}:{path}'], cwd=ROOT)

manifest = json.loads(git_bytes(ENTRY, 'frontend/public/backgrounds/cute-entry-v1/manifest.json'))
pins = [{**item, 'revision': ENTRY} for item in manifest['assets']]
for filename, size, digest in [
    ('me-scrapbook.svg', 2728, '98c351c141e372d8d7e0d6879556105c551fbdf7478d66afe76d360a59a7aae1'),
    ('me-night-ribbon.svg', 2298, '29ad1cb9d04101399e2483fef9de6a3d710c6bf7f08cb1cb4501fb746152932d'),
]:
    pins.append({'file': filename, 'url': f'/backgrounds/cute-wall-me-v1/{filename}',
                 'bytes': size, 'sha256': digest, 'revision': ME})

for item in pins:
    data = git_bytes(item['revision'], 'frontend/public' + item['url'])
    assert len(data) == item['bytes'], item['file']
    assert hashlib.sha256(data).hexdigest() == item['sha256'], item['file']
    element = ET.fromstring(data)
    assert element.tag.endswith('svg'), item['file']
    if item['file'] == 'me-night-ribbon.svg':
        assert element.attrib['viewBox'] == '0 0 390 180'
        assert element.attrib['preserveAspectRatio'] == 'none'
    for node in element.iter():
        assert node.tag.rsplit('}', 1)[-1] not in {'script', 'foreignObject', 'image', 'text', 'filter', 'animate', 'animateTransform'}, item['file']
        for key, value in node.attrib.items():
            assert not key.lower().startswith('on'), item['file']
            if key.rsplit('}', 1)[-1] == 'href':
                assert value.startswith('#'), item['file']

background_selectors = {
    '.public-entry--poster', '.public-entry--poster .public-poster',
    '.public-entry--poster .public-header', '.auth-sign-in .concert-window',
    '.me-hub--reference .me-profile-header', '.me-hub--reference .me-bio',
    '.me-hub--reference .me-activity-navigation',
    '.auth-sign-in .auth-cemetery--top', '.auth-sign-in .auth-cemetery--bottom',
    ".shell-panel[data-mcr-panel='customize'] > .shell-panel-body",
    ".shell-panel[data-mcr-panel='me'] > .shell-panel-body",
}
for wrapper in ['mcr-motion-editor', 'mcr-whole-editor', 'mcr-avatar-editor']:
    for child in ['body', 'footer']:
        background_selectors.add(f'.{wrapper} > .concert-window > .concert-window__{child}')
opacity_selectors = {'.public-entry--poster .landing-web > g',
                     '.public-entry--poster .landing-scene > g',
                     '.auth-sign-in .auth-cemetery > image'}
properties = {'background-color', 'background-image', 'background-position',
              'background-size', 'background-repeat', 'background-blend-mode'}
image_mapping = {
    '.public-entry--poster .public-poster': '/backgrounds/cute-entry-v1/landing-garden-night.svg',
    '.auth-sign-in .concert-window': '/backgrounds/cute-entry-v1/sign-in-paper.svg',
    '.auth-sign-in .auth-cemetery--top': '/backgrounds/cute-entry-v1/sign-in-clouds-top.svg',
    '.auth-sign-in .auth-cemetery--bottom': '/backgrounds/cute-entry-v1/sign-in-garden-bottom.svg',
    '.me-hub--reference .me-profile-header': '/backgrounds/cute-wall-me-v1/me-night-ribbon.svg',
    '.me-hub--reference .me-bio': '/backgrounds/cute-wall-me-v1/me-scrapbook.svg',
    '.me-hub--reference .me-activity-navigation': '/backgrounds/cute-wall-me-v1/me-scrapbook.svg',
    ".shell-panel[data-mcr-panel='customize'] > .shell-panel-body": '/backgrounds/cute-entry-v1/customize-paper.svg',
    ".shell-panel[data-mcr-panel='me'] > .shell-panel-body": '/backgrounds/cute-wall-me-v1/me-scrapbook.svg',
}
for selector in background_selectors:
    if selector.startswith('.mcr-'):
        image_mapping[selector] = '/backgrounds/cute-entry-v1/customize-paper.svg'
source = CSS.read_text()
plain = re.sub(r'/\*.*?\*/', '', source, flags=re.S)
rules = re.findall(r'([^{}]+)\{([^{}]+)\}', plain)
seen = set()
urls = set()
for selector_list, body in rules:
    selectors = [s.strip() for s in selector_list.split(',')]
    assert all(s.startswith(':root ') for s in selectors), selectors
    selectors = [s.removeprefix(':root ') for s in selectors]
    seen.update(selectors)
    for declaration in body.split(';'):
        if not declaration.strip():
            continue
        key, value = [s.strip() for s in declaration.split(':', 1)]
        assert '!important' not in value
        if key == 'opacity':
            assert set(selectors) <= opacity_selectors and value == '0'
        else:
            assert set(selectors) <= background_selectors and key in properties, (selectors, key)
        used = re.findall(r"url\(['\"]?([^'\")]+)['\"]?\)", value)
        if key == 'background-image':
            assert all(used == [image_mapping[s]] for s in selectors), selectors
        if selectors == ['.me-hub--reference .me-profile-header'] and key == 'background-color':
            assert value == '#1c1a18', 'Preserve former Me header fallback contrast'
        urls.update(used)
assert seen == background_selectors | opacity_selectors
assert urls == {item['url'] for item in pins}

report = {'task': 'ART-SCREEN-ID01', 'status': 'IN_PROGRESS',
          'base': '7c25078f118da53dd9a524d0a58e707cca190f0f',
          'sourceSha256': hashlib.sha256(CSS.read_bytes()).hexdigest(),
          'grantedSelectorCount': len(seen), 'closedAssets': pins,
          'checks': {'selectorProperties': 'PASS', 'assetBytesAndHashes': 'PASS',
                     'svgLocalInertSource': 'PASS', 'allSevenAssetsConsumed': 'PASS'},
          'limits': ['Static source validation only; no SVG decode or actual rendered contrast evidence',
                     'Root alone integrates assets/imports and publishes; no root imports authored here',
                     'Corrected paint before/after390,360/430 and four focused Me links NOT_RUN; original390 evidence is separate',
                     'Visual acceptance and physical-device/live-flow qualification remain unproven']}
(ROOT / 'docs/mcr/evidence/art-screen-id01/source-validation.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'selectors': len(seen), 'assets': len(pins), 'checks': report['checks']}))
