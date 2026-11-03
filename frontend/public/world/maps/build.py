#!/usr/bin/env python3
"""Original AR02 virtual courtyard. No physical path/footprint is traced."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]


def dump(name, value):
    (HERE / name).write_text(json.dumps(value, indent=2) + '\n')


def sha(raw):
    return 'sha256:' + hashlib.sha256(raw).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=True, separators=(',', ':')).encode('ascii')


def build():
    width, height = 36, 28
    terrain = [[1 for _ in range(width)] for _ in range(height)]
    collision = [[0 for _ in range(width)] for _ in range(height)]
    overdraw = [[0 for _ in range(width)] for _ in range(height)]

    def rect(x1, y1, x2, y2, tile, blocked=None):
        for y in range(y1, y2 + 1):
            for x in range(x1, x2 + 1):
                terrain[y][x] = tile
                if blocked is not None:
                    collision[y][x] = blocked

    # A closed virtual boundary, not an assertion about real fencing.
    for x in range(width):
        collision[0][x] = collision[-1][x] = 1
        terrain[0][x] = terrain[-1][x] = 4
    for y in range(height):
        collision[y][0] = collision[y][-1] = 1
        terrain[y][0] = terrain[y][-1] = 4
    rect(13, 8, 17, 25, 2)
    rect(2, 18, 33, 21, 2)
    rect(23, 9, 33, 24, 3)
    rect(2, 24, 33, 25, 4)
    rect(13, 24, 17, 25, 2)
    # Broad Studio-above-Park relationship only; all extents are original.
    rect(8, 3, 18, 7, 6, 1)
    rect(8, 8, 18, 8, 7, 1)
    rect(12, 9, 18, 10, 3)
    rect(3, 11, 9, 14, 5, 1)
    rect(27, 10, 31, 11, 6, 1)
    rect(26, 20, 31, 21, 7, 1)
    for x1, y1, x2, y2 in [(3, 4, 5, 7), (21, 3, 23, 6), (3, 22, 7, 23), (19, 22, 21, 24), (31, 3, 33, 6)]:
        rect(x1, y1, x2, y2, 1, 1)
        for y in range(y1, y2 + 1):
            for x in range(x1, x2 + 1):
                overdraw[y][x] = 9
    for x, y in [(11, 11), (11, 13), (20, 11), (20, 13), (9, 22), (24, 22)]:
        overdraw[y][x] = 10
    for x, y in [(12, 18), (18, 18), (24, 17), (32, 17)]:
        overdraw[y][x] = 14
    for x, y in [(3, 10), (9, 10)]:
        overdraw[y][x] = 12

    definitions = [
        ('studio', 'Virtual Studio landmark', 13, 9, 'venue_diagram', 'Broad diagram-relative position above park; original virtual footprint.'),
        ('park', 'Virtual Culture Park lawn', 12, 16, 'venue_overview', 'Lawn character and broad relation only; no real paths or scale.'),
        ('stage', 'Virtual stage', 6, 15, 'concept_reference', 'Original activity landmark; no physical stage location asserted.'),
        ('gifts', 'Virtual gift corner', 28, 13, 'concept_reference', 'Original virtual activity destination; not a pickup location.'),
        ('wall', 'Virtual pixel wall', 28, 22, 'concept_reference', 'Original virtual mural destination; no physical wall claimed.'),
    ]
    places, interactions, landmarks = [], [], []
    source_pack = ROOT / 'frontend/public/world/places/source-pack.json'
    source_hash = sha(source_pack.read_bytes())
    for number, (key, label, x, y, source, basis) in enumerate(definitions, 1):
        pid = f'6a5c196b-58e1-47d5-99bb-{number:012d}'
        places.append({'place_id': pid, 'label': label, 'place_kind': 'VIRTUAL_CONCEPT', 'verification_state': 'CONCEPT_ONLY',
                       'closure_state': 'OPEN', 'accessibility_notes': 'Virtual landmark only. Physical routes, accessibility, closure and meeting eligibility are not established.',
                       'provenance': {'source_id': 'ar02_original_courtyard', 'source_kind': 'ORIGINAL_ART', 'source_revision': 'ar02_courtyard_v1; source-pack ' + source_hash,
                                      'source_uri': None, 'author': 'MCR AR02 map author', 'license': 'Original MCR geometry and metadata; no third-party raster or map artwork included.',
                                      'review_state': 'UNREVIEWED', 'reviewed_by': None, 'reviewed_at': None, 'photo_permission': 'NOT_APPLICABLE'}})
        interactions.append({'interaction_id': key + '_info', 'place_id': pid, 'cell_x': x, 'cell_y': y})
        landmarks.append({'landmark_id': key, 'place_id': pid, 'interaction_id': key + '_info', 'cell_x': x, 'cell_y': y,
                          'source_reference': source, 'qualification': basis, 'physical_candidate_alias': None})
    geometry = {'width_cells': width, 'height_cells': height, 'tile_width_px': 32, 'tile_height_px': 32, 'collision_id': 'paradise_virtual_collision_v1',
                'collision_grid': collision, 'terrain_layer': terrain, 'overdraw_layer': overdraw,
                'spawns': [{'spawn_id': key, 'cell_x': x, 'cell_y': y} for key, x, y in [('courtyard_entry', 15, 24), ('lawn_entry', 6, 20), ('community_entry', 30, 18)]],
                'interactions': interactions, 'default_spawn_id': 'courtyard_entry'}
    tiled = {'type': 'map', 'orientation': 'orthogonal', 'renderorder': 'right-down', 'infinite': False, 'version': '1.10', 'tiledversion': 'AR02-authored-v1',
             'width': width, 'height': height, 'tilewidth': 32, 'tileheight': 32,
             'layers': [{'id': i, 'name': name, 'type': 'tilelayer', 'width': width, 'height': height, 'x': 0, 'y': 0, 'opacity': 1, 'visible': True, 'data': sum(grid, [])}
                        for i, (name, grid) in enumerate([('terrain', terrain), ('collision', collision), ('overdraw', overdraw)], 1)],
             'tilesets': [{'firstgid': 1, 'name': 'theme', 'image': 'mcr-theme.png', 'imagewidth': 256, 'imageheight': 128,
                          'tilewidth': 32, 'tileheight': 32, 'tilecount': 32, 'columns': 8, 'margin': 0, 'spacing': 0}]}
    dump('courtyard-tiled.json', tiled)
    dump('geometry.json', geometry)
    dump('virtual-places.json', places)
    dump('landmarks.json', landmarks)
    dump('binding-inputs.json', {'format': 'AR02_LOCAL_BINDING_INPUTS_NOT_RELEASE_MANIFEST', 'map_id': 'paradise_virtual_courtyard', 'map_revision': 1,
                               'geometry_hash': sha(canonical(geometry)), 'tilemap_sha256': sha((HERE / 'courtyard-tiled.json').read_bytes()),
                               'tilemap_path': 'world/maps/courtyard-tiled.json', 'source_pack_sha256': source_hash,
                               'texture_dimensions': {'width_px': 256, 'height_px': 128}, 'tile_semantics': ['empty', 'lawn', 'paving', 'alternate_paving', 'gravel', 'stage_deck', 'roof', 'wall', 'water', 'canopy', 'flower', 'banner', 'truss', 'sign', 'lamp'],
                               'publication': 'UNBOUND; final MapManifest/ThemeManifest and release pins belong to Delivery.',
                               'limits': ['Original virtual geometry, not surveyed physical venue.', 'No physical candidate IDs or operational destinations published.', 'Collision layer is logical0/1; rendering must hide collision and choose an AR04 texture for inert marker.', 'Spawns/interactions are normalized C06 data; landmarks are sidecar associations.']})


if __name__ == '__main__':
    build()
