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
ART_COMMIT = 'e3af146ae4757794ca153363fd1408300ffde29e'
TILED_COMMIT = 'aa3e2f3d67afc1f288aee63f994dc829fcc81e7b'
BASE = 'world/reference-scene-v2/'
RUNTIME = BASE + 'runtime-v4/'
TILED_PATH = BASE + 'runtime/canonical-map-v2.json'
TILED_HASH = 'sha256:5436a99d36c9b91a4c59302ee65130f327214e4ca78981620759846bb40e919f'
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


def published_input(path, revision=SOURCE_COMMIT):
    """Freeze exact published artist bytes and reject checkout drift."""
    relative = 'frontend/public/' + path
    data = subprocess.check_output(['git', 'show', revision + ':' + relative], cwd=ROOT)
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
        'map_revision': 2, 'geometry_hash': GEOMETRY_HASH,
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
        'tilemap': {'format': 'TILED_JSON', 'resource_state': 'PROVIDED',
                    'path': TILED_PATH, 'sha256': TILED_HASH},
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
    geometry_path = BASE + 'geometry-proposal.json'
    geometry_data = published_input(geometry_path)
    names = ['parade-layout.json', 'parade-resource-inputs.json',
             'parade-scenery.json', 'parade-integer-projection.json',
             'parade-map-metadata.json']
    data = {RUNTIME + name: published_input(RUNTIME + name, ART_COMMIT) for name in names}
    proposal = json.loads(geometry_data)
    tiled_data = published_input(TILED_PATH, TILED_COMMIT)
    require(sha(tiled_data) == TILED_HASH and len(tiled_data) == 24058, 'published Tiled pin mismatch')
    geometry = proposal['geometry']
    expected_tiled = {
        'type': 'map', 'version': '1.10', 'tiledversion': '1.10.2',
        'orientation': 'orthogonal', 'renderorder': 'right-down', 'infinite': False,
        'width': 48, 'height': 81, 'tilewidth': 16, 'tileheight': 16,
        'layers': [{'id': i + 1, 'name': name, 'type': 'tilelayer', 'width': 48, 'height': 81,
                    'x': 0, 'y': 0, 'opacity': 1, 'visible': True,
                    'data': [cell for row in geometry[field] for cell in row]}
                   for i, (name, field) in enumerate([('terrain', 'terrain_layer'),
                       ('collision', 'collision_grid'), ('overdraw', 'overdraw_layer')])],
        'tilesets': [{'firstgid': 1, 'name': 'theme', 'image': 'mcr-theme.png',
                      'imagewidth': 768, 'imageheight': 1296, 'tilewidth': 16,
                      'tileheight': 16, 'tilecount': 3888, 'columns': 48, 'margin': 0, 'spacing': 0}],
    }
    require(json.loads(tiled_data) == expected_tiled, 'published Tiled differs from canonical finite grids')
    layout = json.loads(data[RUNTIME + 'parade-layout.json'])
    resources = json.loads(data[RUNTIME + 'parade-resource-inputs.json'])
    scenery = json.loads(data[RUNTIME + 'parade-scenery.json'])
    require(layout['version'] == resources['version'] == 4
            and layout['geometry_hash'] == resources['geometry_hash'] == GEOMETRY_HASH,
            'scene/geometry version mismatch')
    require(resources['source_revision'] == SOURCE_COMMIT
            and resources['theme_id'] == 'PARADE'
            and resources['status'] == 'PROPOSAL_NOT_RELEASE_BOUND', 'runtime source mismatch')
    # Validate the exact closed public Scenery definition without manufacturing
    # a theme manifest or upgrading the source's review state.
    api = assets.read_json(ROOT / 'backend/docs/api/mcr-core.openapi.json')
    assets.Draft202012Validator({'$ref': '#/components/schemas/Scenery',
                                'components': api['components']}).validate(scenery)
    assets.validate_scenery(scenery, proposal['geometry'])
    require(len(scenery['objects']) == 22, 'unexpected WORLD object projection')
    manifest = candidate_map(proposal)
    runtime = []
    for declared in resources['resources']:
        path = RUNTIME + declared['path']
        raw = published_input(path, ART_COMMIT)
        require(sha(raw) == declared['sha256'] and len(raw) == declared['bytes'],
                'artist runtime resource pin mismatch: ' + path)
        width, height = png_size(raw)
        require((width, height) == (declared['width'], declared['height']), 'runtime dimensions mismatch')
        runtime.append({**file_pin(path, raw), 'width_px': width, 'height_px': height})
    require([r['path'] for r in runtime] == [scenery['ground']['path']]
            + [a['path'] for a in scenery['atlases']], 'scenery/resource path mismatch')
    for pin, descriptor in zip(runtime, [scenery['ground']] + scenery['atlases']):
        require(all(pin[k] == descriptor[k] for k in ['path', 'sha256', 'bytes', 'width_px', 'height_px']),
                'scenery/resource pin mismatch')
    byte_count = sum(r['bytes'] for r in runtime)
    pixels = sum(r['width_px'] * r['height_px'] for r in runtime)
    binding = {
        'format': 'MCR_SOURCE_BOUND_SPATIAL_RELEASE', 'version': 1,
        'source_commit': SOURCE_COMMIT, 'art_commit': ART_COMMIT, 'tilemap_commit': TILED_COMMIT,
        'map_id': MAP_ID, 'map_revision': 2,
        'map_content_hash': manifest['content_hash'], 'geometry_hash': GEOMETRY_HASH,
        'geometry_source': file_pin(geometry_path, geometry_data),
        'tilemap_source': file_pin(TILED_PATH, tiled_data),
        'scenery_source': file_pin(RUNTIME + 'parade-scenery.json', data[RUNTIME + 'parade-scenery.json']),
        'scene_sources': [file_pin(RUNTIME + name, data[RUNTIME + name]) for name in names
                          if name != 'parade-scenery.json'],
        'runtime_resources': runtime,
    }
    blockers = ['MISSING_PAINTED_REVENGE', 'MISSING_PAINTED_DANGER',
                'UNREVIEWED_SOURCE_PROVENANCE',
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
        'source_commit': SOURCE_COMMIT, 'art_commit': ART_COMMIT, 'tilemap_commit': TILED_COMMIT,
        'map': {key: manifest[key] for key in
            ['map_id', 'map_revision', 'content_hash', 'geometry_hash']},
        'skins': {'PARADE': 'SOURCE_PROPOSAL', 'REVENGE': 'UNAVAILABLE', 'DANGER': 'UNAVAILABLE'},
        'blockers': blockers,
        'scenery_budget': {'bytes': byte_count, 'pixels': pixels,
                           'max_bytes': MAX_SCENERY, 'max_pixels': MAX_SCENERY},
        'fractional_depth_count': fractional_depths,
        'bootstrap': None, 'theme_manifests': None,
        'limits': ['No art fidelity acceptance', 'No physical place qualification',
                   'Published Tiled resource binds the same canonical grids',
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
