#!/usr/bin/env python3
"""Bind reviewed v2 into the existing source-bound quality-v6 template.

This changes avatar references only. It neither admits an event nor changes the
map, scenery, source review, supplied originals, or production registry.
"""
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[4]
OUT = ROOT / 'backend/releases/selected-character-motion-v2/reviewed/runtime-fixture'

def digest(value):
    return 'sha256:' + hashlib.sha256(json.dumps(value, ensure_ascii=True, sort_keys=True,
                                               separators=(',', ':')).encode()).hexdigest()

def main():
    source = ROOT / 'backend/releases/operator-bundle/quality-v6'
    document = json.loads((source / 'bootstrap.json').read_text())
    avatar = json.loads((OUT.parent / 'manifest.json').read_text())
    document['avatar_manifest'] = avatar
    state = {'revision': 1, 'config': avatar['presets'][0]['config']}
    document['avatar'] = state
    document['participant']['profile']['avatar'] = state
    OUT.mkdir(exist_ok=True)
    for theme in document['theme_manifests']:
        theme['avatar_manifest_id'] = avatar['manifest_id']
        theme['avatar_manifest_version'] = avatar['manifest_version']
        theme['avatar_content_hash'] = avatar['content_hash']
        theme.pop('content_hash')
        theme['content_hash'] = digest(theme)
        (OUT / ('theme-' + theme['theme_id'].lower() + '.json')).write_text(json.dumps(theme, indent=2) + '\n')
    (OUT / 'bootstrap.json').write_text(json.dumps(document, indent=2) + '\n')
    print('BUILT_RUNTIME_FIXTURE_NOT_ADMITTED: avatar binding only; source map/theme art unchanged')

if __name__ == '__main__':
    main()
