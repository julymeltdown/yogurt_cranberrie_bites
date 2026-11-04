"""Rebuild original MCR decorative backgrounds; stdlib only, no source images.

Run from any directory: python3 /absolute/path/to/author.py
All ornament geometry is editable below. No characters, lettering or controls.
"""
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parent
DEFS = '''<defs>
  <g id="heart"><path d="M0 5C-15-4-11-16-3-13L0-9 3-13C11-16 15-4 0 5Z" fill="currentColor"/></g>
  <g id="star"><path d="M0-11Q2-2 10 0 2 2 0 11-2 2-10 0-2-2 0-11Z" fill="currentColor"/></g>
  <g id="flower"><path d="M0-5C-11-20-22-5-9 2-24 9-10 23 0 10 10 23 24 9 9 2 22-5 11-20 0-5Z" fill="currentColor"/><circle cy="4" r="4" fill="#fff1ce"/></g>
  <g id="note" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M0 8V-10L12-14V4M0-5 12-9"/><ellipse cx="-3" cy="9" rx="4" ry="3" fill="currentColor" stroke="none"/><ellipse cx="9" cy="5" rx="4" ry="3" fill="currentColor" stroke="none"/></g>
  <g id="cloud"><path d="M-34 7C-49 7-50-13-35-15-36-31-14-36-5-23 8-39 31-28 29-14 48-15 51 8 32 9Z" fill="currentColor"/></g>
  <g id="sprig" fill="none" stroke="#a4b49a" stroke-width="2" stroke-linecap="round"><path d="M0 0Q-6-18 1-40"/><path d="M-2-10C-21-8-19-23-3-16M-2-23C15-23 15-36 0-30" fill="#b9c5a8" stroke="none"/></g>
</defs>'''

def use(name, x, y, scale=1, color="#d9a9b7", rotate=0, opacity=1):
    return f'<use href="#{name}" transform="translate({x} {y}) rotate({rotate}) scale({scale})" color="{color}" opacity="{opacity}"/>'

def svg(width, height, layers):
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" fill="none">
<!-- Original background ornament. No text, controls, portraits or source-pack pixels. -->
{DEFS}
{chr(10).join(layers)}
</svg>
'''

night = ['<rect width="390" height="660" fill="#624250"/>',
    '<path d="M0 0H390V142Q300 117 235 147T87 148T0 133Z" fill="#6b4b5b"/>',
    '<circle cx="283" cy="341" r="83" fill="#efd6bd"/>',
    '<path d="M-28 405C-57 354-13 307 29 325 33 280 101 267 124 311 161 293 197 332 180 363 223 362 230 425 186 438H-28Z" fill="#d6afbb"/>',
    '<path d="M224 415C200 389 218 352 250 353 238 319 278 286 309 312 342 272 402 295 399 337 442 337 456 400 415 425Z" fill="#c7b6d1"/>',
    '<path d="M0 423Q50 392 94 422T188 420T285 422T390 415V660H0Z" fill="#503b4a"/>',
    '<path d="M0 434Q54 418 106 437T210 434T313 436T390 429V461H0Z" fill="#8f7389"/>']
# Low, rounded cloud banks remain below the wordmark and above its printed copy.
for x, y, s, c in [(16, 381, 1.35, '#f1d8d1'), (369, 367, 1.35, '#ead8e8'),
                     (3, 441, .9, '#d0b5c4'), (386, 439, .85, '#d0b5c4')]:
    night.append(use('cloud', x, y, s, c))
for x, y, s, c in [(19, 80, .6, '#e6bdc3'), (366, 110, .52, '#d2c6e3'),
    (31, 235, .72, '#e4bfcc'), (356, 255, .65, '#d7c4e6'),
    (18, 330, .5, '#f2d49e'), (375, 324, .5, '#f2d49e'),
    (27, 490, .65, '#e5b4c3'), (364, 495, .65, '#cbbddd')]:
    night.append(use('star', x, y, s, c))
for x, y, s in [(29, 302, .75), (361, 351, .65), (26, 566, .55), (368, 577, .55)]:
    night.append(use('heart', x, y, s, '#dca6b5', -10 if x < 100 else 14))
for x, y, s, r in [(31, 415, .75, -17), (357, 414, .85, 16)]:
    night.append(use('flower', x, y, s, '#dba3b5', r))
for x, y, s, r in [(23, 188, .72, -12), (366, 189, .65, 12)]:
    night.append(use('note', x, y, s, '#bcafce', r))

paper = ['<rect width="390" height="720" fill="#fbf2df"/>',
    '<ellipse cx="-83" cy="210" rx="218" ry="320" fill="#f4dfd9"/>',
    '<ellipse cx="478" cy="120" rx="215" ry="300" fill="#eee4f0"/>',
    '<ellipse cx="468" cy="705" rx="206" ry="239" fill="#e9eddd"/>',
    '<ellipse cx="-88" cy="718" rx="226" ry="242" fill="#f3e4d1"/>']
for x, y, name, scale, color in [(15, 155, 'heart', .45, '#dbbac2'),
    (376, 211, 'star', .5, '#d0c4dd'), (13, 368, 'star', .45, '#ddc99c'),
    (379, 412, 'heart', .42, '#ddbcc2'), (18, 563, 'note', .55, '#c2cbb8'),
    (375, 603, 'star', .5, '#d8bfd3')]:
    paper.append(use(name, x, y, scale, color, opacity=.58))

top = []
# Optional transparent decorative layer; existing foreground must remain visible.
for x, y, s, c in [(7, 26, 2.5, '#e7d6e7'), (378, 31, 2.4, '#f1d9d5'),
    (51, 4, 1.5, '#f3e4c9'), (335, 0, 1.5, '#e4e9d7')]:
    top.append(use('cloud', x, y, s, c))
top.append('<path d="M-12 20Q195 94 402 20" stroke="#d5bcbd" stroke-width="1.6" stroke-dasharray="2 5" stroke-linecap="round"/>')
for x, y, s, c, r in [(44, 38, .6, '#d59daf', -14), (101, 54, .5, '#c2b2d2', 9),
    (195, 60, .5, '#d59daf', 0), (289, 54, .5, '#c2b2d2', -9), (346, 38, .6, '#d59daf', 14)]:
    top.append(use('heart' if x in [44, 195, 346] else 'star', x, y, s, c, r))
top.extend([use('note', 20, 95, .8, '#b7bfa6', -14), use('note', 366, 91, .8, '#baabc9', 14),
    use('star', 20, 153, .5, '#d8b9bb'), use('star', 373, 164, .5, '#c7bfd8')])

bottom = ['<path d="M0 121Q30 82 60 118 91 97 121 136 154 120 183 147 214 118 244 137 274 100 305 121 338 76 366 111 380 105 390 112V180H0Z" fill="#e1e5cf"/>',
    '<path d="M0 158Q12 143 26 156 39 135 57 153 74 143 89 162 108 147 125 162 145 150 163 164 186 153 205 166 226 150 247 164 266 145 283 161 303 144 323 157 342 138 357 155 377 140 390 154V180H0Z" fill="#d2d9bd"/>',
    '<path d="M0 173Q24 163 46 173T90 172T137 174T184 173T232 174T280 173T328 174T390 172V180H0Z" fill="#bac7a5"/>']
for x, y, r in [(22, 169, -10), (53, 175, 12), (337, 173, -12), (372, 167, 10)]:
    bottom.append(use('sprig', x, y, 1 if x in [22, 372] else .65, rotate=r))
for x, y, s, r in [(22, 117, .95, -14), (56, 146, .7, 12), (335, 143, .65, 8), (372, 118, 1, 16)]:
    bottom.append(use('flower', x, y, s, '#dfafbd', r))
bottom.extend([use('heart', 13, 73, .55, '#c3b2d3', -15), use('heart', 376, 74, .55, '#dba7b8', 15),
    use('note', 54, 113, .65, '#b4bc9e', -8), use('star', 344, 96, .65, '#c9b6d3')])

customize = ['<rect width="390" height="700" fill="#fbf2df"/>',
    '<path d="M0 0H390V20Q369 13 348 21T306 20T264 21T222 20T180 21T138 20T96 21T54 20T0 21Z" fill="#eee1df"/>',
    '<ellipse cx="-23" cy="665" rx="206" ry="237" fill="#f0dedd"/>',
    '<ellipse cx="414" cy="677" rx="205" ry="232" fill="#e9e5ed"/>',
    '<path d="M0 686Q38 659 75 684T150 684T225 684T300 684T390 677V700H0Z" fill="#e0e6cf"/>']
for x, y, name, s, c in [(14, 98, 'star', .4, '#d3c0d9'), (377, 221, 'heart', .35, '#dcb8c4'),
    (12, 398, 'heart', .35, '#dcb8c4'), (379, 505, 'star', .4, '#c1caae'),
    (20, 654, 'note', .65, '#c0b2ce'), (370, 657, 'note', .65, '#c0b2ce'),
    (42, 683, 'flower', .65, '#dbb4c0'), (346, 685, 'flower', .65, '#dbb4c0')]:
    customize.append(use(name, x, y, s, c, opacity=.7))

assets = [
    ('landing-garden-night.svg', 390, 660, night, False, 'A01 existing poster background only; white print stays unchanged'),
    ('sign-in-paper.svg', 390, 720, paper, False, 'A02 paper surface; quiet form center x=60..330'),
    ('sign-in-clouds-top.svg', 390, 240, top, True, 'A02 optional upper decorative background; never cover existing foreground'),
    ('sign-in-garden-bottom.svg', 390, 180, bottom, True, 'A02 optional bottom decorative background; no positioning or clipping changes'),
    ('customize-paper.svg', 390, 700, customize, False, 'U02 outer panel paper only; preview, canvas and option art excluded'),
]
manifest = {'id': 'mcr-cute-entry-backgrounds', 'revision': 1, 'authorship': 'Original hand-authored SVG geometry; no raster/source-image derivation',
    'source_base': 'bfa0193939c3eebd6c80ade85083c6530c6875e1',
    'reference_sha256': '2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23',
    'reference_use': 'Human collage inspected for existing foreground contrast and composition only; no traced/copied pixels or paths',
    'dependencies': [], 'character_assets': [], 'assets': []}
for filename, width, height, layers, alpha, usage in assets:
    data = svg(width, height, layers).encode()
    (ROOT / filename).write_bytes(data)
    manifest['assets'].append({'file': filename, 'url': '/backgrounds/cute-entry-v1/' + filename,
        'viewBox': [0, 0, width, height], 'transparent_background': alpha, 'bytes': len(data),
        'sha256': hashlib.sha256(data).hexdigest(), 'usage': usage})
(ROOT / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(f'Wrote {len(assets)} backgrounds and manifest ({sum(x["bytes"] for x in manifest["assets"])} SVG bytes)')
