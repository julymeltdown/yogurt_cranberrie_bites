#!/usr/bin/env python3
"""Bounded stdlib validation and hash sealing; not a renderer/native UI test."""
import hashlib
import json
from pathlib import Path
import re
import sys
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[4]
ASSETS = ROOT / 'frontend/public/reference-window-controls-v2'
PACKET = Path(__file__).resolve().parent
EXPECTED = {'stars.svg': (72, 24), 'close.svg': (24, 24),
            'back.svg': (24, 24), 'expand.svg': (24, 24), 'reduce.svg': (24, 24)}
NS = '{http://www.w3.org/2000/svg}'
ATTRS = {'svg': {'width', 'height', 'viewBox', 'fill'},
         'path': {'d', 'fill', 'stroke', 'stroke-width', 'stroke-linecap', 'stroke-linejoin'}}
PALETTE = {'none', '#c32b35', '#f4ecd5'}


def check():
    manifest_path = ASSETS / 'manifest.json'
    manifest = json.loads(manifest_path.read_text())
    entries = {item['file']: item for item in manifest['assets']}
    assert set(entries) == set(EXPECTED), 'Manifest asset identity mismatch'
    assert {p.name for p in ASSETS.glob('*.svg')} == set(EXPECTED), 'Unexpected SVG set'
    report = {'schemaVersion': 1, 'task': 'ART-WINDOW-CONTROLS-02',
              'method': 'Python standard library XML/JSON and source byte inspection',
              'nativeQualification': 'NOT_RUN', 'checks': [], 'assets': []}
    for name, (width, height) in EXPECTED.items():
        data = (ASSETS / name).read_bytes()
        assert len(data) < 8192, f'{name}: oversized source'
        assert not re.search(rb'<!|<\?|&', data), f'{name}: declaration/entity/PI forbidden'
        svg = ET.fromstring(data)
        assert svg.tag == NS + 'svg', f'{name}: root/namespace'
        assert svg.attrib['width'] == str(width) and svg.attrib['height'] == str(height)
        assert svg.attrib['viewBox'] == f'0 0 {width} {height}'
        assert svg.attrib['fill'] == 'none', f'{name}: background must remain transparent'
        assert entries[name]['viewBox'] == [0, 0, width, height]
        assert entries[name]['alignmentY'] == height / 2
        assert len(svg) == (3 if name == 'stars.svg' else 1)
        for element in svg.iter():
            tag = element.tag.removeprefix(NS)
            assert element.tag == NS + tag and tag in ATTRS, f'{name}: unsupported element'
            assert set(element.attrib) <= ATTRS[tag], f'{name}: unsupported or active attribute'
            assert not (element.text or '').strip() and not (element.tail or '').strip()
            for key in ('fill', 'stroke'):
                if key in element.attrib:
                    assert element.attrib[key] in PALETTE, f'{name}: invalid palette/resource'
            if tag == 'path':
                assert re.fullmatch(r'[MmLlHhVvZz0-9.,+\s-]+', element.attrib['d']), f'{name}: unsupported path command'
                if name != 'stars.svg':
                    assert element.attrib['stroke'] == '#f4ecd5'
                    assert float(element.attrib['stroke-width']) == entries[name]['strokeWidth']
                    assert element.attrib['stroke-linecap'] == 'square'
        if name == 'stars.svg':
            assert [node.attrib['fill'] for node in svg] == ['#c32b35', '#f4ecd5', '#f4ecd5']
        sha = hashlib.sha256(data).hexdigest()
        assert entries[name]['sha256'] in (None, sha), f'{name}: stale source hash'
        entries[name]['sha256'] = sha
        report['assets'].append({'file': name, 'bytes': len(data), 'sha256': sha,
                                 'viewBox': entries[name]['viewBox']})
    preview = (PACKET / 'preview.html').read_text()
    assert not re.search(r'<(?:script|button|input|iframe|object|embed)\b', preview, re.I)
    assert not re.search(r'\son\w+\s*=', preview, re.I)
    assert set(re.findall(r'reference-window-controls-v2/([a-z]+\.svg)', preview)) == set(EXPECTED)
    report['checks'] = ['five bounded editable vector sources', 'valid XML and viewBox agreement',
                        'no scripts, links, fonts, text, raster, filters or external resources',
                        'explicit approved palette and transparent source background',
                        'stroke/alignment metadata agreement', 'static noninteractive specimen source',
                        'SHA-256 source pins']
    report['result'] = 'PASS'
    manifest['status'] = 'SOURCE_VALIDATED_NATIVE_QUALIFICATION_PENDING'
    manifest['qualification']['sourceReport'] = 'docs/mcr/evidence/art-window-controls-v2/source-validation.json'
    manifest_path.write_text(json.dumps(manifest, indent=2) + '\n')
    (PACKET / 'source-validation.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'result': 'PASS', 'assetCount': len(report['assets']),
                      'svgBytes': sum(a['bytes'] for a in report['assets']),
                      'report': str((PACKET / 'source-validation.json').relative_to(ROOT))}))


if __name__ == '__main__':
    try:
        check()
    except (AssertionError, KeyError, ValueError, ET.ParseError) as error:
        print(f'FAIL: {error}', file=sys.stderr)
        raise SystemExit(1)
