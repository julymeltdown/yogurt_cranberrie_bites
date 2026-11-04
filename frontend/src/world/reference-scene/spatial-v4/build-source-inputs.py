#!/usr/bin/env python3
"""Verify published v8 source and derive additive runtime bytes; never alter artist/older files."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
PUBLIC = HERE.parents[3] / 'public'
BASE = PUBLIC / 'world/reference-scene-v2'
SOURCE_REVISION = '492160b15465818eca172f4017f3816d58c989a9'


def sha(raw):
    return 'sha256:' + hashlib.sha256(raw).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False)


def build():
    pins = {}

    def read(path, expected, size=None):
        raw = (BASE / path).read_bytes()
        assert sha(raw) == expected, path
        assert size is None or len(raw) == size, path
        pins[path] = expected
        return json.loads(raw) if path.endswith('.json') else raw

    manifest = read('quality-v8/manifest.json', 'sha256:0aa7ffc44d4fa69ad8d0ac9dfb8ab30907ce0e785aa461a03a1452b8461518d6')
    source = read('spatial-v4/geometry-proposal.json', manifest['geometry']['sha256'])
    projection = read('spatial-v4/integer-projection.json', manifest['integer_projection']['sha256'])
    evidence = read('spatial-v4/revision-evidence.json', manifest['geometry_revision_evidence']['sha256'])
    read('quality-v8/source-metadata.json', manifest['source_metadata']['sha256'])
    for item in manifest['sources']:
        read('quality-v8/' + item['path'], item['sha256'], item['bytes'])
    geometry = source['geometry']
    assert sha(canonical(geometry).encode()) == source['geometry_hash'] == projection['geometry_hash']
    assert projection['placements'] == source['placements'] and len(projection['placements']) == 34
    assert len(evidence['changed_cells']) == 72
    previous = json.loads((BASE / 'spatial-v3/geometry-proposal.json').read_bytes())
    old_projection = json.loads((BASE / 'spatial-v3/integer-projection.json').read_bytes())
    assert projection['placements'][:22] == old_projection['placements']
    assert projection['overview_representations'] == old_projection['overview_representations']
    changed = []
    for y, row in enumerate(geometry['collision_grid']):
        for x, cell in enumerate(row):
            old = previous['geometry']['collision_grid'][y][x]
            assert cell >= old
            if cell != old:
                changed.append(dict(cell_x=x, cell_y=y, previous=old, proposed=cell))
    assert changed == evidence['changed_cells']
    for key in ['terrain_layer', 'overdraw_layer', 'spawns', 'interactions', 'default_spawn_id']:
        assert geometry[key] == previous['geometry'][key], key
    for key in ['places', 'world_camera', 'overview_camera', 'plaza_parcel']:
        assert source[key] == previous[key], key
    scenes, binding_scenes, shared = {}, [], None
    for theme in manifest['themes']:
        docs, inputs = {}, []
        for item in theme['files']:
            path = 'quality-v8/' + item['path']
            value = read(path, item['sha256'], item['bytes'])
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
    # This one finite three-layer serializer is the runtime owner's output, not another art proposal.
    tiled = dict(type='map', version='1.10', tiledversion='1.10.2', orientation='orthogonal', renderorder='right-down',
                 infinite=False, width=48, height=81, tilewidth=16, tileheight=16,
                 layers=[dict(id=i + 1, name=name, type='tilelayer', width=48, height=81, x=0, y=0, opacity=1, visible=True,
                              data=[cell for row in geometry[key] for cell in row])
                         for i, (name, key) in enumerate([('terrain', 'terrain_layer'), ('collision', 'collision_grid'), ('overdraw', 'overdraw_layer')])],
                 tilesets=[dict(firstgid=1, name='theme', image='mcr-theme.png', imagewidth=768, imageheight=1296,
                                tilewidth=16, tileheight=16, tilecount=3888, columns=48, margin=0, spacing=0)])
    tile_bytes = (json.dumps(tiled, separators=(',', ':')) + '\n').encode()
    runtime = BASE / 'runtime/spatial-v4'
    runtime.mkdir(parents=True, exist_ok=True)
    tile_path = 'world/reference-scene-v2/runtime/spatial-v4/canonical-map.json'
    tile = dict(path=tile_path, sha256=sha(tile_bytes), bytes=len(tile_bytes), format='TILED_JSON')
    (PUBLIC / tile_path).write_bytes(tile_bytes)
    output = dict(source_revision=SOURCE_REVISION, map_id='mcr_reference_scene_r3', minimum_map_revision=5, inputs=pins,
                  geometry_hash=source['geometry_hash'], geometry=geometry, places=source['places'],
                  map_labels=shared['map_labels'], overview_representations=shared['overview_representations'],
                  plaza_parcel=shared['plaza_parcel'], world_camera=source['world_camera'], scenes=scenes)
    (HERE / 'source-inputs.json').write_text(json.dumps(output, separators=(',', ':')) + '\n')
    (HERE / 'tile-pin.json').write_text(json.dumps(tile, indent=2) + '\n')
    labels_dir = BASE / 'map-labels-v1'
    presentation_raw = (labels_dir / 'presentation.json').read_bytes()
    handoff_raw = (labels_dir / 'handoff.json').read_bytes()
    assert sha(presentation_raw) == 'sha256:f9599a9859fe0b660fee591440dee2f7ab8e025a5f525df82091b61b091b23cc'
    assert sha(handoff_raw) == 'sha256:a3cfaf56d880bca913f34e91fea72c70aefa8b8c8e3bc8aa8b30f76e3b473686'
    presentation, handoff = json.loads(presentation_raw), json.loads(handoff_raw)
    assert presentation['geometry_hash'] == handoff['source_geometry_hash'] == source['geometry_hash']
    for item in handoff['files']:
        raw = (labels_dir / item['path']).read_bytes()
        assert len(raw) == item['bytes'] and sha(raw) == item['sha256'], item['path']
    for item in presentation['frozen_inputs'] + [presentation['font'], presentation['font_license']]:
        raw = (HERE.parents[4] / item['path']).read_bytes()
        assert len(raw) == item['bytes'] and sha(raw) == item['sha256'], item['path']
    assert [(v['label'], v['x'], v['y'], v['interaction_id']) for v in presentation['labels']] == [(v['label'], v['x'], v['y'], v['interaction_id']) for v in source['map_labels']]
    (HERE / 'map-labels-source.json').write_bytes(presentation_raw)
    labels_pin = dict(source_revision='f27cc114e2482b98d75ba00ce0104008a97d2e9e',
                     presentation=dict(path='world/reference-scene-v2/map-labels-v1/presentation.json', bytes=len(presentation_raw), sha256=sha(presentation_raw)),
                     handoff=dict(path='world/reference-scene-v2/map-labels-v1/handoff.json', bytes=len(handoff_raw), sha256=sha(handoff_raw)),
                     files=handoff['files'], runtime_font=presentation['font'],
                     authority='MAP-only decoration; no third PNG, C06 extension, admission or invented hit target.')
    (HERE / 'map-labels-pins.json').write_text(json.dumps(labels_pin, indent=2) + '\n')
    binding = dict(format='MCR_REFERENCE_SCENE_SPATIAL4_PUBLIC_INPUTS', version=1, status='NOT_RELEASE_BOUND',
                   production_status='NOT_ADMITTED', source_revision=SOURCE_REVISION,
                   candidate_map=dict(map_id=output['map_id'], map_revision=None, minimum_map_revision=5,
                                      allocation='PENDING_ACTUAL_BACKEND_PRODUCER', geometry_hash=source['geometry_hash']),
                   source_inputs=[dict(path='world/reference-scene-v2/' + path, sha256=pin) for path, pin in pins.items()],
                   tilemap=tile, scenes=binding_scenes, map_presentation=labels_pin,
                   authority='Exact public source bytes only; actual server revision is producer-owned. No release/source-review/physical admission.')
    (runtime / 'source-binding.json').write_text(json.dumps(binding, indent=2) + '\n')
    print(json.dumps(dict(map=binding['candidate_map'], tilemap=tile, source_pins=len(pins), world_objects=34, changed_cells=len(changed))))


if __name__ == '__main__':
    build()
