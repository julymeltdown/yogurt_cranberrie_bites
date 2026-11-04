#!/usr/bin/env python3
"""Stdlib source checks for TEST_ONLY reference vectors; no runtime certification."""
import hashlib
import json
from pathlib import Path
import re
import sys
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[4]
ASSETS = ROOT / 'frontend/public/reference-gifts-content-v1'
PACKET = Path(__file__).resolve().parent
FILES = {'sticker-pack.svg', 'hand-banner.svg', 'diy-buttons.svg'}
NS = '{http://www.w3.org/2000/svg}'
COMMON = {'fill', 'stroke', 'stroke-width', 'stroke-linejoin', 'stroke-linecap', 'fill-rule'}
ALLOWED = {'svg': {'width', 'height', 'viewBox', 'fill'}, 'g': COMMON | {'transform'},
           'path': COMMON | {'d'}, 'circle': COMMON | {'cx', 'cy', 'r'}}


def check():
    manifest_path = ASSETS / 'source-provenance.json'
    manifest = json.loads(manifest_path.read_text())
    assert manifest['usage'] == 'TEST_ONLY_REFERENCE_FIXTURE'
    assert manifest['notLiveInventory'] is True and manifest['productionBinding'] is None
    assert manifest['reference']['sha256'] == '2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23'
    assert manifest['reference']['pixelsCopied'] is False
    assert manifest['reference']['cropFilesCreated'] is False
    assert manifest['derivation']['charactersOrPortraits'] is False
    assert manifest['derivation']['fonts'] == [] and manifest['derivation']['thirdPartyAssets'] == []
    entries = {row['file']: row for row in manifest['assets']}
    assert len(entries) == len(manifest['assets']) == 3 and set(entries) == FILES
    assert {p.name for p in ASSETS.glob('*.svg')} == FILES
    palette = set(manifest['palette']) | {'none'}
    report = {'schemaVersion': 1, 'task': 'ART-GIFTS-CONTENT-02',
              'usage': manifest['usage'], 'method': 'Python stdlib XML/JSON and source inspection',
              'renderedInspection': 'NOT_RUN', 'nativeQualification': 'NOT_RUN',
              'productionBinding': 'NOT_READY', 'assets': []}
    for name in sorted(FILES):
        data = (ASSETS / name).read_bytes()
        assert len(data) < 32768, f'{name}: oversized source'
        assert not re.search(rb'<!|<\?|&', data), f'{name}: declaration/entity/PI forbidden'
        svg = ET.fromstring(data)
        assert svg.tag == NS + 'svg'
        assert svg.attrib == {'width': '256', 'height': '256', 'viewBox': '0 0 256 256', 'fill': 'none'}
        assert entries[name]['viewBox'] == [0, 0, 256, 256]
        for el in svg.iter():
            tag = el.tag.removeprefix(NS)
            assert el.tag == NS + tag and tag in ALLOWED, f'{name}: unsupported element'
            assert set(el.attrib) <= ALLOWED[tag], f'{name}: active or unsupported attribute'
            assert not (el.text or '').strip() and not (el.tail or '').strip()
            for key in ('fill', 'stroke'):
                if key in el.attrib:
                    assert el.attrib[key] in palette, f'{name}: palette/external resource'
            if 'transform' in el.attrib:
                assert re.fullmatch(r'rotate\([-+0-9.,\s]+\)', el.attrib['transform'])
            if tag == 'path':
                assert re.fullmatch(r'[MmLlHhVvCcQqSsTtAaZz0-9.,+\s-]+', el.attrib['d'])
            if tag == 'circle':
                assert float(el.attrib['r']) > 0
                for key in ('cx', 'cy', 'r'):
                    assert re.fullmatch(r'[0-9.]+', el.attrib[key])
            if 'stroke-width' in el.attrib:
                assert 0 < float(el.attrib['stroke-width']) <= 10
        sha = hashlib.sha256(data).hexdigest()
        assert entries[name]['sha256'] in (None, sha), f'{name}: stale source pin'
        entries[name]['sha256'] = sha
        report['assets'].append({'file': name, 'bytes': len(data), 'sha256': sha,
                                 'viewBox': entries[name]['viewBox'],
                                 'elementCount': sum(1 for _ in svg.iter())})
    preview = (PACKET / 'preview.html').read_text()
    assert 'TEST ONLY' in preview and 'not live inventory' in preview
    assert not re.search(r'<(?:script|button|input|iframe|object|embed)\b', preview, re.I)
    assert not re.search(r'\son\w+\s*=', preview, re.I)
    assert set(re.findall(r'reference-gifts-content-v1/([a-z-]+\.svg)', preview)) == FILES
    report['checks'] = ['three bounded editable XML vector sources', 'transparent square viewBoxes',
                        'no fonts, text nodes, raster, scripts, links, filters or external resources',
                        'explicit finite palette and safe geometry attributes',
                        'TEST_ONLY usage and unbound production metadata',
                        'reference hash and original/no-pixel-copy provenance',
                        'static specimen source', 'SHA-256 source pins']
    report['result'] = 'PASS'
    # Source validation does not own or reset a separately recorded native result.
    manifest['checks']['source'] = 'PASS'
    manifest_path.write_text(json.dumps(manifest, indent=2) + '\n')
    (PACKET / 'source-validation.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'result': 'PASS', 'assetCount': len(report['assets']),
                      'svgBytes': sum(a['bytes'] for a in report['assets'])}))


if __name__ == '__main__':
    try:
        check()
    except (AssertionError, KeyError, ValueError, ET.ParseError) as error:
        print(f'FAIL: {error}', file=sys.stderr)
        raise SystemExit(1)
