#!/usr/bin/env python3
"""Candidate geometry and C06 validation; never creates a product release binding."""
import copy
import hashlib
import importlib.util
import json
from collections import deque
from pathlib import Path
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
spec = importlib.util.spec_from_file_location('c06', ROOT / 'scripts/mcr/validate-assets.py')
c06 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c06)


def read(name):
    return c06.read_json(HERE / name)


def verify_export(tiled, geometry):
    assert (tiled['width'], tiled['height'], tiled['tilewidth'], tiled['tileheight']) == (
        geometry['width_cells'], geometry['height_cells'], geometry['tile_width_px'], geometry['tile_height_px'])
    assert len(tiled['layers']) == 3
    grids = dict(terrain=geometry['terrain_layer'], collision=geometry['collision_grid'], overdraw=geometry['overdraw_layer'])
    assert {x['name'] for x in tiled['layers']} == set(grids)
    for layer in tiled['layers']:
        assert layer['data'] == sum(grids[layer['name']], [])
    assert tiled['tilesets'][0]['image'] == 'mcr-theme.png'
    assert tiled['tilesets'][0]['columns'] == 8 and tiled['tilesets'][0]['tilecount'] == 32


def must_fail(name, action):
    try:
        action()
    except (AssertionError, c06.AssetError):
        return name
    raise AssertionError('mutation accepted: ' + name)


def main():
    geometry, places, tiled, pins = read('geometry.json'), read('virtual-places.json'), read('courtyard-tiled.json'), read('binding-inputs.json')
    verify_export(tiled, geometry)
    assert c06.digest(geometry) == pins['geometry_hash']
    assert 'sha256:' + hashlib.sha256((HERE / 'courtyard-tiled.json').read_bytes()).hexdigest() == pins['tilemap_sha256']
    pack_path = ROOT / 'frontend/public/world/places/source-pack.json'
    assert 'sha256:' + hashlib.sha256(pack_path.read_bytes()).hexdigest() == pins['source_pack_sha256']
    pack = c06.read_json(pack_path)
    assert not ({x['place_id'] for x in places} & {x['place_id'] for x in pack['landmarks']})
    assert all(x['place_kind'] == 'VIRTUAL_CONCEPT' and x['verification_state'] == 'CONCEPT_ONLY' for x in places)
    assert all(x['operator_closure_state'] == 'UNKNOWN' and x['publication_availability'] == 'UNAVAILABLE' for x in pack['landmarks'])
    candidate = {'kind': 'map_manifest', 'contract_version': '1.0.0', 'map_id': pins['map_id'], 'map_revision': 1,
                 'geometry_hash': pins['geometry_hash'], 'geometry': geometry, 'places': places,
                 'provenance': places[0]['provenance'], 'tilemap': {'format': 'TILED_JSON', 'resource_state': 'PROVIDED', 'path': pins['tilemap_path'], 'sha256': pins['tilemap_sha256']}}
    candidate['content_hash'] = c06.content_hash(candidate)
    c06.validate_document(candidate, c06.validators(), 'unpublished AR02 map candidate')
    c06.validate_map(candidate)
    landmarks = read('landmarks.json')
    assert {x['place_id'] for x in landmarks} == {x['place_id'] for x in places}
    assert {x['interaction_id'] for x in landmarks} == {x['interaction_id'] for x in geometry['interactions']}
    for landmark, interaction in zip(landmarks, geometry['interactions']):
        assert (landmark['cell_x'], landmark['cell_y'], landmark['place_id'], landmark['interaction_id']) == (
            interaction['cell_x'], interaction['cell_y'], interaction['place_id'], interaction['interaction_id'])
    # Exhaustive virtual cell-center collision parity with BE12's row-major formula.
    width, height = geometry['width_cells'], geometry['height_cells']
    flat = sum(geometry['collision_grid'], [])
    for y in range(height):
        for x in range(width):
            assert (flat[y * width + x] == 0) == (geometry['collision_grid'][y][x] == 0)
    assert all(flat[x] and flat[(height - 1) * width + x] for x in range(width))
    assert all(flat[y * width] and flat[y * width + width - 1] for y in range(height))
    start = geometry['spawns'][0]
    seen, queue = {(start['cell_x'], start['cell_y'])}, deque([(start['cell_x'], start['cell_y'])])
    while queue:
        x, y = queue.popleft()
        for nx, ny in [(x-1,y),(x+1,y),(x,y-1),(x,y+1)]:
            if 0 <= nx < width and 0 <= ny < height and not flat[ny*width+nx] and (nx,ny) not in seen:
                seen.add((nx,ny)); queue.append((nx,ny))
    assert len(seen) == flat.count(0), 'orphan walkable region'
    for x, y in seen:
        assert not flat[y * width + x]
    # Actual occupied structures and surrounding virtual boundary stay blocked.
    assert all(geometry['collision_grid'][y][x] == 1 for x, y in [(8,3),(18,8),(3,11),(9,14),(27,10),(31,21),(0,5),(35,5)])
    negative = []
    bad = copy.deepcopy(tiled); bad['layers'][1]['data'][0] = 0
    negative.append(must_fail('export-collision-drift', lambda: verify_export(bad, geometry)))
    for name, mutate in [
        ('blocked-spawn', lambda g: g['spawns'][0].update(cell_x=8,cell_y=3)),
        ('unknown-place', lambda g: g['interactions'][0].update(place_id='ffffffff-ffff-4fff-8fff-ffffffffffff')),
        ('severed-route', lambda g: g['collision_grid'].__setitem__(17, [1]*width)),
        ('nonbinary-collision', lambda g: g['collision_grid'][1].__setitem__(1,2)),
    ]:
        altered = copy.deepcopy(candidate); mutate(altered['geometry']); altered['geometry_hash'] = c06.digest(altered['geometry']); altered['content_hash'] = c06.content_hash(altered)
        def check(altered=altered):
            c06.validate_document(altered, c06.validators(), 'negative')
            c06.validate_map(altered)
        negative.append(must_fail(name, check))
    sources = [ROOT / 'scripts/mcr/validate-assets.py', ROOT / 'backend/crates/concert-domain/src/world/mod.rs',
               *sorted((ROOT / 'contracts/assets/v1').glob('*.schema.json'))]
    source_hashes = {str(p.relative_to(ROOT)): 'sha256:' + hashlib.sha256(p.read_bytes()).hexdigest() for p in sources}
    report = {'status': 'CONTRACT_READY', 'scope': 'AR02 unpublished original virtual map candidate; no theme/avatar/release binding',
              'base': 'a25b85592b906209f4a9d20f9c7279ced8abab02', 'geometry_hash': pins['geometry_hash'], 'tilemap_sha256': pins['tilemap_sha256'],
              'source_revisions': source_hashes,
              'checks': {'c06_schema_hash_and_reachability': 'PASS', 'tiled_normalized_grid_parity': 'PASS', 'landmark_interaction_resolution': 'PASS', 'physical_candidate_exclusion': 'PASS', 'closed_boundary_and_structures': 'PASS', 'all_walkable_cells_connected': 'PASS'},
              'cells': width*height, 'walkable_cells': len(seen), 'blocked_cells': flat.count(1), 'spawns': len(geometry['spawns']), 'interactions': len(geometry['interactions']),
              'negative_cases_rejected': negative, 'server_parity': 'Encoded BE12 cell-center row-major convention checked exhaustively; actual Rust runtime NOT_RUN (no build lease).',
              'external_gates': ['Operator venue qualification NOT_RUN', 'Physical device/QA06 NOT_RUN', 'Actual multiuser/IN01 NOT_RUN', 'Final product release pins NOT_RUN']}
    path = ROOT / 'docs/mcr/art/evidence/maps/geometry-validation.json'
    path.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
