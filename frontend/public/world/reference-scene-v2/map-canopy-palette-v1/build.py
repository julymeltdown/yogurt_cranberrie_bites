#!/usr/bin/env python3
"""Bounded MAP palette correction using existing art; no render or raster output."""
import hashlib
import io
import json
import subprocess
import sys
from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parent
ROOT = SOURCE.parents[3]
read = lambda p: json.loads(p.read_text())
sha = lambda p: 'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()
pin = lambda p: {'path': str(p.relative_to(ROOT)), 'bytes': p.stat().st_size, 'sha256': sha(p)}
layout_path = SOURCE/'architecture-v2/parade/layout.json'
layout = read(layout_path)
atlas_path = SOURCE/'architecture-v2/parade/atlas.png'
reference_path = SOURCE/'map-service-labels-v1/original-target.jpg'
assert sha(reference_path) == 'sha256:2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23'
captured_path = 'docs/mcr/evidence/reference-scene-v5/architecture-v2/native-04/editable-parade.png'
captured_commit = 'a0db1770b048161c5df818f7e4e2417923c8d542'
blob = subprocess.check_output(['git', 'show', captured_commit+':'+captured_path], cwd=ROOT)
assert hashlib.sha256(blob).hexdigest() == '9156109e82573868a1a9d24585f305b890353a96889392b34988359c590ba524'
reference = Image.open(reference_path).convert('RGB')
actual = Image.open(io.BytesIO(blob)).convert('RGB')
assert reference.size == (1280, 853) and actual.size == (768, 1296)

# Matched source-coordinate ROI, excluding the two lateral red-roof facades.
# JPEG crop is approximate: original reference MAP crop scaled5/6.
crop = [975*5/6, 95*5/6, 248*5/6, 420*5/6]
roi = [210, 0, 542, 160]
red = lambda c: c[0] > 1.25*c[1] and c[0] > 1.15*c[2] and c[0]-c[1] >= 25
proxy = []
for name, image in [('attached_reference', reference), ('actual_native04', actual)]:
    count = total = 0
    for y in range(roi[1], roi[3]):
        for x in range(roi[0], roi[2]):
            xy = (int(crop[0]+(x+.5)/768*crop[2]), int(crop[1]+(y+.5)/1296*crop[3])) if name == 'attached_reference' else (x, y)
            count += red(image.getpixel(xy))
            total += 1
    proxy.append({'input': name, 'red_classified_points': count, 'sample_points': total, 'fraction': count/total})
assert proxy[0]['red_classified_points'] == 481 and proxy[1]['red_classified_points'] == 28935

ids = ['rooted_tree_01', 'rooted_tree_02', 'grove_north_west', 'grove_north_gap']
alpha = Image.open(atlas_path).getchannel('A')
cores = {}
for name in ['tree_red', 'tree_green']:
    frame = layout['frames'][name]
    mask = alpha.crop((frame['x'], frame['y'], frame['x']+frame['width'], frame['y']+frame['height'])).point(lambda v: 255 if v >= 32 else 0)
    cores[name] = {'frame': frame, 'bbox': mask.getbbox()}
replacements = []
for id in ids:
    original = next(p for p in layout['placements'] if p['id'] == id)
    assert original['frame'] == 'tree_red' and original['interaction_id'] is None
    assert original['y'] < 0 and original['depth_y'] in [136, 137]
    replacement = {k: original[k] for k in ['id', 'x', 'y', 'width', 'height', 'depth_y']}
    old_base = original['y']+cores['tree_red']['bbox'][3]/cores['tree_red']['frame']['height']*original['height']
    new_base = original['y']+cores['tree_green']['bbox'][3]/cores['tree_green']['frame']['height']*original['height']
    assert abs(new_base-old_base) < .01
    replacement.update(replaces_prop_id=id, frame='tree_green', interaction_id=None,
                       place_id=None, view='MAP_ONLY', authority='VIRTUAL_SCENERY_ONLY',
                       source_placement=original,
                       source_placement_sha256='sha256:'+hashlib.sha256(json.dumps(original, sort_keys=True, separators=(',', ':')).encode()).hexdigest(),
                       alpha_core_base_delta_px=new_base-old_base)
    replacements.append(replacement)

analysis = {'format': 'MCR_ATTACHED_REFERENCE_CANOPY_COLOR_COMPARISON',
    'reference': pin(reference_path), 'reference_dimensions': [1280, 853],
    'retained_actual': {'published_commit': captured_commit, 'path': captured_path,
                       'bytes': len(blob), 'sha256': 'sha256:'+hashlib.sha256(blob).hexdigest()},
    'attached_map_crop_approx_xywh': crop, 'normalized_source_roi_xyxy': roi,
    'rgb_rule': 'R>1.25G and R>1.15B and R-G>=25', 'results': proxy,
    'limits': 'Coarse fixed-band color proxy after actual image inspection; not foliage segmentation, a target-fidelity score or an independently rerun native render. JPEG sampling repeats low-resolution source pixels. No corrected output image is created.'}
presentation = {'format': 'MCR_MAP_CANOPY_PALETTE_V1', 'version': 1,
    'status': 'CONTRACT_READY_METADATA_RENDER_VALIDATION_NOT_RUN',
    'view': 'MAP_ONLY', 'theme_id': 'PARADE', 'production_status': 'NOT_ADMITTED',
    'geometry_hash': layout['geometry_hash'], 'source_layout': pin(layout_path),
    'source_atlas': pin(atlas_path), 'source_provenance': pin(SOURCE/'architecture-v2/parade/provenance.json'),
    'required_architecture': pin(SOURCE/'architecture-v2/atlas-handoff.json'),
    'required_service_labels': pin(SOURCE/'map-service-labels-v1/label-presentation.json'),
    'required_massing': pin(SOURCE/'map-venue-massing-v1/presentation.json'),
    'display_replacements': replacements, 'frame_alpha_cores': cores,
    'reason': 'Attached MAP upper district has muted green/yellow foliage; actual native04 has a broad crimson band between the north facades. Substitute existing green crowns in four existing positions, without scattering new props.',
    'scope': 'PARADE MAP only. Other era views and all World views unchanged. No reference evidence is inferred for other eras.',
    'unchanged': ['All34 World objects/source geometry/server feet', 'All4 prop IDs/destination rectangles/depths',
                  'All atlas/ground/font/character pixels', 'All stage/annex/garden/label placements and feature intents',
                  'Canonical place authority, cameras and release admission'],
    'consumer': 'Separately allocated optional follow-up; do not mutate or block sealed service-label/action/wings delivery. Replace only four MAP frame references after exact source/prop/pair validation; preserve stable depth ordering. No new objects or resources.',
    'validation': {'new_objects': 0, 'new_rasters_fonts': 0, 'new_browser_exports': 0,
                   'source_alpha_base_change_max_px': max(abs(r['alpha_core_base_delta_px']) for r in replacements),
                   'corrected_native_visibility_palette_mobile': 'NOT_RUN',
                   'density_framing_full_reference': 'IN_PROGRESS_NOT_ACCEPTED'}}

def emit(name, value):
    payload = json.dumps(value, indent=2)+'\n'
    if '--verify' in sys.argv:
        assert (HERE/name).read_text() == payload
    else:
        (HERE/name).write_text(payload)

emit('reference-analysis.json', analysis)
emit('presentation.json', presentation)
handoff = {'format': 'MCR_MAP_CANOPY_PALETTE_V1_HANDOFF',
    'status': presentation['status'], 'source_parent_commit': 'b50177e745af331dcbc9d5755dcb2c04585b5cff',
    'payloads': [pin(HERE/name) for name in ['build.py', 'README.md', 'reference-analysis.json', 'presentation.json']],
    'changes': 'Four existing PARADE MAP canopy frame substitutions only; no placement/object/geometry/resource changes.',
    'qualification': 'Actual attached reference and saved native image compared. Corrected consumer/native/mobile capture NOT_RUN; full density/framing/target fidelity remains open.',
    'adoption': 'Requires explicit separate consumer lease; current sealed service bundle unchanged.'}
emit('handoff.json', handoff)
print(json.dumps({'replacements': len(replacements), 'new_objects': 0, 'new_media': 0,
                  'reference_red_fraction': proxy[0]['fraction'], 'actual_red_fraction': proxy[1]['fraction'],
                  'max_alpha_core_base_delta_px': presentation['validation']['source_alpha_base_change_max_px'],
                  'corrected_native': 'NOT_RUN'}))
