#!/usr/bin/env python3
"""Build the source-bound Round 3 spatial candidate; never grant product admission."""
from __future__ import annotations
import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[3]
SOURCE_COMMIT = '8f89d1905a86143d34ff77d2951fc60651ab2497'
BASE = 'world/reference-scene-v2/'
GEOMETRY_HASH = 'sha256:20168a1df64cd3c72ea3b8cb576ca895dcff8c7edfb0e77886cc647fb1df37e5'
MAP_ID = 'mcr_reference_scene_r3'
MAX_SCENERY = 2_097_152
spec = importlib.util.spec_from_file_location('mcr_spatial_assets', ROOT / 'scripts/mcr/validate-assets.py')
assets = importlib.util.module_from_spec(spec)
spec.loader.exec_module(assets)


def sha(data):
    return 'sha256:' + hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def published_input(path):
    """Freeze exact published artist bytes and reject checkout drift."""
    relative = 'frontend/public/' + path
    data = subprocess.check_output(['git', 'show', SOURCE_COMMIT + ':' + relative], cwd=ROOT)
    actual = (ROOT / relative).read_bytes()
    require(data == actual, 'artist source changed from pinned revision: ' + path)
    return data


def file_pin(path, data):
    return {'path': path, 'sha256': sha(data), 'bytes': len(data)}


def png_size(data):
    require(len(data) >= 33 and data[:8] == b'\x89PNG\r\n\x1a\n'
            and data[8:16] == b'\x00\x00\x00\rIHDR', 'invalid PNG header')
    width, height = struct.unpack('>II', data[16:24])
    require(width > 0 and height > 0, 'empty PNG')
    return width, height


def candidate_map(proposal):
    require(proposal['format'] == 'MCR_REFERENCE_SCENE_PROPOSAL'
            and proposal['proposal_version'] == 2
            and proposal['status'] == 'PROPOSAL_NOT_RELEASE_BOUND', 'unknown proposal')
    geometry = copy.deepcopy(proposal['geometry'])
    require(assets.digest(geometry) == proposal['geometry_hash'] == GEOMETRY_HASH,
            'proposal geometry pin mismatch')
    manifest = {
        'contract_version': '1.0.0', 'kind': 'map_manifest', 'map_id': MAP_ID,
        'map_revision': 1, 'geometry_hash': GEOMETRY_HASH,
        'provenance': {
            'source_id': 'mcr_reference_scene_r3', 'source_kind': 'ORIGINAL_ART',
            'source_revision': SOURCE_COMMIT + '; geometry-proposal.json v2',
            'source_uri': 'https://github.com/julymeltdown/mcr/blob/' + SOURCE_COMMIT
                          + '/frontend/public/' + BASE + 'geometry-proposal.json',
            'author': 'MCR reference scene author; canonical spatial projection',
            'license': 'Original MCR virtual geometry and metadata; source attribution retained in place provenance.',
            'review_state': 'UNREVIEWED', 'reviewed_by': None, 'reviewed_at': None,
            'photo_permission': 'NOT_APPLICABLE',
        },
        'tilemap': {'format': 'TILED_JSON', 'resource_state': 'NOT_PROVIDED',
                    'path': None, 'sha256': None},
        'geometry': geometry, 'places': copy.deepcopy(proposal['places']),
    }
    manifest['content_hash'] = assets.content_hash(manifest)
    assets.validate_document(manifest, assets.validators(), 'spatial candidate')
    assets.validate_map(manifest)
    require(all(p['place_kind'] == 'VIRTUAL_CONCEPT'
                and p['verification_state'] == 'CONCEPT_ONLY' for p in manifest['places']),
            'proposal must not grant physical eligibility')
    return manifest


def build():
    names = ['geometry-proposal.json', 'painted/layout.json',
             'painted/resource-inputs.json', 'painted/provenance.json']
    data = {BASE + name: published_input(BASE + name) for name in names}
    proposal = assets.read_json(ROOT / 'frontend/public' / BASE / names[0])
    layout = json.loads(data[BASE + 'painted/layout.json'])
    resources = json.loads(data[BASE + 'painted/resource-inputs.json'])
    require(layout['version'] == resources['version'] == 3
            and layout['geometry_hash'] == resources['geometry_hash'] == GEOMETRY_HASH,
            'scene/geometry version mismatch')
    manifest = candidate_map(proposal)
    runtime = []
    for name in resources['runtime_resources']:
        path = BASE + 'painted/' + name
        raw = published_input(path)
        declared = next(r for r in resources['resources'] if r['path'] == name)
        require(sha(raw) == declared['sha256'] and len(raw) == declared['bytes'],
                'artist runtime resource pin mismatch: ' + path)
        width, height = png_size(raw)
        runtime.append({**file_pin(path, raw), 'width_px': width, 'height_px': height})
    byte_count = sum(r['bytes'] for r in runtime)
    pixels = sum(r['width_px'] * r['height_px'] for r in runtime)
    binding = {
        'format': 'MCR_SOURCE_BOUND_SPATIAL_RELEASE', 'version': 1,
        'source_commit': SOURCE_COMMIT, 'map_id': MAP_ID, 'map_revision': 1,
        'map_content_hash': manifest['content_hash'], 'geometry_hash': GEOMETRY_HASH,
        'geometry_source': file_pin(BASE + names[0], data[BASE + names[0]]),
        'scene_sources': [file_pin(BASE + name, data[BASE + name]) for name in names[1:]],
        'runtime_resources': runtime,
    }
    blockers = ['MISSING_PAINTED_REVENGE', 'MISSING_PAINTED_DANGER',
                'UNREVIEWED_SOURCE_PROVENANCE', 'TILED_RESOURCE_NOT_PROVIDED',
                'PRODUCT_BOOTSTRAP_NOT_CONFIGURED']
    if byte_count > MAX_SCENERY:
        blockers.append('C06_SCENERY_BYTE_BUDGET')
    if pixels > MAX_SCENERY:
        blockers.append('C06_SCENERY_PIXEL_BUDGET')
    fractional_depths = sum(type(p['depth_y']) is not int for p in layout['placements'])
    if fractional_depths:
        blockers.append('C06_INTEGER_SCENERY_PROJECTION_REQUIRED')
    readiness = {
        'task': 'BE-SPATIAL-RELEASE', 'state': 'NOT_ADMITTED',
        'source_commit': SOURCE_COMMIT, 'map': {key: manifest[key] for key in
            ['map_id', 'map_revision', 'content_hash', 'geometry_hash']},
        'skins': {'PARADE': 'SOURCE_PROPOSAL', 'REVENGE': 'UNAVAILABLE', 'DANGER': 'UNAVAILABLE'},
        'blockers': blockers,
        'scenery_budget': {'bytes': byte_count, 'pixels': pixels,
                           'max_bytes': MAX_SCENERY, 'max_pixels': MAX_SCENERY},
        'fractional_depth_count': fractional_depths,
        'bootstrap': None, 'theme_manifests': None,
        'limits': ['No art fidelity acceptance', 'No physical place qualification',
                   'No public URL invented for a missing Tiled resource',
                   'No event-current map publication or production admission'],
    }
    return {'map-manifest.json': manifest, 'source-binding.json': binding, 'readiness.json': readiness}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    directory = ROOT / 'backend/releases/reference-scene-v2'
    outputs = build()
    for name, document in outputs.items():
        # Canonical map bytes are accepted by the owning domain CatalogRelease.
        raw = assets.canonical_bytes(document) if name == 'map-manifest.json' else (
            json.dumps(document, ensure_ascii=True, sort_keys=True, indent=2) + '\n').encode()
        path = directory / name
        if args.check:
            require(path.is_file() and path.read_bytes() == raw, 'generated artifact drift: ' + name)
        else:
            directory.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
    print(json.dumps({'state': 'NOT_ADMITTED', 'map': outputs['readiness.json']['map'],
                      'blockers': outputs['readiness.json']['blockers']}, sort_keys=True))


if __name__ == '__main__':
    main()
