#!/usr/bin/env python3
"""Static source-alpha point analysis, not a browser/native render oracle."""
import hashlib
import json
import math
import sys
from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parent
read = lambda p: json.loads(p.read_text())
presentation = read(HERE/'presentation.json')
landscape = read(SOURCE/'map-layout-v2/landscape-presentation.json')
results = []
for era in ['revenge', 'parade', 'danger']:
    layout = read(SOURCE/'architecture-v2'/era/'layout.json')
    alpha = Image.open(SOURCE/'architecture-v2'/era/'atlas.png').getchannel('A')
    replaced = {o.get('replaces_prop_id') for o in layout['overview_representations']}
    props = [o for o in layout['placements'] if o['id'] not in replaced]
    props += layout['overview_representations'] + landscape['decorations']

    def opaque(obj, x, y):
        if not (obj['x'] <= x < obj['x']+obj['width'] and obj['y'] <= y < obj['y']+obj['height']):
            return False
        f = layout['frames'][obj['frame']]
        sx = f['x'] + min(f['width']-1, math.floor((x-obj['x'])/obj['width']*f['width']))
        sy = f['y'] + min(f['height']-1, math.floor((y-obj['y'])/obj['height']*f['height']))
        return alpha.getpixel((sx, sy)) >= 32

    for annex in presentation['annexes']:
        total = visible = 0
        occluders = {}
        ahead = sorted([p for p in props+presentation['annexes'] if p['depth_y'] > annex['depth_y']], key=lambda p: -p['depth_y'])
        for y in range(annex['y'], annex['y']+annex['height']):
            for x in range(annex['x'], annex['x']+annex['width']):
                if not opaque(annex, x+.5, y+.5):
                    continue
                total += 1
                hit = next((p for p in ahead if opaque(p, x+.5, y+.5)), None)
                if hit:
                    occluders[hit['id']] = occluders.get(hit['id'], 0)+1
                else:
                    visible += 1
        assert visible > 0 and total > visible
        results.append({'era': era, 'annex': annex['id'], 'source_alpha_points': total,
                        'unoccluded_points': visible, 'ratio': visible/total, 'occluders': occluders})
report = {'format': 'MCR_MAP_MASSING_STATIC_ALPHA_ANALYSIS',
          'presentation_sha256': hashlib.sha256((HERE/'presentation.json').read_bytes()).hexdigest(),
          'method': 'Destination pixel centers; nearest source alpha>=32; only later-depth object alpha occludes. Source metadata analysis without image output.',
          'limits': 'No native antialiasing, GPU masks, mobile visibility or real-avatar occlusion proof. Labels/gardens are rectangle-disjoint by build.py. Actual consumer renders remain NOT_RUN.',
          'results': results}
payload = json.dumps(report, indent=2)+'\n'
if '--verify' in sys.argv:
    assert (HERE/'visibility-analysis.json').read_text() == payload
else:
    (HERE/'visibility-analysis.json').write_text(payload)
print(json.dumps({'new_images': 0, 'cases': len(results), 'static_unoccluded_ratios': [round(r['ratio'], 4) for r in results], 'native': 'NOT_RUN'}))
