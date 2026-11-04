#!/usr/bin/env python3
"""Bind a published canonical map checkpoint without granting runtime admission."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[5]
PUBLIC = ROOT / 'frontend/public'
OUTPUT = HERE / 'candidate-map-v4-pin.json'


def git(*args):
    return subprocess.check_output(['git', '-C', str(ROOT), *args])


def sha(raw):
    return 'sha256:' + hashlib.sha256(raw).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=True, separators=(',', ':')).encode()


revision = sys.argv[1] if len(sys.argv) == 2 else json.loads(OUTPUT.read_bytes())['backend_map_manifest']['revision']
revision = git('rev-parse', '--verify', revision + '^{commit}').decode().strip()
# Caller fetches master first. Local-only producer commits must never be described as published.
subprocess.run(['git', '-C', str(ROOT), 'merge-base', '--is-ancestor', revision, 'origin/master'], check=True)
artifacts = {}
docs = {}
for name in ['map-manifest', 'source-binding', 'readiness', 'engineering-source-review']:
    path = 'backend/releases/reference-scene-v2/quality-v6/' + name + '.json'
    raw = git('show', revision + ':' + path)
    artifacts[name] = dict(revision=revision, path=path, bytes=len(raw), sha256=sha(raw))
    docs[name] = json.loads(raw)
m, binding, readiness, review = (docs[n] for n in ['map-manifest', 'source-binding', 'readiness', 'engineering-source-review'])
source = json.loads((HERE / 'source-binding-v4.json').read_bytes())
proposal = json.loads((PUBLIC / source['geometry_source']['path']).read_bytes())
identity = {k: m[k] for k in ['map_id', 'map_revision', 'content_hash', 'geometry_hash']}
assert {k: identity[k] for k in source['candidate_map']} == source['candidate_map']
assert m['geometry'] == proposal['geometry'] and m['places'] == proposal['places']
assert sha(canonical({k: v for k, v in m.items() if k != 'content_hash'})) == m['content_hash']
assert binding['version'] == 2 and binding['art_commit'] == source['source_revision']
assert all(binding[k] == v for k, v in dict(map_id=m['map_id'], map_revision=4, map_content_hash=m['content_hash'], geometry_hash=m['geometry_hash']).items())
assert m['tilemap']['path'] == source['tilemap']['path'] and m['tilemap']['sha256'] == source['tilemap']['sha256']
assert binding['tilemap_source'] == {k: source['tilemap'][k] for k in ['path', 'bytes', 'sha256']}
for expected in source['scenes']:
    actual = next(s for s in binding['scenes'] if s['theme_id'] == expected['theme_id'])
    assert actual['scenery_source'] == next(s for s in expected['sources'] if s['path'].endswith('/scenery.json'))
    for pin in expected['sources']:
        assert pin in actual['scene_sources'] or pin == actual['scenery_source'] or pin in [{k: r[k] for k in ['path', 'bytes', 'sha256']} for r in actual['runtime_resources']]
    assert actual['runtime_resources'] == [{k: v for k, v in r.items() if k != 'id'} for r in expected['runtime_resources']]
assert readiness['map'] == review['map'] == identity and readiness['state'] == 'NOT_ADMITTED'
assert review['source_binding_sha256'] == artifacts['source-binding']['sha256']
for pin in [source['geometry_source'], source['tilemap']]:
    raw = (PUBLIC / pin['path']).read_bytes()
    assert len(raw) == pin['bytes'] and sha(raw) == pin['sha256']
output = dict(format='MCR_REFERENCE_SCENE_CANONICAL_MAP_PIN', version=1, status='NOT_ADMITTED',
    authority='Published canonical map identity and engineering source pins only; no Product bootstrap, live, physical or fidelity acceptance.',
    candidate_map=identity, dimensions=dict(width_px=768, height_px=1296),
    backend_map_manifest=artifacts['map-manifest'], backend_source_binding=artifacts['source-binding'],
    backend_readiness={**artifacts['readiness'], 'state': readiness['state']},
    backend_engineering_source_review={**artifacts['engineering-source-review'], 'scope': review['scope']},
    tilemap={**source['tilemap'], 'source_revision': binding['tilemap_commit']},
    frontend_consumer=dict(source_revision='8d7cff5dbf749f336682cdbf64ee162f5d25f2ff', artist_revision=source['source_revision'],
        theme_ids=[s['theme_id'] for s in source['scenes']], binding='Exact geometry, original virtual places, public Tiled and all three quality-v6 source projections match.'),
    place_scope='Four original VIRTUAL_CONCEPT / CONCEPT_ONLY / UNREVIEWED places unchanged; no physical eligibility.',
    limitations=readiness['limits'] + ['Backend Rust/runtime qualification is a separate receipt; inspect its current status, never infer it from this static pin.', 'Frontend isolated native evidence is not real multiuser or physical device evidence.'])
OUTPUT.write_text(json.dumps(output, indent=2) + '\n')
print(json.dumps(dict(result='PASS', revision=revision, candidate_map=identity, status='NOT_ADMITTED')))
