#!/usr/bin/env python3
"""Versioned virtual feature labels; no canonical geometry or raster editing."""
import copy
import hashlib
import json
import sys
from pathlib import Path
from fontTools.ttLib import TTFont
from PIL import Image

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parent
ROOT = SOURCE.parents[3]
read = lambda p: json.loads(p.read_text())
sha = lambda p: 'sha256:' + hashlib.sha256(p.read_bytes()).hexdigest()
pin = lambda p: {'path': str(p.relative_to(ROOT)), 'bytes': p.stat().st_size, 'sha256': sha(p)}
base_path = SOURCE/'map-layout-v2/label-presentation.json'
base = read(base_path)
assert sha(base_path) == 'sha256:f5c3207617919fb15a8a2d9707c8cc64765c8289523b403c6e6dbb029b1cda54'
reference = HERE/'original-target.jpg'
assert reference.stat().st_size == 482737
assert sha(reference) == 'sha256:2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23'
assert Image.open(reference).size == (1280, 853)

# Original filled polygons in the same24-unit icon grid as the frozen vectors.
rect = lambda x, y, w, h: [[x, y], [x+w, y], [x+w, y+h], [x, y+h]]
feed = [rect(2, 3, 20, 2), rect(2, 5, 2, 13), rect(20, 5, 2, 13),
        rect(4, 16, 16, 2), [[5, 18], [5, 22], [10, 18]],
        rect(6, 7, 12, 2), rect(6, 11, 9, 2)]
wall = [rect(x, y, 4, 4) for y in [3, 10, 17] for x in [3, 10, 17]]
mapping = {'MERCH': ('WALL', 'wall', 'wall'), 'FOOD': ('FEED', 'feed', 'feed'),
           'MEETUP ZONE': ('MEET', 'people', 'meet'),
           'GIVEAWAY BOOTH': ('GIFTS', 'gift', 'gifts')}

presentation = copy.deepcopy(base)
presentation.update(version=4, status='CONTRACT_READY_METADATA_RENDER_VALIDATION_NOT_RUN',
                    view='MAP_ONLY', production_status='NOT_ADMITTED',
                    source_layout_version='map-service-labels-v1',
                    source_lineage_commit='9a441859e4ec9c97e28f7e5326307e62cf95cfb2',
                    publication='SOURCE_ONLY. Explicit new consumer adoption, route/action checks and native qualification required.',
                    required_base_labels=pin(base_path),
                    required_landscape=pin(SOURCE/'map-layout-v2/landscape-presentation.json'),
                    required_architecture=pin(SOURCE/'architecture-v2/atlas-handoff.json'),
                    required_massing=pin(SOURCE/'map-venue-massing-v1/presentation.json'),
                    supplied_reference=pin(reference))
presentation['icons'] = {key: copy.deepcopy(base['icons'][key]) for key in ['stage', 'people', 'gift']}
presentation['icons'].update(feed=feed, wall=wall)
presentation['icon_cutouts'] = {}
for label in presentation['labels']:
    source_label = label['label']
    label['source_label'] = source_label
    label['interaction_id'] = None
    label['place_id'] = None
    label['navigation_intent'] = None
    label['authority'] = 'VIRTUAL_SCENERY_ONLY'
    label['binding_policy'] = 'Virtual scenery only; no activation or canonical-place claim.'
    if source_label in mapping:
        text, icon, intent = mapping[source_label]
        label.update(label=text, lines=[text], icon=icon, navigation_intent=intent,
                     authority='VIRTUAL_FEATURE_NAVIGATION_ONLY',
                     binding_policy='Closed virtual feature navigation only; no physical destination or canonical place binding.')
presentation['composition'] = 'Paint after MAP scenery in unchanged explicit rectangles. Center a single text line vertically within its existing paper panel, preserving exact font and size. WORLD has no feature-label overlay. Actual navigation/44px screen targets are consumer-owned.'
presentation['authority'] = 'All six annotation interaction_id/place_id fields are null. MAIN STAGE/PLAZA are virtual scenery; WALL/FEED/MEET/GIFTS are closed feature intents. Canonical stage_info/gifts_info identities and reviewed-place catalog/safety remain separate and unchanged. No food/retail/physical route or eligibility is introduced.'
presentation['runtime_resources'] = 'No new runtime PNG or font. Original target JPEG is reference/provenance only, never a product texture. Frozen architecture/landscape/massing inputs are pinned lineage, not automatic activation.'
presentation['derivation'] = 'Human semantic correction: four names/two new original glyphs, with unchanged six-entry order, source_label lineage, rectangles, anchors, style and font. Existing canonical geometry/authority is not renamed.'
presentation['provenance']['icons'] = 'Original hand-authored feed/message and wall/mural-grid polygons in this build.py; existing stage/people/gift vectors exact. No utensil/shopping glyph in this version; no raster extraction.'
presentation['provenance']['label_text_anchors_bindings'] = 'New human-authorized virtual semantic annotation, independently pinned atop the old geometry/art pair; all six canonical bindings removed from this annotation only. Source geometry, canonical identities and source originals untouched.'
presentation['baseline_pin_policy'] = 'required_* pins identify frozen existing inputs. New consumer validates the old art pair first, then this separately pinned semantic layer. Do not mutate or reinterpret canonical place IDs; required_massing does not itself activate unadopted wings.'

font_path = ROOT/base['font']['path']
assert pin(font_path) == base['font']
font = TTFont(font_path)
cmap = font.getBestCmap()
metrics = font['hmtx'].metrics
units = font['head'].unitsPerEm
text_metrics = []
for before, label in zip(base['labels'], presentation['labels']):
    assert [before[k] for k in ['x', 'y', 'rect']] == [label[k] for k in ['x', 'y', 'rect']]
    assert label['interaction_id'] is None and label['place_id'] is None
    assert label['navigation_intent'] in [None, 'feed', 'wall', 'meet', 'gifts']
    available = label['rect']['width']-2*base['style']['padding']-(base['style']['icon_cell'] if label['icon'] else 0)
    advance = sum(metrics[cmap[ord(c)]][0] for c in label['label'])/units*base['style']['font_size']
    assert advance <= available
    text_metrics.append({'source_label': label['source_label'], 'label': label['label'],
                         'advance_px': advance, 'available_text_width_px': available,
                         'line_count': len(label['lines']), 'rectangle_height': label['rect']['height']})
assert presentation['style'] == base['style']
assert sorted(l['navigation_intent'] for l in presentation['labels'] if l['navigation_intent']) == ['feed', 'gifts', 'meet', 'wall']
assert all(0 <= x <= 24 and 0 <= y <= 24 for polygons in presentation['icons'].values() for polygon in polygons for x, y in polygon)
assert all(presentation['icons'][k] == base['icons'][k] for k in ['stage', 'people', 'gift'])

qualification = {
    'status': 'SOURCE_STRUCTURAL_CHECKS_PASS_RENDER_ACTION_CHECKS_NOT_RUN',
    'reference': pin(reference), 'reference_dimensions': [1280, 853],
    'reference_comparison': 'Attached original actually viewed. Preserve its six-panel composition while applying human feature semantics; source-only vectors do not prove native typography, service actions, density, framing or full target fidelity.',
    'changed_strings': {k: v[0] for k, v in mapping.items()},
    'changed_glyphs': ['feed', 'wall'], 'preserved_glyphs': ['stage', 'people', 'gift'],
    'null_canonical_annotation_bindings': 6, 'closed_feature_intents': 4,
    'style_font_anchors_rectangles_exact': True, 'text_metrics': text_metrics,
    'no_new_runtime_rasters_fonts': True,
    'consumer_native': 'NOT_RUN', 'actual_navigation_actions': 'NOT_RUN',
    'reviewed_place_path_safety': 'CONSUMER_OWNED_NOT_RUN',
    'original_target_fidelity': 'IN_PROGRESS_NOT_ACCEPTED', 'production': 'NOT_ADMITTED'
}
for name, value in [('label-presentation.json', presentation), ('qualification.json', qualification)]:
    payload = json.dumps(value, indent=2)+'\n'
    if '--verify' in sys.argv:
        assert (HERE/name).read_text() == payload
    else:
        (HERE/name).write_text(payload)

# Final consumer freeze is label/glyph-only. No derived landscape, wing or
# display-replacement output is part of this source handoff.
payload_names = ['build.py', 'verify.py', 'README.md', 'original-target.jpg',
                 'label-presentation.json', 'qualification.json']
handoff = {'format': 'MCR_MAP_SERVICE_LABELS_V1_HANDOFF',
    'status': 'CONTRACT_READY_METADATA_RENDER_ACTION_VALIDATION_NOT_RUN',
    'production_status': 'NOT_ADMITTED',
    'source_parent_commit': '9a441859e4ec9c97e28f7e5326307e62cf95cfb2',
    'labels': pin(HERE/'label-presentation.json'),
    'payloads': [pin(HERE/name) for name in payload_names],
    'baseline_pin_policy': 'New labels pin immutable OLD base/landscape/architecture/massing lineage. Existing two wings can only be activated by explicit consumer allocation. No new paired art derivative or display replacement.',
    'consumer_counts': {'feature_actions': 4, 'scenic_labels': 2, 'canonical_annotation_bindings': 0},
    'validation': 'build.py --verify and verify.py are source/byte/authority checks, not browser/native/action execution or full target acceptance.',
    'consumer_followup': 'Root under exact recorded consumer lease maps the closed intents to actual routes, removes only annotation-owned old hitareas, keeps genuine reviewed-place catalog/safety separate and validates44px targets/native/actions/reference. No manager review gate.',
    'known_visual_limit': 'Existing groceries/stall imagery is not changed by this label-only source. It does not create a food service; neutral display art remains a separate unresolved visual source concern.',
    'source_originals': 'Supplied JPEG copied byte-exact. Prior source/wing/character/geometry/font/PNG artifacts unchanged.'}
payload = json.dumps(handoff, indent=2)+'\n'
if '--verify' in sys.argv:
    assert (HERE/'handoff.json').read_text() == payload
else:
    (HERE/'handoff.json').write_text(payload)

print(json.dumps({'labels': len(presentation['labels']), 'virtual_feature_intents': 4,
                  'null_interaction_and_place_bindings': 6, 'new_runtime_images': 0,
                  'rectangles_anchors_style_font': 'EXACT_BASE', 'status': qualification['status']}))
