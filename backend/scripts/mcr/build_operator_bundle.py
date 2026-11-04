#!/usr/bin/env python3
"""Derive an inert, source-bound operator candidate; never enable a capability."""
from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import uuid

from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
PUBLIC = ROOT / 'frontend/public'
SOURCE = ROOT / 'backend/releases/reference-scene-v2/quality-v6'
OUTPUT = ROOT / 'backend/releases/operator-bundle/quality-v6'
QUALIFICATION_SHA = '70c3d1ac268c6e5da39d30a20e01406fc7eb27668d6f0e6a66e1d87de15d7d07'
THEMES = {'REVENGE', 'PARADE', 'DANGER'}


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


assets = module('operator_assets', ROOT / 'scripts/mcr/validate-assets.py')
core = module('operator_core', ROOT / 'scripts/mcr/validate-core.py')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return 'sha256:' + hashlib.sha256(data).hexdigest()


def pin(path):
    data = path.read_bytes()
    return {'path': path.relative_to(ROOT).as_posix(), 'sha256': digest(data), 'bytes': len(data)}


def checked_resource(record):
    path = PUBLIC / record['path']
    require(path.resolve().is_relative_to(PUBLIC.resolve()), 'resource escapes public root')
    data = path.read_bytes()
    require(digest(data) == record['sha256'], 'source hash differs: ' + record['path'])
    if 'bytes' in record:
        require(len(data) == record['bytes'], 'source size differs: ' + record['path'])
    if 'width_px' in record:
        with Image.open(io.BytesIO(data)) as image:
            require(image.format == 'PNG', 'resource is not PNG')
            image.load()
            require(image.size == (record['width_px'], record['height_px']), 'decoded image dimensions differ')
    return data


def build():
    qualification_path = SOURCE / 'scene-source-qualification.json'
    require(hashlib.sha256(qualification_path.read_bytes()).hexdigest() == QUALIFICATION_SHA,
            'reviewed scene source qualification differs')
    qualification = assets.read_json(qualification_path)
    binding_path = SOURCE / 'source-binding.json'
    require(digest(binding_path.read_bytes()) == qualification['source_binding_sha256'], 'source binding differs')
    binding = assets.read_json(binding_path)
    map_path = SOURCE / 'map-manifest.json'
    map_ = assets.read_json(map_path)
    require(all(map_[key] == value for key, value in qualification['map'].items()), 'qualified map differs')
    for name in ['geometry_source', 'tilemap_source']:
        checked_resource(binding[name])
    proposal = json.loads(checked_resource(binding['geometry_source']))
    tiled = json.loads(checked_resource(binding['tilemap_source']))
    geometry = map_['geometry']
    require(proposal['geometry'] == geometry and proposal['places'] == map_['places'],
            'map geometry or original virtual places changed')
    layers = {layer['name']: layer for layer in tiled['layers']}
    for name, key in [('terrain', 'terrain_layer'), ('collision', 'collision_grid'), ('overdraw', 'overdraw_layer')]:
        require(layers[name]['data'] == [cell for row in geometry[key] for cell in row], 'Tiled grid differs')
    for scene in binding['scenes']:
        for record in [scene['scenery_source'], *scene['scene_sources'], *scene['runtime_resources']]:
            checked_resource(record)

    avatar_path = ROOT / 'contracts/fixtures/assets/selected-static/avatar-manifest.json'
    avatar = assets.read_json(avatar_path)
    require(avatar['representation'] == 'whole_character'
            and avatar['source']['source_pack_id'] == 'mcr_user_characters_20261103', 'selected avatar required')
    checked_resource(avatar['atlas']['texture'])
    source_release = json.loads(checked_resource({'path': avatar['source']['release_path'],
                                                'sha256': avatar['source']['release_sha256']}))
    require(source_release['source_pack_id'] == avatar['source']['source_pack_id'], 'selected source release differs')
    pack_path = ROOT / 'frontend/src/entities/avatar/character-source-pack.json'
    pack = assets.read_json(pack_path)
    source = next(row for row in pack['files'] if row['source_id'] == avatar['source']['source_id'])
    require('sha256:' + source['sha256'] == avatar['source']['source_sha256'], 'selected original source differs')
    checked_resource({'path': 'world/avatars/supplied-v1/' + source['file'],
                      'sha256': 'sha256:' + source['sha256'], 'bytes': source['byte_length']})

    palette_path = PUBLIC / 'world/themes/binding-inputs.json'
    palettes = {row['theme_id']: row['palette'] for row in assets.read_json(palette_path)['themes']}
    themes = []
    require({scene['theme_id'] for scene in qualification['scenes']} == THEMES, 'three eras required')
    for scene in qualification['scenes']:
        scenery = json.loads(checked_resource(scene['scenery_source']))
        checked_resource(scene['artist_provenance_source'])
        provenance = copy.deepcopy(scene['source_provenance'])
        require(provenance['source_kind'] == 'ORIGINAL_ART' and provenance['review_state'] == 'REVIEWED'
                and provenance['reviewed_by'] and provenance['reviewed_at'], 'reviewed source required')
        expected_resources = [scenery['ground'], *scenery['atlases']]
        require([{key: resource[key] for key in ['path', 'sha256', 'bytes', 'width_px', 'height_px']}
                 for resource in expected_resources] == scene['runtime_resources'], 'scenery resource set differs')
        for resource in expected_resources:
            checked_resource(resource)
        ground = scenery['ground']
        tileset = tiled['tilesets'][0]
        require(len(tiled['tilesets']) == 1 and tileset['image'] == 'mcr-theme.png'
                and (tileset['imagewidth'], tileset['imageheight']) == (ground['width_px'], ground['height_px'])
                and (tileset['tilewidth'], tileset['tileheight']) == (geometry['tile_width_px'], geometry['tile_height_px']),
                'canonical Tiled texture binding differs')
        theme = {
            'contract_version': '1.0.0', 'kind': 'theme_manifest',
            'theme_id': scene['theme_id'], 'theme_revision': 1, 'provenance': provenance,
            'map_id': map_['map_id'], 'map_revision': map_['map_revision'],
            'map_content_hash': map_['content_hash'], 'geometry_hash': map_['geometry_hash'],
            'collision_id': map_['geometry']['collision_id'],
            'place_ids': sorted(place['place_id'] for place in map_['places']),
            'interaction_ids': sorted(item['interaction_id'] for item in map_['geometry']['interactions']),
            'avatar_manifest_id': avatar['manifest_id'], 'avatar_manifest_version': avatar['manifest_version'],
            'avatar_content_hash': avatar['content_hash'],
            'texture': {'image_state': 'PROVIDED', **{key: ground[key] for key in ['path', 'sha256', 'width_px', 'height_px']}},
            'palette': palettes[scene['theme_id']], 'scenery': scenery,
        }
        theme['content_hash'] = assets.content_hash(theme)
        themes.append(theme)
    themes.sort(key=lambda theme: theme['theme_id'])
    config = next(preset['config'] for preset in avatar['presets']
                  if preset['preset_id'] == avatar['default_preset_id'])
    bundle = {'avatar': avatar, 'map': map_, 'config': config}
    bundle.update({theme['theme_id']: theme for theme in themes})
    assets.validate_bundle(bundle)

    # These identities name an inert validation candidate, not an admitted event
    # or a current participant. Runtime binding replaces private request fields.
    event_id = str(uuid.uuid5(uuid.NAMESPACE_URL, 'https://contracts.mcr.invalid/operator/quality-v6'))
    actor_id = str(uuid.uuid5(uuid.NAMESPACE_URL, 'https://contracts.mcr.invalid/operator/validation-actor'))
    appearance = {'revision': 1, 'config': copy.deepcopy(config)}
    bootstrap = {
        'contract_version': '0.1.0',
        'event': {'event_id': event_id, 'slug': 'mcr-quality-v6-candidate',
                  'title': 'MCR source-bound validation candidate', 'phase': 'OPEN', 'timezone': 'Asia/Seoul', 'revision': 1},
        'participant': {'event_id': event_id, 'user_id': actor_id, 'revision': 1,
                        'profile': {'user_id': actor_id, 'display_name': 'Validation participant', 'bio': None,
                                    'avatar': copy.deepcopy(appearance)},
                        'preferences': {'theme': 'REVENGE', 'low_power': False, 'reduced_motion': False, 'sound_enabled': False},
                        'age_status': 'UNKNOWN'},
        'avatar': appearance, 'avatar_manifest': avatar, 'map_manifest': map_, 'theme_manifests': themes,
        'capabilities': {name: {'available': False, 'reason': 'FEATURE_DISABLED'}
                         for name in ['world', 'feed', 'meet', 'gifts', 'wall', 'social_sharing', 'mutations']},
        'server_time': qualification['scenes'][0]['source_provenance']['reviewed_at'],
    }
    core.validate(bootstrap, 'Bootstrap', assets.read_json(ROOT / 'contracts/generated/mcr-core.openapi.json'))
    documents = {f'theme-{theme["theme_id"].lower()}.json': theme for theme in themes}
    documents['bootstrap.json'] = bootstrap
    result = {name: (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=True) + '\n').encode()
              for name, value in documents.items()}
    manifest = {
        'format': 'MCR_OPERATOR_BUNDLE_CANDIDATE', 'version': 1, 'state': 'CONTRACT_READY',
        'admission': 'NOT_ADMITTED', 'capabilities_enabled': False,
        'source_qualification': pin(qualification_path), 'map': pin(map_path), 'avatar': pin(avatar_path),
        'generator': pin(Path(__file__).resolve()),
        'avatar_source_registry': pin(pack_path), 'source_binding': pin(binding_path),
        'palette_source': pin(palette_path), 'resource_root': 'frontend/public',
        'outputs': [{'path': (OUTPUT / name).relative_to(ROOT).as_posix(), 'sha256': digest(data), 'bytes': len(data)}
                    for name, data in sorted(result.items())],
        'checks': ['C03 Bootstrap schema and invariants', 'C06 complete bundle schema/hash/bindings',
                   'exact reviewed source qualification and source binding', 'all scene source hashes/sizes',
                   'PNG full decode/dimensions/hash', 'selected avatar original/derived source identity'],
        'not_certified_by_builder': ['Rust from_source_bound_files constructor', 'Gateway HTTP', 'operator admission', 'visual fidelity acceptance'],
        'limits': ['Source provenance applies to exact supplied scene inputs only.',
                   'Original virtual places remain UNREVIEWED/VIRTUAL_CONCEPT/CONCEPT_ONLY.',
                   'The validation event/actor are inert template identities, not real admission or authorization.',
                   'No original source, artwork, map, avatar manifest or fixture is modified.'],
    }
    result['bundle.json'] = (json.dumps(manifest, indent=2, sort_keys=True) + '\n').encode()
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    try:
        result = build()
        for name, data in result.items():
            destination = OUTPUT / name
            if args.check:
                require(destination.is_file() and destination.read_bytes() == data, 'generated candidate drift: ' + name)
            else:
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(data)
        print(json.dumps({'result': 'PASS', 'outputs': len(result), 'admission': 'NOT_ADMITTED',
                          'rust_constructor': 'NOT_RUN', 'gateway_http': 'NOT_RUN'}))
        return 0
    except (OSError, ValueError, KeyError, StopIteration) as error:
        print('OPERATOR BUNDLE FAIL: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
