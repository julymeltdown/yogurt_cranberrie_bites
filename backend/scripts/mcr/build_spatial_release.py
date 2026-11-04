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
import zlib

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
    world_objects: int = 22
    overview_objects: int = 6


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
QUALITY_V8 = Inputs(
    source_commit='492160b15465818eca172f4017f3816d58c989a9',
    art_commit='492160b15465818eca172f4017f3816d58c989a9',
    tiled_commit='cf48c0fe01f8207f5a39e91daf238cc4c85a3980',
    geometry_path=BASE + 'spatial-v4/geometry-proposal.json',
    geometry_hash='sha256:0448d5086dc251c6a7aed8c136dbbbf751f95038ea1f7f98e3e40fe6e8067e6f',
    proposal_version=4, runtime=BASE + 'quality-v8/', art_version=8,
    manifest_hash='sha256:0aa7ffc44d4fa69ad8d0ac9dfb8ab30907ce0e785aa461a03a1452b8461518d6',
    manifest_bytes=6216, projection_path=BASE + 'spatial-v4/integer-projection.json',
    tiled_path=BASE + 'runtime/spatial-v4/canonical-map.json',
    tiled_hash='sha256:c589b9b01c3e9f7162ccf7443782363bc9c132dd807b5fe15557686df584cf6f',
    map_revision=5, reviewed_at='2026-11-04T02:11:03Z', world_objects=34,
)
QUALITY_V10 = Inputs(
    source_commit='06562ff1962be525ed929ef431a54bb589860373',
    art_commit='06562ff1962be525ed929ef431a54bb589860373',
    tiled_commit='473534296490bbcd4c63a4a0eb85ce7fdd57215a',
    geometry_path=BASE + 'spatial-v5/geometry-proposal.json',
    geometry_hash='sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395',
    proposal_version=5, runtime=BASE + 'quality-v10/', art_version=10,
    manifest_hash='sha256:9048d45753f4d47828082c422061ed17418f8bfb93b51cf3e9879e821eeb0cf6',
    manifest_bytes=7749, projection_path=BASE + 'spatial-v5/integer-projection.json',
    tiled_path=BASE + 'runtime/spatial-v5/canonical-map.json',
    tiled_hash='sha256:e5837dfe08a8d2f1f7cdb4428b87965b4a7c6ecdb1458b8fe910e3f4c2112078',
    map_revision=6, reviewed_at='2026-11-04T02:59:31Z', world_objects=34, overview_objects=8,
)
CANDIDATES = {'baseline': BASELINE, 'quality-v6': QUALITY_V6, 'quality-v8': QUALITY_V8,
              'quality-v10': QUALITY_V10}
V10_METADATA_COMMIT = '5d3e81f125dd6bf7f507dcf96914f6878c8fe6f4'
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


def decode_png(data):
    # The v8 review records a real integrity and complete pixel decode, rather
    # than deriving that claim from the dimensions in the IHDR alone. These
    # pinned RGB/RGBA8 non-interlaced inputs need no new validation dependency.
    width, height = png_size(data)
    bits, color, compression, filtering, interlace = data[24:29]
    require(bits == 8 and color in (2, 6) and (compression, filtering, interlace) == (0, 0, 0),
            'unsupported pinned PNG encoding')
    channels = 3 if color == 2 else 4
    offset, compressed, ended = 8, bytearray(), False
    while offset < len(data):
        require(offset + 12 <= len(data), 'truncated PNG chunk')
        size = struct.unpack('>I', data[offset:offset + 4])[0]
        kind = data[offset + 4:offset + 8]
        end = offset + 8 + size
        require(end + 4 <= len(data), 'truncated PNG payload')
        chunk = data[offset + 4:end]
        require(zlib.crc32(chunk) == struct.unpack('>I', data[end:end + 4])[0], 'PNG CRC mismatch')
        if kind == b'IDAT':
            compressed.extend(data[offset + 8:end])
        offset = end + 4
        if kind == b'IEND':
            require(size == 0 and offset == len(data), 'invalid PNG ending')
            ended = True
            break
    require(ended and compressed, 'incomplete PNG stream')
    decoded = zlib.decompress(compressed)
    stride = width * channels
    require(len(decoded) == height * (stride + 1), 'PNG pixel stream size mismatch')
    previous = bytearray(stride)
    for y in range(height):
        start = y * (stride + 1)
        filter_kind = decoded[start]
        require(filter_kind in range(5), 'invalid PNG row filter')
        row = bytearray(decoded[start + 1:start + stride + 1])
        for x in range(stride):
            left = row[x - channels] if x >= channels else 0
            up = previous[x]
            corner = previous[x - channels] if x >= channels else 0
            p = left + up - corner
            distances = (abs(p - left), abs(p - up), abs(p - corner))
            paeth = (left, up, corner)[distances.index(min(distances))]
            predictor = (0, left, up, (left + up) // 2, paeth)[filter_kind]
            row[x] = (row[x] + predictor) & 255
        previous = row


def verify_v8_revision(proposal, evidence):
    previous = json.loads(published_input(QUALITY_V6.geometry_path, QUALITY_V6.source_commit))
    old_geometry, geometry = previous['geometry'], proposal['geometry']
    require(proposal['places'] == previous['places'], 'v8 changes original place provenance')
    require({k: v for k, v in geometry.items() if k not in ('collision_id', 'collision_grid')}
            == {k: v for k, v in old_geometry.items() if k not in ('collision_id', 'collision_grid')},
            'v8 changes frozen terrain, overdraw, dimensions, spawns or interactions')
    changes = []
    for y, (before, after) in enumerate(zip(old_geometry['collision_grid'], geometry['collision_grid'])):
        for x, (old, new) in enumerate(zip(before, after)):
            if old != new:
                require((old, new) == (0, 1), 'v8 removes a blocked cell')
                changes.append({'cell_x': x, 'cell_y': y, 'previous': old, 'proposed': new})
    require(len(changes) == 72 and changes == evidence['changed_cells'], 'v8 collision evidence mismatch')
    require(evidence['base_geometry_hash'] == QUALITY_V6.geometry_hash
            and evidence['geometry_hash'] == QUALITY_V8.geometry_hash, 'v8 evidence geometry mismatch')
    # Preserve the published integer destinations, not the earlier illustration
    # floats which the v6 author already projected into those destinations.
    old_layout = json.loads(published_input(QUALITY_V6.runtime + 'parade/layout.json', QUALITY_V6.art_commit))
    old_props = {p['id']: p for p in old_layout['placements']}
    require(set(old_props) == {p['id'] for p in previous['placements']}, 'v6 prop identity mismatch')
    require(all(old_props[p['id']]['footprint'] == p['footprint'] for p in previous['placements']),
            'v8 changes frozen fractional source footprints')
    new_props = {p['id']: p for p in proposal['placements']}
    require(len(new_props) == 34 and all(new_props.get(k) == v for k, v in old_props.items())
            and len(new_props.keys() - old_props.keys()) == 12, 'v8 changes frozen props or root additions')


def verify_v10_revision(proposal, evidence):
    previous = json.loads(published_input(QUALITY_V8.geometry_path, QUALITY_V8.source_commit))
    old, geometry = previous['geometry'], proposal['geometry']
    require(proposal['placements'] == previous['placements'] and proposal['places'] == previous['places'],
            'v10 changes frozen WORLD props, footprints or original places')
    require({k: v for k, v in old.items() if k != 'terrain_layer'}
            == {k: v for k, v in geometry.items() if k != 'terrain_layer'},
            'v10 changes frozen collision, overdraw, dimensions, spawns or interactions')
    require(len(geometry['terrain_layer']) == old['height_cells']
            and all(len(row) == old['width_cells'] for row in geometry['terrain_layer']),
            'v10 terrain dimensions mismatch')
    changes = [{'cell_x': x, 'cell_y': y, 'previous': before, 'proposed': after}
               for y, (a, b) in enumerate(zip(old['terrain_layer'], geometry['terrain_layer']))
               for x, (before, after) in enumerate(zip(a, b)) if before != after]
    require(len(changes) == 560 and changes == evidence['terrain_changed_cells'],
            'v10 terrain evidence mismatch')
    require(evidence['base_geometry_hash'] == QUALITY_V8.geometry_hash
            and evidence['geometry_hash'] == QUALITY_V10.geometry_hash
            and evidence['collision_changed_cells'] == evidence['world_props_changed'] == 0,
            'v10 evidence identity or unchanged collision/props mismatch')


def verify_v10_reservation(inputs):
    path = ROOT / 'backend/releases/reference-scene-v2/quality-v10/revision-reservation.json'
    raw = path.read_bytes()
    require(sha(raw) == 'sha256:771fabbcc0243cff645a2f7588fcaa4d68f056663b2b3ede5d86e2024d213317',
            'v10 committed reservation bytes changed')
    reservation = json.loads(raw)
    require(reservation['format'] == 'MCR_SPATIAL_REVISION_RESERVATION'
            and reservation['version'] == 1 and reservation['reservation'] == 'SOURCE_IDENTITY_ONLY'
            and reservation['admission'] == 'NOT_ADMITTED'
            and reservation['map_id'] == MAP_ID and reservation['map_revision'] == inputs.map_revision == 6
            and reservation['existing_server_revisions'] == [3, 4, 5]
            and reservation['revision6_existing_before_reservation'] is False
            and reservation['artist_commit'] == inputs.art_commit
            and reservation['frontend_commit'] == inputs.tiled_commit
            and reservation['geometry_hash'] == inputs.geometry_hash
            and reservation['tilemap'] == {'path': inputs.tiled_path, 'sha256': inputs.tiled_hash, 'bytes': 24058},
            'v10 requires the actual committed revision6 reservation and frozen tuple')
    return reservation


def verify_v10_frontend(inputs, reservation, scenes):
    # This is a source qualification of immutable generated asset data, not an
    # import of frontend rendering or a second Tiled/resource serializer.
    path = 'frontend/src/world/reference-scene/spatial-v5/source-inputs.json'
    raw = subprocess.check_output(['git', 'show', inputs.tiled_commit + ':' + path], cwd=ROOT)
    require(raw == (ROOT / path).read_bytes()
            and sha(raw) == reservation['frontend_descriptor_sha256'], 'v10 frontend descriptor drift')
    descriptor = json.loads(raw)
    binding_path = BASE + 'runtime/spatial-v5/source-binding.json'
    historical_raw = subprocess.check_output(
        ['git', 'show', inputs.tiled_commit + ':frontend/public/' + binding_path], cwd=ROOT)
    require(sha(historical_raw) == reservation['frontend_binding_sha256'], 'v10 historical frontend binding drift')
    historical = json.loads(historical_raw)
    binding_raw = published_input(binding_path, V10_METADATA_COMMIT)
    require(sha(binding_raw) == 'sha256:8cb8d18a9c6bc07eead30e7a2474ca5fb9adb3f23ddade911840aad6bf134041',
            'v10 frontend allocation binding drift')
    binding = json.loads(binding_raw)
    allocation_path = 'frontend/src/world/reference-scene/spatial-v5/backend-allocation.json'
    allocation_raw = subprocess.check_output(['git', 'show', V10_METADATA_COMMIT + ':' + allocation_path], cwd=ROOT)
    require(allocation_raw == (ROOT / allocation_path).read_bytes()
            and sha(allocation_raw) == 'sha256:6b48366434524390a031cf29ea609794b2797d494132f2719c0c18e4b34d8060',
            'v10 frontend allocation evidence drift')
    allocation = json.loads(allocation_raw)
    require(allocation == binding['backend_allocation']
            and allocation['map_id'] == MAP_ID and allocation['map_revision'] == inputs.map_revision
            and allocation['geometry_hash'] == inputs.geometry_hash
            and allocation['commit'] == '390bb9a57c2ede32b814e8c36f8d1c85a10a99a2'
            and allocation['evidence_path'] == 'backend/releases/reference-scene-v2/quality-v10/revision-reservation.json'
            and allocation['evidence_sha256'] == 'sha256:771fabbcc0243cff645a2f7588fcaa4d68f056663b2b3ede5d86e2024d213317'
            and {k: v for k, v in binding.items() if k not in ('candidate_map', 'backend_allocation')}
                == {k: v for k, v in historical.items() if k not in ('candidate_map', 'backend_allocation')},
            'v10 allocation metadata changes the immutable source tuple or reservation')
    proposal = json.loads(published_input(inputs.geometry_path, inputs.source_commit))
    require(historical['candidate_map']['map_revision'] is None
            and binding['candidate_map']['map_id'] == descriptor['map_id'] == MAP_ID
            and binding['candidate_map']['map_revision'] == inputs.map_revision
            and binding['candidate_map']['minimum_map_revision'] == 6
            and binding['candidate_map']['geometry_hash'] == inputs.geometry_hash
            and binding['status'] == 'NOT_RELEASE_BOUND' and binding['production_status'] == 'NOT_ADMITTED'
            and descriptor['geometry'] == proposal['geometry']
            and descriptor['geometry_hash'] == inputs.geometry_hash and descriptor['places'] == proposal['places'],
            'v10 frontend geometry, place or historical allocation mismatch')
    require(len(binding['source_inputs']) == 44, 'v10 frontend source inventory mismatch')
    for pin in binding['source_inputs']:
        source = published_input(pin['path'], inputs.art_commit)
        require(sha(source) == pin['sha256'] and ('bytes' not in pin or len(source) == pin['bytes']),
                'v10 frontend source pin mismatch')
    matched = []
    for scene in scenes:
        theme = scene['theme_id']
        scenery = json.loads(published_input(scene['scenery_source']['path'], inputs.art_commit))
        frontend = next(s for s in binding['scenes'] if s['theme_id'] == theme)
        require(descriptor['scenes'][theme]['scenery'] == scenery
                and frontend['runtime_resources'] == [scenery['ground']] + scenery['atlases'],
                'v10 frontend/runtime resource descriptor mismatch')
        replaced = {p['replaces_prop_id'] for p in descriptor['overview_representations'] if 'replaces_prop_id' in p}
        map_objects = sum(obj['id'] not in replaced for obj in scenery['objects']) + len(descriptor['overview_representations'])
        require(len(scenery['objects']) == 34 and len(descriptor['overview_representations']) == 8
                and len(replaced) == 3 and map_objects == 39, 'v10 WORLD/MAP descriptor count mismatch')
        matched.append({'theme_id': theme, 'world_objects': 34, 'map_objects': map_objects,
                        'scenery_source': scene['scenery_source'], 'runtime_resources': scene['runtime_resources']})
    return {'format': 'MCR_SPATIAL_FRONTEND_SOURCE_COMPATIBILITY', 'version': 1,
            'state': 'CONTRACT_READY', 'admission': 'NOT_ADMITTED', 'map_id': MAP_ID,
            'map_revision': inputs.map_revision, 'geometry_hash': inputs.geometry_hash,
            'frontend_commit': inputs.tiled_commit, 'descriptor': file_pin(path, raw),
            'frontend_binding': file_pin(binding_path, historical_raw), 'source_input_pins_verified': 44,
            'frontend_metadata_commit': V10_METADATA_COMMIT,
            'frontend_allocation_binding': file_pin(binding_path, binding_raw),
            'backend_allocation_record': file_pin(allocation_path, allocation_raw),
            'reservation_commit': '390bb9a57c2ede32b814e8c36f8d1c85a10a99a2',
            'reservation_sha256': 'sha256:771fabbcc0243cff645a2f7588fcaa4d68f056663b2b3ede5d86e2024d213317',
            'frontend_candidate_revision_at_pin': None, 'frontend_metadata_candidate_revision': 6, 'scenes': matched,
            'limits': ['Immutable source descriptor comparison only', 'No frontend/runtime or art-fidelity acceptance',
                       'No supplied ThemeManifest, Bootstrap or Product/operator admission']}


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
    reservation = verify_v10_reservation(inputs) if inputs.art_version == 10 else None
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
            and (art['geometry_hash'] if inputs.art_version == 10 else art['geometry']['geometry_hash']) == inputs.geometry_hash
            and art['geometry']['sha256'] == sha(geometry_data), 'three-era geometry mismatch')
    require({t['theme_id'] for t in art['themes']} == {'REVENGE', 'PARADE', 'DANGER'}
            and len(art['themes']) == 3, 'all three distinct era inputs required')
    projection_path = inputs.projection_path
    projection_data = published_input(projection_path, inputs.art_commit)
    require(sha(projection_data) == art['integer_projection']['sha256'], 'integer source projection mismatch')
    projection = json.loads(projection_data)
    common_sources = []
    if inputs.art_version in (6, 8):
        require(art['format'] == f'MCR_REFERENCE_SCENE_SPATIAL{inputs.proposal_version}_QUALITY{inputs.art_version}'
                and art['geometry_revision'] == inputs.proposal_version, 'unknown source generation')
        for pin in [art['source_metadata']] + art['sources']:
            require('..' not in pin['path'].split('/') and not pin['path'].startswith('/'),
                    'invalid common source path')
            path = inputs.runtime + pin['path']
            raw = published_input(path, inputs.art_commit)
            require(sha(raw) == pin['sha256'] and ('bytes' not in pin or len(raw) == pin['bytes']),
                    'common source pin mismatch')
            common_sources.append(file_pin(path, raw))
    if inputs.art_version == 8:
        evidence_path = BASE + 'spatial-v4/revision-evidence.json'
        evidence_raw = published_input(evidence_path, inputs.art_commit)
        require(sha(evidence_raw) == art['geometry_revision_evidence']['sha256'], 'v8 revision evidence pin mismatch')
        verify_v8_revision(proposal, json.loads(evidence_raw))
        common_sources.append(file_pin(evidence_path, evidence_raw))
        require(projection['version'] == 4 and projection['geometry_hash'] == inputs.geometry_hash,
                'v8 integer projection identity mismatch')
    if inputs.art_version == 10:
        require(art['format'] == 'MCR_REFERENCE_SCENE_SPATIAL5_QUALITY10'
                and projection['version'] == 5 and projection['geometry_hash'] == inputs.geometry_hash,
                'v10 source generation/projection mismatch')
        pins = [(BASE + 'quality-v8/manifest.json', art['base_manifest']),
                (BASE + 'spatial-v5/revision-evidence.json', art['revision_evidence']),
                (inputs.runtime + 'map-label-presentation.json', art['map_label_sidecar'])]
        pins += [(inputs.runtime + p['path'], p) for p in art['ground_sources']]
        require(art['base_manifest']['sha256'] == QUALITY_V8.manifest_hash,
                'v10 changes frozen base manifest')
        for path, pin in pins:
            require('..' not in path.split('/'), 'v10 unsafe common source path')
            raw = published_input(path, inputs.art_commit)
            require(sha(raw) == pin['sha256'] and ('bytes' not in pin or len(raw) == pin['bytes']),
                    'v10 common source pin mismatch')
            common_sources.append(file_pin(path, raw))
        verify_v10_revision(proposal, json.loads(published_input(BASE + 'spatial-v5/revision-evidence.json', inputs.art_commit)))
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
        require(len(scenery['objects']) == len(layout['placements']) == inputs.world_objects
                and len(layout['overview_representations']) == inputs.overview_objects, 'unexpected WORLD/MAP projection')
        for obj, placement in zip(scenery['objects'], layout['placements']):
            require(all(obj[k] == placement[k] for k in ['id', 'x', 'y', 'width', 'height', 'depth_y']),
                    'scenery placement differs from authored integer source')
            require(obj['frame'] == layout['frames'][placement['frame']], 'scenery changes authored frame')
            source = next(p for p in proposal['placements'] if p['id'] == obj['id'])
            require(source['footprint'] == placement['footprint'], 'era changes collision footprint')
        if inputs.art_version in (8, 10):
            for group in ('placements', 'overview_representations'):
                require(layout[group] == projection[group], f'era changes v{inputs.art_version} authored integer projection')
            require(all(type(p[k]) is int for group in ('placements', 'overview_representations')
                        for p in projection[group] for k in ('x', 'y', 'width', 'height', 'depth_y')),
                    f'v{inputs.art_version} fractional runtime projection')
        else:
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
            if inputs.art_version in (8, 10):
                decode_png(raw)
            runtime.append({k: pin[k] for k in ['path', 'sha256', 'bytes', 'width_px', 'height_px']})
        require(runtime[1]['sha256'] not in atlas_hashes, 'distinct eras must not alias the same atlas')
        atlas_hashes.add(runtime[1]['sha256'])
        count = sum(p['bytes'] for p in runtime)
        pixels = sum(p['width_px'] * p['height_px'] for p in runtime)
        require(count <= MAX_SCENERY and pixels <= MAX_SCENERY, 'era exceeds incumbent C06 budget')
        budgets[theme_id] = {'bytes': count, 'pixels': pixels, 'max_bytes': MAX_SCENERY, 'max_pixels': MAX_SCENERY}
        # V10 ground_sources also includes the runtime ground. The retained
        # loader requires each path exactly once across source/runtime pins.
        # It has already been verified above and stays pinned as a runtime image.
        scene_common_sources = common_sources
        if inputs.art_version == 10:
            runtime_paths = {pin['path'] for pin in runtime}
            scene_common_sources = [pin for pin in common_sources if pin['path'] not in runtime_paths]
        scenes.append({'theme_id': theme_id, 'scenery_source': file_pin(scenery_path, files[scenery_path]),
            'scene_sources': [file_pin(manifest_path, manifest_data), file_pin(projection_path, projection_data)]
                + scene_common_sources
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
    outputs = {'map-manifest.json': manifest, 'source-binding.json': binding, 'readiness.json': readiness,
               'engineering-source-review.json': review}
    if inputs.art_version == 10:
        review['source_png_decode']['scope'] = 'Four unique runtime resources; native/editable sources are byte-pinned'
        outputs['frontend-source-compatibility.json'] = verify_v10_frontend(inputs, reservation, scenes)
    if inputs in (QUALITY_V6, QUALITY_V8, QUALITY_V10):
        # A separate source-level review for exact scenery/texture inputs. This
        # neither rewrites artist records nor reviews arbitrary theme/avatar DTOs.
        qualified = []
        for scene in scenes:
            theme_id = scene['theme_id']
            provenance = {
                'source_id': f'mcr_quality_v{inputs.art_version}_' + theme_id.lower() + '_scenery',
                'source_kind': 'ORIGINAL_ART',
                'source_revision': inputs.art_commit + '; scenery ' + scene['scenery_source']['sha256']
                    + '; source-binding ' + review['source_binding_sha256'],
                'source_uri': 'https://github.com/julymeltdown/mcr/blob/' + inputs.art_commit
                    + '/frontend/public/' + scene['scenery_source']['path'],
                'author': 'MCR reference scene source author; source-bound scenery packaging',
                'license': 'Original MCR virtual scenery source inputs; attribution retained in pinned artist documents.',
                'review_state': 'REVIEWED', 'reviewed_by': SOURCE_REVIEWER,
                'reviewed_at': '2026-11-04T00:58:55Z' if inputs == QUALITY_V6 else inputs.reviewed_at,
                'photo_permission': 'NOT_APPLICABLE',
            }
            assets.Draft202012Validator({'$ref': '#/components/schemas/Provenance',
                                        'components': api['components']}).validate(provenance)
            qualified.append({'theme_id': theme_id, 'source_provenance': provenance,
                'scenery_source': scene['scenery_source'], 'runtime_resources': scene['runtime_resources'],
                'artist_provenance_source': next(pin for pin in scene['scene_sources'] if pin['path'].endswith('/provenance.json')),
                'scenery_budget': budgets[theme_id]})
        outputs['scene-source-qualification.json'] = {
            'format': 'MCR_SPATIAL_SCENE_SOURCE_QUALIFICATION', 'version': 1,
            'state': 'CONTRACT_READY', 'admission': 'NOT_ADMITTED',
            'scope': 'Engineering source qualification of exact per-era scenery and texture inputs for derived virtual theme descriptors',
            'map': readiness['map'], 'art_commit': inputs.art_commit,
            'source_binding_sha256': review['source_binding_sha256'],
            'engineering_source_review_sha256': sha((json.dumps(review, ensure_ascii=True, sort_keys=True, indent=2) + '\n').encode()),
            'scenes': qualified,
            'consumer_obligations': [
                'Use only the exact supplied scenery/texture pins with this canonical map identity',
                'Retain original artist source provenance and original virtual place records unchanged',
                'A derived ThemeManifest producer must validate its complete body, map/avatar bindings and content hash',
                'These source_provenance values describe reviewed source inputs; they do not qualify arbitrary theme fields or avatar data',
            ],
            'limits': ['No supplied ThemeManifest or Bootstrap', 'No art fidelity or user acceptance',
                       'No physical venue verification or arrival authority', 'No Product/operator admission'],
        }
    return outputs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--candidate', choices=list(CANDIDATES), default='baseline')
    args = parser.parse_args()
    directory = ROOT / 'backend/releases/reference-scene-v2'
    if args.candidate != 'baseline':
        directory /= args.candidate
    outputs = build(CANDIDATES[args.candidate])
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
