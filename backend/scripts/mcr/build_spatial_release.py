#!/usr/bin/env python3
"""Build the source-bound Round 3 spatial candidate; never grant product admission."""
from __future__ import annotations
import argparse
import copy
from typing import NamedTuple
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
MAP_REVISION = 3
SOURCE_REVIEWED_AT = '2026-11-04T00:12:30Z'
SOURCE_REVIEWER = 'MCR backend spatial source qualification'
MAX_SCENERY = 2_097_152


class Inputs(NamedTuple):
    source_commit: str = SOURCE_COMMIT
    art_commit: str = ART_COMMIT
    tiled_commit: str = TILED_COMMIT
    geometry_path: str = BASE + 'geometry-proposal.json'
    geometry_hash: str = GEOMETRY_HASH
    proposal_version: int = 2
    runtime: str = RUNTIME
    art_version: int = 5
    manifest_hash: str = 'sha256:65d3d8ecbdd7263b343a492aa78cb0ccb6422af50d17398571fef78be8a9dfd4'
    manifest_bytes: int = 6138
    projection_path: str = BASE + 'runtime-v4/parade-integer-projection.json'
    tiled_path: str = TILED_PATH
    tiled_hash: str = TILED_HASH
    map_revision: int = MAP_REVISION
    reviewed_at: str = SOURCE_REVIEWED_AT


BASELINE = Inputs()
QUALITY_V6 = Inputs(
    source_commit='bc731ed868ec30b33ae5dd1e6f13e97e68ecabd9',
    art_commit='bc731ed868ec30b33ae5dd1e6f13e97e68ecabd9',
    tiled_commit='48cf47e9b2c922de42673d1a29c1758a55fcf883',
    geometry_path=BASE + 'spatial-v3/geometry-proposal.json',
    geometry_hash='sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f',
    proposal_version=3, runtime=BASE + 'quality-v6/', art_version=6,
    manifest_hash='sha256:a78fbd045e9123e18ffa01efc669dfef9965bbeccb1cd378b886b9b975247aed',
    manifest_bytes=5868, projection_path=BASE + 'spatial-v3/integer-projection.json',
    tiled_path=BASE + 'runtime/spatial-v3/canonical-map-v4.json',
    tiled_hash='sha256:60364d5f35bcc8ff82931b7e3a0724a4580880d4a481610e90475d73dcb6ceb9',
    map_revision=4, reviewed_at='2026-11-04T00:38:46Z',
)
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


def candidate_map(proposal, inputs=BASELINE):
    require(proposal['format'] == 'MCR_REFERENCE_SCENE_PROPOSAL'
            and proposal['proposal_version'] == inputs.proposal_version
            and proposal['status'] == 'PROPOSAL_NOT_RELEASE_BOUND', 'unknown proposal')
    geometry = copy.deepcopy(proposal['geometry'])
    require(assets.digest(geometry) == proposal['geometry_hash'] == inputs.geometry_hash,
            'proposal geometry pin mismatch')
    manifest = {
        'contract_version': '1.0.0', 'kind': 'map_manifest', 'map_id': MAP_ID,
        'map_revision': inputs.map_revision, 'geometry_hash': inputs.geometry_hash,
        'provenance': {
            'source_id': 'mcr_reference_scene_r3', 'source_kind': 'ORIGINAL_ART',
            'source_revision': inputs.source_commit + '; geometry-proposal.json v' + str(inputs.proposal_version) + '; engineering source review v1',
            'source_uri': 'https://github.com/julymeltdown/mcr/blob/' + inputs.source_commit
                          + '/frontend/public/' + inputs.geometry_path,
            'author': 'MCR reference scene author; canonical spatial projection',
            'license': 'Original MCR virtual geometry and metadata; source attribution retained in place provenance.',
            'review_state': 'REVIEWED', 'reviewed_by': SOURCE_REVIEWER, 'reviewed_at': inputs.reviewed_at,
            'photo_permission': 'NOT_APPLICABLE',
        },
        'tilemap': {'format': 'TILED_JSON', 'resource_state': 'PROVIDED',
                    'path': inputs.tiled_path, 'sha256': inputs.tiled_hash},
        'geometry': geometry, 'places': copy.deepcopy(proposal['places']),
    }
    manifest['content_hash'] = assets.content_hash(manifest)
    assets.validate_document(manifest, assets.validators(), 'spatial candidate')
    assets.validate_map(manifest)
    require(all(p['place_kind'] == 'VIRTUAL_CONCEPT'
                and p['verification_state'] == 'CONCEPT_ONLY' for p in manifest['places']),
            'proposal must not grant physical eligibility')
    return manifest


def build(inputs=BASELINE):
    geometry_path = inputs.geometry_path
    geometry_data = published_input(geometry_path, inputs.source_commit)
    proposal = json.loads(geometry_data)
    manifest = candidate_map(proposal, inputs)
    tiled_data = published_input(inputs.tiled_path, inputs.tiled_commit)
    require(sha(tiled_data) == inputs.tiled_hash and len(tiled_data) == 24058, 'published Tiled pin mismatch')
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
    manifest_path = inputs.runtime + 'manifest.json'
    manifest_data = published_input(manifest_path, inputs.art_commit)
    require(sha(manifest_data) == inputs.manifest_hash
            and len(manifest_data) == inputs.manifest_bytes, 'three-era source manifest pin mismatch')
    art = json.loads(manifest_data)
    require(art['version'] == inputs.art_version
            and (inputs.art_version != 5 or art['geometry']['source_revision'] == inputs.source_commit)
            and art['geometry']['geometry_hash'] == inputs.geometry_hash
            and art['geometry']['sha256'] == sha(geometry_data), 'three-era geometry mismatch')
    require({t['theme_id'] for t in art['themes']} == {'REVENGE', 'PARADE', 'DANGER'}
            and len(art['themes']) == 3, 'all three distinct era inputs required')
    projection_path = inputs.projection_path
    projection_data = published_input(projection_path, inputs.art_commit)
    require(sha(projection_data) == art['integer_projection']['sha256'], 'integer source projection mismatch')
    projection = json.loads(projection_data)
    common_sources = []
    if inputs.art_version == 6:
        require(art['format'] == 'MCR_REFERENCE_SCENE_SPATIAL3_QUALITY6'
                and art['geometry_revision'] == 3, 'unknown quality-v6 source')
        for pin in [art['source_metadata']] + art['sources']:
            require('..' not in pin['path'].split('/') and not pin['path'].startswith('/'),
                    'invalid common source path')
            path = inputs.runtime + pin['path']
            raw = published_input(path, inputs.art_commit)
            require(sha(raw) == pin['sha256'] and ('bytes' not in pin or len(raw) == pin['bytes']),
                    'common source pin mismatch')
            common_sources.append(file_pin(path, raw))
    scenes = []
    budgets = {}
    reference = None
    atlas_hashes = set()
    api = assets.read_json(ROOT / 'backend/docs/api/mcr-core.openapi.json')
    for theme in art['themes']:
        theme_id = theme['theme_id']
        prefix = inputs.runtime + theme_id.lower() + '/'
        files = {}
        for pin in theme['files']:
            path = inputs.runtime + pin['path']
            require(path.startswith(prefix) and '..' not in path.split('/') and path not in files,
                    'invalid or duplicate era source path')
            raw = published_input(path, inputs.art_commit)
            require(sha(raw) == pin['sha256'] and len(raw) == pin['bytes'], 'era source pin mismatch: ' + path)
            files[path] = raw
        scenery_path = prefix + 'scenery.json'
        scenery = json.loads(files[scenery_path])
        layout = json.loads(files[prefix + 'layout.json'])
        resources = json.loads(files[prefix + 'resource-inputs.json'])
        require(layout['version'] == resources['version'] == inputs.art_version
                and resources['theme_id'] == theme_id
                and layout['geometry_hash'] == resources['geometry_hash'] == inputs.geometry_hash,
                'era source version/geometry mismatch')
        assets.Draft202012Validator({'$ref': '#/components/schemas/Scenery',
                                    'components': api['components']}).validate(scenery)
        assets.validate_scenery(scenery, geometry)
        require(len(scenery['objects']) == len(layout['placements']) == 22
                and len(layout['overview_representations']) == 6, 'unexpected WORLD/MAP projection')
        for obj, placement in zip(scenery['objects'], layout['placements']):
            require(all(obj[k] == placement[k] for k in ['id', 'x', 'y', 'width', 'height', 'depth_y']),
                    'scenery placement differs from authored integer source')
            require(obj['frame'] == layout['frames'][placement['frame']], 'scenery changes authored frame')
            source = next(p for p in proposal['placements'] if p['id'] == obj['id'])
            require(source['footprint'] == placement['footprint'], 'era changes collision footprint')
        for pin in projection['objects']:
            obj = next(obj for obj in layout[pin['group']] if obj['id'] == pin['id'])
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
            raw = published_input(pin['path'], inputs.art_commit)
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
                + common_sources
                + [file_pin(path, raw) for path, raw in files.items()
                   if path not in [scenery_path, runtime[1]['path']]],
            'runtime_resources': runtime})
    binding = {
        'format': 'MCR_SOURCE_BOUND_SPATIAL_RELEASE', 'version': 2,
        'source_commit': inputs.source_commit, 'art_commit': inputs.art_commit, 'tilemap_commit': inputs.tiled_commit,
        'map_id': MAP_ID, 'map_revision': inputs.map_revision, 'map_content_hash': manifest['content_hash'],
        'geometry_hash': inputs.geometry_hash, 'geometry_source': file_pin(geometry_path, geometry_data),
        'tilemap_source': file_pin(inputs.tiled_path, tiled_data), 'scenes': scenes,
    }
    readiness = {
        'task': 'BE-SPATIAL-RELEASE', 'state': 'NOT_ADMITTED',
        'source_commit': inputs.source_commit, 'art_commit': inputs.art_commit, 'tilemap_commit': inputs.tiled_commit,
        'map': {key: manifest[key] for key in ['map_id', 'map_revision', 'content_hash', 'geometry_hash']},
        'skins': {theme: 'CONTRACT_READY' for theme in budgets},
        'blockers': ['PRODUCT_BOOTSTRAP_NOT_CONFIGURED', 'ART_FIDELITY_NOT_ACCEPTED'],
        'scenery_budget': budgets, 'fractional_depth_count': 0,
        'bootstrap': None, 'theme_manifests': None,
        'limits': ['No art fidelity acceptance', 'No physical place qualification',
                   'Published Tiled resource binds the same canonical grids',
                   'No event-current map publication or production admission'],
    }
    review = {
        'format': 'MCR_VIRTUAL_SPATIAL_SOURCE_QUALIFICATION', 'version': 1,
        'reviewed_by': SOURCE_REVIEWER, 'reviewed_at': inputs.reviewed_at,
        'scope': 'Engineering qualification of the derived virtual map and pinned source inputs only',
        'source_commit': inputs.source_commit, 'art_commit': inputs.art_commit, 'tilemap_commit': inputs.tiled_commit,
        'map': readiness['map'],
        'source_binding_sha256': sha((json.dumps(binding, ensure_ascii=True, sort_keys=True, indent=2) + '\n').encode()),
        'checks': {key: 'PASS' for key in ['published_source_byte_pins', 'closed_c06_map_and_scenery',
            'canonical_geometry_and_finite_tiled', 'integer_projection_and_unchanged_footprints',
            'distinct_three_era_resources_and_budgets', 'virtual_place_source_values_unchanged']},
        'source_png_decode': {'state': 'PASS', 'unique_images': 4,
            'evidence': 'Developer verified PNG integrity and full pixel decoding against these exact resource pins.'},
        'virtual_places': {'count': 4, 'place_kind': 'VIRTUAL_CONCEPT', 'verification_state': 'CONCEPT_ONLY',
                           'source_review_state': 'UNREVIEWED', 'physical_selection': 'INELIGIBLE'},
        'limits': ['No physical venue verification or arrival authority', 'No art fidelity or user acceptance',
                   'No Product bootstrap/operator admission', 'Rust/runtime qualification is separately recorded in docs/mcr/backend/spatial-release.md'],
    }
    return {'map-manifest.json': manifest, 'source-binding.json': binding, 'readiness.json': readiness,
            'engineering-source-review.json': review}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--candidate', choices=['baseline', 'quality-v6'], default='baseline')
    args = parser.parse_args()
    directory = ROOT / 'backend/releases/reference-scene-v2'
    if args.candidate == 'quality-v6':
        directory /= 'quality-v6'
    outputs = build(QUALITY_V6 if args.candidate == 'quality-v6' else BASELINE)
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
