#!/usr/bin/env python3
"""Derive finite v10 runtime inputs from the immutable, published artist packet."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
PUBLIC = HERE.parents[3] / 'public'
BASE = PUBLIC / 'world/reference-scene-v2'
SOURCE_REVISION = '06562ff1962be525ed929ef431a54bb589860373'


def sha(raw):
    return 'sha256:' + hashlib.sha256(raw).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False)


def build():
    pins = {}

    def read(path, expected=None, size=None):
        path = str(Path(path))
        raw = (BASE / path).read_bytes()
        assert expected is None or sha(raw) == expected, path
        assert size is None or len(raw) == size, path
        pins[path] = sha(raw)
        return json.loads(raw) if path.endswith('.json') else raw

    handoff = read('quality-v10/handoff.json', 'sha256:ac8abde55c18e9c28580a97a0f996b29d83c1daadf45b190511ccec792c0fcfd', 33217)
    for item in handoff['files']:
        raw = (PUBLIC.parent / item['path'].removeprefix('frontend/')).read_bytes()
        assert len(raw) == item['bytes'] and sha(raw) == item['sha256'], item['path']
    manifest = read('quality-v10/manifest.json', 'sha256:9048d45753f4d47828082c422061ed17418f8bfb93b51cf3e9879e821eeb0cf6')
    def pinned(pin):
        # Preserve the exact source location after resolving its relative manifest path.
        path = (BASE / 'quality-v10' / pin['path']).resolve().relative_to(BASE.resolve())
        return read(path, pin['sha256'], pin.get('bytes'))
    source, projection = pinned(manifest['geometry']), pinned(manifest['integer_projection'])
    evidence = pinned(manifest['revision_evidence'])
    pinned(manifest['base_manifest'])
    for item in manifest['ground_sources']:
        pinned(item)
    geometry = source['geometry']
    assert sha(canonical(geometry).encode()) == source['geometry_hash'] == projection['geometry_hash'] == manifest['geometry_hash']
    previous = json.loads((BASE / 'spatial-v4/geometry-proposal.json').read_bytes())
    old_projection = json.loads((BASE / 'spatial-v4/integer-projection.json').read_bytes())
    assert {k: v for k, v in geometry.items() if k != 'terrain_layer'} == {k: v for k, v in previous['geometry'].items() if k != 'terrain_layer'}
    changed = [dict(cell_x=x, cell_y=y, previous=previous['geometry']['terrain_layer'][y][x], proposed=cell)
               for y, row in enumerate(geometry['terrain_layer']) for x, cell in enumerate(row)
               if cell != previous['geometry']['terrain_layer'][y][x]]
    assert changed == evidence['terrain_changed_cells'] and len(changed) == 560
    assert evidence['collision_changed_cells'] == 0 and evidence['world_props_changed'] == 0
    assert projection['placements'] == source['placements'] == old_projection['placements']
    assert len(projection['placements']) == 34
    for key in ['places', 'world_camera', 'overview_camera', 'plaza_parcel', 'map_labels']:
        assert source[key] == previous[key], key
    scenes, binding_scenes, shared = {}, [], None
    for theme in manifest['themes']:
        docs, inputs = {}, []
        for item in theme['files']:
            value = pinned(item)
            path = (BASE / 'quality-v10' / item['path']).resolve().relative_to(BASE.resolve()).as_posix()
            inputs.append(dict(path='world/reference-scene-v2/' + path, **{k: item[k] for k in ['sha256', 'bytes']}))
            if path.endswith('.json'):
                docs[path.rsplit('/', 1)[-1]] = value
        layout, meta, scenery = docs['layout.json'], docs['map-metadata.json'], docs['scenery.json']
        assert layout['placements'] == projection['placements']
        assert layout['overview_representations'] == projection['overview_representations'] == meta['overview_representations']
        assert layout['geometry_hash'] == meta['geometry_hash'] == source['geometry_hash']
        assert meta['places'] == source['places'] and meta['map_labels'] == source['map_labels']
        assert len(scenery['objects']) == 34
        for prop, packed in zip(scenery['objects'], projection['placements']):
            assert all(prop[k] == packed[k] for k in ['id', 'x', 'y', 'width', 'height', 'depth_y'])
            assert prop['frame'] == layout['frames'][packed['frame']]
        assert shared is None or meta == shared
        shared = meta
        resources = [scenery['ground'], *scenery['atlases']]
        assert len(resources) == 2 and sum(r['bytes'] for r in resources) <= 2097152
        assert sum(r['width_px'] * r['height_px'] for r in resources) == 2043904
        for item in resources:
            read(item['path'].removeprefix('world/reference-scene-v2/'), item['sha256'], item['bytes'])
        scenes[theme['theme_id']] = dict(scenery=scenery, frames=layout['frames'])
        binding_scenes.append(dict(theme_id=theme['theme_id'], sources=inputs, runtime_resources=resources))
    tiled = dict(type='map', version='1.10', tiledversion='1.10.2', orientation='orthogonal', renderorder='right-down',
                 infinite=False, width=48, height=81, tilewidth=16, tileheight=16,
                 layers=[dict(id=i + 1, name=name, type='tilelayer', width=48, height=81, x=0, y=0, opacity=1, visible=True,
                              data=[cell for row in geometry[key] for cell in row])
                         for i, (name, key) in enumerate([('terrain', 'terrain_layer'), ('collision', 'collision_grid'), ('overdraw', 'overdraw_layer')])],
                 tilesets=[dict(firstgid=1, name='theme', image='mcr-theme.png', imagewidth=768, imageheight=1296,
                                tilewidth=16, tileheight=16, tilecount=3888, columns=48, margin=0, spacing=0)])
    tile_bytes = (json.dumps(tiled, separators=(',', ':')) + '\n').encode()
    runtime = BASE / 'runtime/spatial-v5'
    runtime.mkdir(parents=True, exist_ok=True)
    tile_path = 'world/reference-scene-v2/runtime/spatial-v5/canonical-map.json'
    tile = dict(path=tile_path, sha256=sha(tile_bytes), bytes=len(tile_bytes), format='TILED_JSON')
    (PUBLIC / tile_path).write_bytes(tile_bytes)
    labels_raw = (BASE / 'quality-v10/map-label-presentation.json').read_bytes()
    labels = pinned(manifest['map_label_sidecar'])
    assert labels['geometry_hash'] == source['geometry_hash']
    for item in labels['frozen_inputs'] + [labels['font'], labels['font_license'], labels['base_presentation']]:
        raw = (PUBLIC.parent / item['path'].removeprefix('frontend/')).read_bytes()
        assert len(raw) == item['bytes'] and sha(raw) == item['sha256'], item['path']
    assert [(v['label'], v['x'], v['y'], v['interaction_id']) for v in labels['labels']] == [(v['label'], v['x'], v['y'], v['interaction_id']) for v in source['map_labels']]
    (HERE / 'map-labels-source.json').write_bytes(labels_raw)
    labels_pin = dict(source_revision=SOURCE_REVISION, presentation=dict(path='world/reference-scene-v2/quality-v10/map-label-presentation.json', bytes=len(labels_raw), sha256=sha(labels_raw)),
                      runtime_font=labels['font'], authority='MAP-only source presentation for ef52; no C06 extension, admission or new destination.')
    (HERE / 'map-labels-pins.json').write_text(json.dumps(labels_pin, indent=2) + '\n')
    output = dict(source_revision=SOURCE_REVISION, map_id='mcr_reference_scene_r3', minimum_map_revision=6, inputs=pins,
                  geometry_hash=source['geometry_hash'], geometry=geometry, places=source['places'],
                  map_labels=shared['map_labels'], overview_representations=shared['overview_representations'],
                  plaza_parcel=shared['plaza_parcel'], world_camera=source['world_camera'], scenes=scenes)
    (HERE / 'source-inputs.json').write_text(json.dumps(output, separators=(',', ':')) + '\n')
    (HERE / 'tile-pin.json').write_text(json.dumps(tile, indent=2) + '\n')
    allocation_path = HERE / 'backend-allocation.json'
    allocation = json.loads(allocation_path.read_bytes()) if allocation_path.exists() else None
    if allocation:
        assert allocation['map_id'] == output['map_id'] and allocation['geometry_hash'] == output['geometry_hash']
        assert type(allocation['map_revision']) is int and allocation['map_revision'] >= 6
    candidate = dict(map_id=output['map_id'], map_revision=allocation['map_revision'] if allocation else None,
                     minimum_map_revision=6, geometry_hash=source['geometry_hash'],
                     allocation='ALLOCATED_BY_CANONICAL_BACKEND_PRODUCER_NOT_RELEASED' if allocation else 'PENDING_CANONICAL_BACKEND_ALLOCATION')
    binding = dict(format='MCR_REFERENCE_SCENE_SPATIAL5_PUBLIC_INPUTS', version=1, status='NOT_RELEASE_BOUND', production_status='NOT_ADMITTED',
                   source_revision=SOURCE_REVISION, candidate_map=candidate, backend_allocation=allocation,
                   source_inputs=[dict(path='world/reference-scene-v2/' + path, sha256=pin) for path, pin in pins.items()],
                   tilemap=tile, scenes=binding_scenes, map_presentation=labels_pin,
                   authority='Exact public source bytes only; minimum6 is a constraint, not a server reservation or release/source-review/physical admission.')
    (runtime / 'source-binding.json').write_text(json.dumps(binding, indent=2) + '\n')
    print(json.dumps(dict(map=candidate, tilemap=tile, source_pins=len(pins), world_objects=34, terrain_changed=len(changed), collision_changed=0)))


if __name__ == '__main__':
    build()
