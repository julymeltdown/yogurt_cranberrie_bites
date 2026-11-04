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
ART_COMMIT = '023201343a452b859759e02039d367c11d1ff8ad'
TILED_COMMIT = 'aa3e2f3d67afc1f288aee63f994dc829fcc81e7b'
BASE = 'world/reference-scene-v2/'
RUNTIME = BASE + 'eras-v5/'
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
    proposal = json.loads(geometry_data)
    manifest = candidate_map(proposal)
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
    manifest_path = RUNTIME + 'manifest.json'
    manifest_data = published_input(manifest_path, ART_COMMIT)
    require(sha(manifest_data) == 'sha256:65d3d8ecbdd7263b343a492aa78cb0ccb6422af50d17398571fef78be8a9dfd4'
            and len(manifest_data) == 6138, 'three-era source manifest pin mismatch')
    art = json.loads(manifest_data)
    require(art['version'] == 5 and art['geometry']['source_revision'] == SOURCE_COMMIT
            and art['geometry']['geometry_hash'] == GEOMETRY_HASH
            and art['geometry']['sha256'] == sha(geometry_data), 'three-era geometry mismatch')
    require({t['theme_id'] for t in art['themes']} == {'REVENGE', 'PARADE', 'DANGER'}
            and len(art['themes']) == 3, 'all three distinct era inputs required')
    projection_path = BASE + 'runtime-v4/parade-integer-projection.json'
    projection_data = published_input(projection_path, ART_COMMIT)
    require(sha(projection_data) == art['integer_projection']['sha256'], 'integer source projection mismatch')
    projection = json.loads(projection_data)
    scenes = []
    budgets = {}
    reference = None
    atlas_hashes = set()
    api = assets.read_json(ROOT / 'backend/docs/api/mcr-core.openapi.json')
    for theme in art['themes']:
        theme_id = theme['theme_id']
        prefix = RUNTIME + theme_id.lower() + '/'
        files = {}
        for pin in theme['files']:
            path = RUNTIME + pin['path']
            require(path.startswith(prefix) and '..' not in path.split('/') and path not in files,
                    'invalid or duplicate era source path')
            raw = published_input(path, ART_COMMIT)
            require(sha(raw) == pin['sha256'] and len(raw) == pin['bytes'], 'era source pin mismatch: ' + path)
            files[path] = raw
        scenery_path = prefix + 'scenery.json'
        scenery = json.loads(files[scenery_path])
        layout = json.loads(files[prefix + 'layout.json'])
        resources = json.loads(files[prefix + 'resource-inputs.json'])
        require(layout['version'] == resources['version'] == 5
                and resources['theme_id'] == theme_id
                and layout['geometry_hash'] == resources['geometry_hash'] == GEOMETRY_HASH,
                'era source version/geometry mismatch')
        assets.Draft202012Validator({'$ref': '#/components/schemas/Scenery',
                                    'components': api['components']}).validate(scenery)
        assets.validate_scenery(scenery, geometry)
        require(len(scenery['objects']) == len(layout['placements']) == 22
                and len(layout['overview_representations']) == 6, 'unexpected WORLD/MAP projection')
        for obj, placement in zip(scenery['objects'], layout['placements']):
            require(all(obj[k] == placement[k] for k in ['id', 'x', 'y', 'width', 'height', 'depth_y']),
                    'scenery placement differs from authored integer source')
            source = next(p for p in proposal['placements'] if p['id'] == obj['id'])
            require(source['footprint'] == placement['footprint'], 'era changes collision footprint')
        for pin in projection['objects']:
            obj = layout[pin['group']][pin['index']]
            require(obj['id'] == pin['id'] and all(type(obj[k]) is int and obj[k] == v
                    for k, v in pin['runtime'].items()), 'era changes canonical integer projection')
        destinations = [{k: obj[k] for k in ['id', 'x', 'y', 'width', 'height', 'depth_y']}
                        for obj in scenery['objects']]
        if reference is None:
            reference = destinations
        require(destinations == reference, 'era changes shared visual destinations')
        runtime = []
        descriptors = [scenery['ground']] + scenery['atlases']
        require(len(resources['resources']) == len(descriptors) == 2, 'unexpected runtime resources')
        for pin, descriptor in zip(resources['resources'], descriptors):
            raw = published_input(pin['path'], ART_COMMIT)
            require(all(pin[k] == descriptor[k] for k in ['path', 'sha256', 'bytes', 'width_px', 'height_px'])
                    and sha(raw) == pin['sha256'] and len(raw) == pin['bytes'], 'runtime scenery/resource mismatch')
            require(png_size(raw) == (pin['width_px'], pin['height_px']), 'runtime dimension mismatch')
            runtime.append({k: pin[k] for k in ['path', 'sha256', 'bytes', 'width_px', 'height_px']})
        require(runtime[1]['sha256'] not in atlas_hashes, 'distinct eras must not alias the same atlas')
        atlas_hashes.add(runtime[1]['sha256'])
        count = sum(p['bytes'] for p in runtime)
        pixels = sum(p['width_px'] * p['height_px'] for p in runtime)
        require(count <= MAX_SCENERY and pixels <= MAX_SCENERY, 'era exceeds incumbent C06 budget')
        budgets[theme_id] = {'bytes': count, 'pixels': pixels, 'max_bytes': MAX_SCENERY, 'max_pixels': MAX_SCENERY}
        scenes.append({'theme_id': theme_id, 'scenery_source': file_pin(scenery_path, files[scenery_path]),
            'scene_sources': [file_pin(manifest_path, manifest_data), file_pin(projection_path, projection_data)]
                + [file_pin(path, raw) for path, raw in files.items()
                   if path not in [scenery_path, runtime[1]['path']]],
            'runtime_resources': runtime})
    binding = {
        'format': 'MCR_SOURCE_BOUND_SPATIAL_RELEASE', 'version': 2,
        'source_commit': SOURCE_COMMIT, 'art_commit': ART_COMMIT, 'tilemap_commit': TILED_COMMIT,
        'map_id': MAP_ID, 'map_revision': 2, 'map_content_hash': manifest['content_hash'],
        'geometry_hash': GEOMETRY_HASH, 'geometry_source': file_pin(geometry_path, geometry_data),
        'tilemap_source': file_pin(TILED_PATH, tiled_data), 'scenes': scenes,
    }
    readiness = {
        'task': 'BE-SPATIAL-RELEASE', 'state': 'NOT_ADMITTED',
        'source_commit': SOURCE_COMMIT, 'art_commit': ART_COMMIT, 'tilemap_commit': TILED_COMMIT,
        'map': {key: manifest[key] for key in ['map_id', 'map_revision', 'content_hash', 'geometry_hash']},
        'skins': {theme: 'CONTRACT_READY' for theme in budgets},
        'blockers': ['UNREVIEWED_SOURCE_PROVENANCE', 'PRODUCT_BOOTSTRAP_NOT_CONFIGURED', 'ART_FIDELITY_NOT_ACCEPTED'],
        'scenery_budget': budgets, 'fractional_depth_count': 0,
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
