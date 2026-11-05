"""Analysis artifacts only: original Photo2 is never product UI or rewritten."""
import hashlib
import json
import os
from pathlib import Path
from PIL import Image

root = Path(__file__).resolve().parents[4]
source = root / 'docs/mcr/design/parade-ink-2026-11-05/parade-ink.jpg'
assert hashlib.sha256(source.read_bytes()).hexdigest() == '9e7f17a666e1ecfc738a14b75bb3a166235a05c401f206aa24f26cf7463cbc27'
out = Path(os.environ['MCR_O04_OUTPUT'])
crop = (24, 60, 419, 847)
with Image.open(source) as original:
    reference = original.crop(crop)
    reference.resize((390, round(reference.height * 390 / reference.width)), Image.Resampling.LANCZOS).save(out / 'reference-frame-390.png')
(out / 'reference.json').write_text(json.dumps({'source': str(source.relative_to(root)), 'sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
    'originalSize': [1280, 853], 'crop': crop, 'normalization': '395 source pixels to 390 CSS px; original source bytes retained',
    'adaptation': 'O04 is unpictured. Compare shared title/type/rules/light surfaces/action anatomy; no invented Feed fields/window actions/filters on Reports. Actual app captures retain native CSS geometry.'}, indent=2) + '\n')
(out / 'comparison.html').write_text('''<!doctype html><html lang="en"><meta charset="utf-8"><title>O04 reference comparison</title>
<style>body{font:16px/1.5 system-ui;margin:24px;color:#171717;background:#f9f9f8}main{display:flex;gap:24px;align-items:start;flex-wrap:wrap}figure{margin:0}img{display:block;max-width:100%;height:auto}figcaption{max-width:390px;margin:8px 0}h1{font-size:24px}</style>
<h1>O04 Reports · Photo 2 anatomy comparison</h1><p>Original Feed frame normalized to 390px. Reports is unpictured; the application view uses real controller data. Actual screenshot is shown at its captured width, without page scaling.</p>
<main><figure><figcaption>Photo 2: frame, type, rules and action language</figcaption><img src="reference-frame-390.png" width="390" alt="Normalized original Photo 2 Feed frame"></figure>
<figure><figcaption>Actual default Admin Reports with immutable PI01 source overlay</figcaption><img src="own-390-queue-region.png" alt="Actual Reports queue region at 390px viewport"></figure></main></html>''')
