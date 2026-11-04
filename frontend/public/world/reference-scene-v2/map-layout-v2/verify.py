"""Read-only structural checks. This does not certify a new rendered composition."""
import hashlib
import json
from pathlib import Path

from build import PANELS, components

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
ART = HERE.parent


def load(path):
    return json.loads(path.read_text())


def check_pin(pin):
    value = (ROOT / pin['path']).read_bytes()
    assert len(value) == pin['bytes'], pin['path']
    assert 'sha256:' + hashlib.sha256(value).hexdigest() == pin['sha256'], pin['path']


def inside(point, polygon):
    x, y = point
    odd = False
    previous = polygon[-1]
    for current in polygon:
        ax, ay = previous
        bx, by = current
        if (ay > y) != (by > y) and x < (bx-ax)*(y-ay)/(by-ay)+ax:
            odd = not odd
        previous = current
    return odd


def overlap(a, b):
    return a['x'] < b['x']+b['width'] and b['x'] < a['x']+a['width'] \
        and a['y'] < b['y']+b['height'] and b['y'] < a['y']+a['height']


def verify():
    labels = load(HERE / 'label-presentation.json')
    plants = load(HERE / 'landscape-presentation.json')
    measure = load(HERE / 'target-measurements.json')
    old_labels = load(ART / 'quality-v10/map-label-presentation.json')
    old_plants = load(ART / 'map-landscape-v1/presentation.json')
    geometry = load(ART / 'spatial-v5/geometry-proposal.json')
    assert labels['geometry_hash'] == plants['geometry_hash'] == geometry['geometry_hash']
    assert labels['view'] == plants['view'] == 'MAP_ONLY'
    for document in [labels, plants]:
        check_pin(document['base_presentation'])
        for pin in document['frozen_inputs']:
            check_pin(pin)
        assert document['status'] == 'CONTRACT_READY_METADATA_RENDER_VALIDATION_NOT_RUN'
    check_pin(measure['target'])
    assert plants['required_label_presentation']['path'] == (
        HERE / 'label-presentation.json').relative_to(ROOT).as_posix()
    check_pin(plants['required_label_presentation'])
    observed_components = components()
    assert len(labels['labels']) == len(old_labels['labels']) == 6
    for key in ['style', 'icons', 'icon_cutouts', 'font', 'font_license', 'dimensions', 'authority']:
        assert labels[key] == old_labels[key], key
    maximum_error = 0
    for old, new, evidence in zip(old_labels['labels'], labels['labels'], measure['labels']):
        for key in ['label', 'interaction_id', 'lines', 'icon', 'binding_policy']:
            assert old[key] == new[key], (new['label'], key)
        assert new['rect']['width'] == old['rect']['width']
        assert new['rect']['height'] == old['rect']['height']
        r = new['rect']
        assert 0 <= r['x'] <= 768-r['width'] and 0 <= r['y'] <= 1296-r['height']
        center = [r['x']+r['width']/2+(35 if new['icon'] else 0), r['y']+r['height']/2]
        box = evidence['target_text_paper_bbox']
        assert box == PANELS[new['label']]
        assert observed_components[tuple(box)] == evidence['threshold_component_pixels']
        expected = [(box[0]+box[2])/2*768/248, (box[1]+box[3])/2*1296/420]
        assert evidence['label'] == new['label']
        assert center == evidence['proposed_text_paper_center']
        for a, b in zip(center, expected):
            maximum_error = max(maximum_error, abs(a-b))
            assert abs(a-b) <= .5, (new['label'], center, expected)
    for i, label in enumerate(labels['labels']):
        for other in labels['labels'][i+1:]:
            assert not overlap(label['rect'], other['rect'])
    assert len(plants['decorations']) == len(old_plants['decorations']) == 10
    for key in ['themes', 'dimensions', 'authority', 'provenance']:
        assert plants[key] == old_plants[key], key
    changes = []
    pixel_checks = 0
    for old, new in zip(old_plants['decorations'], plants['decorations']):
        assert old['id'] == new['id']
        assert new['interaction_id'] is None and new['view'] == 'MAP_ONLY'
        assert new['depth_y'] == new['y']+new['height']
        permitted = {'x', 'y', 'width', 'height', 'depth_y'}
        assert {k:v for k,v in old.items() if k not in permitted} == \
               {k:v for k,v in new.items() if k not in permitted}
        for label in labels['labels']:
            assert not overlap(new, label['rect']), (new['id'], label['label'])
        if old != new:
            changes.append(new['id'])
            assert new['surface'] == 'semantic_plaza'
            for y in range(new['y'], new['y']+new['height']):
                for x in range(new['x'], new['x']+new['width']):
                    assert inside((x+.5, y+.5), geometry['plaza_parcel']), (new['id'], x, y)
                    pixel_checks += 1
    assert set(changes) == {'map_garden_plaza_south_west', 'map_garden_plaza_south_east'}
    assert measure['validation']['new_native_export'] == 'NOT_RUN'
    assert measure['validation']['new_mobile_comparison'] == 'NOT_RUN'
    return {'status': 'PASS_STRUCTURAL_ONLY', 'labels': 6, 'decorations': 10,
            'changed_unbound_decorations': 2, 'new_decoration_surface_pixel_checks': pixel_checks,
            'maximum_text_panel_center_error_source_px': maximum_error,
            'new_native_export': 'NOT_RUN', 'new_mobile_comparison': 'NOT_RUN',
            'canonical_geometry_or_runtime_mutations': 0}


if __name__ == '__main__':
    print(json.dumps(verify()))
