"""Small metadata-only layout correction; never renders or changes frozen inputs."""
from collections import deque
from copy import deepcopy
import hashlib
import json
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
ART = HERE.parent
TARGET = ROOT / 'docs/mcr/design/reference-target-2026-11-04.png'
TARGET_SHA = 'd9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617'
CROP = [975, 95, 248, 420]
GEOMETRY = 'sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395'
# These identify the six cream TEXT panels, excluding the tinted icon cells.
# Coordinates are in the original MAP crop. They are measurements, not product pixels.
PANELS = {
    'MAIN STAGE': [108, 54, 187, 76],
    'PLAZA': [81, 158, 130, 185],
    'MERCH': [43, 216, 94, 238],
    'FOOD': [202, 211, 242, 233],
    'MEETUP ZONE': [105, 280, 157, 315],
    'GIVEAWAY BOOTH': [148, 358, 209, 393],
}


def pin(path):
    data = path.read_bytes()
    return {'path': path.relative_to(ROOT).as_posix(), 'bytes': len(data),
            'sha256': 'sha256:' + hashlib.sha256(data).hexdigest()}


def write(name, value):
    (HERE / name).write_text(json.dumps(value, indent=2) + '\n')


def components():
    assert hashlib.sha256(TARGET.read_bytes()).hexdigest() == TARGET_SHA
    image = Image.open(TARGET).convert('RGB')
    x0, y0, width, height = CROP
    pixels = set()
    for y in range(height):
        for x in range(width):
            r, g, b = image.getpixel((x0 + x, y0 + y))
            if r > 210 and g > 200 and b > 170 and max(r, g, b) - min(r, g, b) < 65:
                pixels.add((x, y))
    result = {}
    while pixels:
        first = pixels.pop()
        queue, points = deque([first]), [first]
        while queue:
            x, y = queue.popleft()
            for point in [(x-1, y), (x+1, y), (x, y-1), (x, y+1)]:
                if point in pixels:
                    pixels.remove(point)
                    queue.append(point)
                    points.append(point)
        if len(points) > 80:
            xs, ys = zip(*points)
            result[tuple([min(xs), min(ys), max(xs)+1, max(ys)+1])] = len(points)
    return result


def build():
    old_label_path = ART / 'quality-v10/map-label-presentation.json'
    old_landscape_path = ART / 'map-landscape-v1/presentation.json'
    labels = json.loads(old_label_path.read_text())
    landscape = json.loads(old_landscape_path.read_text())
    assert labels['geometry_hash'] == landscape['geometry_hash'] == GEOMETRY
    source_labels = deepcopy(labels['labels'])
    found = components()
    measurements = []
    for item in labels['labels']:
        box = PANELS[item['label']]
        area = found[tuple(box)]
        target_x = (box[0] + box[2]) / 2 * 768 / CROP[2]
        target_y = (box[1] + box[3]) / 2 * 1296 / CROP[3]
        old = deepcopy(item)
        icon_offset = labels['style']['icon_cell'] / 2 if item['icon'] else 0
        rect = item['rect']
        old_text_x = rect['x'] + rect['width'] / 2 + icon_offset
        old_text_y = rect['y'] + rect['height'] / 2
        item['x'], item['y'] = round(target_x - icon_offset), round(target_y)
        rect['x'] = round(target_x - icon_offset - rect['width'] / 2)
        rect['y'] = round(target_y - rect['height'] / 2)
        new_text_x = rect['x'] + rect['width'] / 2 + icon_offset
        new_text_y = rect['y'] + rect['height'] / 2
        measurements.append({'label': item['label'], 'target_text_paper_bbox': box,
            'threshold_component_pixels': area,
            'target_normalized_text_paper_center': [target_x, target_y],
            'old_text_paper_center': [old_text_x, old_text_y],
            'proposed_text_paper_center': [new_text_x, new_text_y],
            'old_anchor': [old['x'], old['y']], 'proposed_anchor': [item['x'], item['y']],
            'old_center_error_source_px': [old_text_x-target_x, old_text_y-target_y],
            'proposed_center_error_source_px': [new_text_x-target_x, new_text_y-target_y]})
    labels['version'] = 3
    labels['status'] = 'CONTRACT_READY_METADATA_RENDER_VALIDATION_NOT_RUN'
    labels['source_layout_version'] = 'map-layout-v2'
    labels['base_presentation'] = pin(old_label_path)
    labels['derivation'] = ('Explicit MAP-only anchor/rectangle translation from measured target text-paper centers. '
        'Retains six strings, existing/null interaction bindings, rectangle sizes, vectors, style and exact font. '
        'Overrides the frozen proposal display anchors only when explicitly adopted; canonical geometry is unchanged. '
        'No new render, visual acceptance, release admission or source review is inherited.')
    labels['provenance']['label_text_anchors_bindings'] = (
        'Strings and existing/null bindings remain frozen. This sidecar deliberately changes presentation anchors '
        'using target-measurements.json; no target raster is copied into product artwork.')
    labels['publication'] = 'Unpublished source correction; requires explicit matching consumer pin adoption.'

    # The downward PLAZA label otherwise overlaps two existing unbound garden beds.
    # Reposition those original atlas illustrations, with no geometry or texture changes.
    relocations = {'map_garden_plaza_south_west': [240, 468, 30, 20],
                   'map_garden_plaza_south_east': [405, 539, 36, 23]}
    changed = []
    for item in landscape['decorations']:
        if item['id'] in relocations:
            old = deepcopy(item)
            item['x'], item['y'], item['width'], item['height'] = relocations[item['id']]
            item['depth_y'] = item['y'] + item['height']
            changed.append({'id': item['id'], 'before': old, 'after': deepcopy(item)})
    assert len(changed) == 2
    landscape['version'] = 2
    landscape['status'] = 'CONTRACT_READY_METADATA_RENDER_VALIDATION_NOT_RUN'
    landscape['source_layout_version'] = 'map-layout-v2'
    landscape['base_presentation'] = pin(old_landscape_path)
    landscape['composition'] = ('Combine unchanged MAP props and these original atlas decorations by stable '
        'ascending depth_y; draw map-layout-v2 label-presentation.json afterward, then actual runtime markers. '
        'Adopt both sidecars together. WORLD ignores them; no extra PNG/font.')
    landscape['derivation'] = ('Only two unbound PLAZA garden placements differ from map-landscape-v1 to '
        'clear the corrected label; original atlas crops and all canonical metadata remain unchanged. '
        'Visibility/native/mobile follow-up is NOT_RUN.')
    write('label-presentation.json', labels)
    landscape['required_label_presentation'] = pin(HERE / 'label-presentation.json')
    write('landscape-presentation.json', landscape)
    write('target-measurements.json', {'target': pin(TARGET), 'map_crop_xywh': CROP,
        'normalization': {'width': 768, 'height': 1296},
        'method': '4-connected RGB components: R>210,G>200,B>170,max-min<65; area>80. '
                  'Named text-paper components are identified at the recorded original-crop bounds. '
                  'Compare paper-panel centers; preserve the original vector icon cell and rectangle sizes.',
        'limits': 'Component bounds exclude tinted icon cells and do not measure complete label edges, glyph '
                  'outlines, perspective, road topology or whole-scene fidelity. Integer anchor rounding is '
                  'measured separately; this is not a critic score or rendered acceptance.',
        'labels': measurements, 'planting_relocations': changed,
        'frozen_label_values_before': source_labels,
        'validation': {'structural': 'See verify.py', 'new_native_export': 'NOT_RUN',
                       'new_mobile_comparison': 'NOT_RUN'}})
    return measurements


if __name__ == '__main__':
    print(json.dumps({'status': 'METADATA_ONLY', 'labels': len(build()),
                      'raster_or_browser_jobs': 0}))
