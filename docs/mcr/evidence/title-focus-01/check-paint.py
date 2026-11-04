"""Evaluate authored CSS gradients against immutable native pixel coordinates.

Run only in a Resource21d allocated source-check slot. No browser or raster
output is produced. This is a paint model, not actual candidate rendering.
"""
from hashlib import sha256
from io import BytesIO
import json
from pathlib import Path
import re
import subprocess

from PIL import Image

ROOT = Path(__file__).resolve().parents[4]
BASE = '9e7952fcd89360f7b4fe43c69f22060b9aa55c74'
CSS = 'frontend/src/app/shell-chrome/shell-chrome.css'
SELECTOR = '.mcr-chrome-panel-header:has(> #panel-title:focus-visible)'
MAP_PACKET = '4600542ba8f5d16210075c93847fa60960809540'
MAP_PREFIX = 'docs/mcr/evidence/reference-scene-v5/map-cozy-v1/native-03/'
ME_PACKET = '5186f11017185a040e4ccde1c59589e140563ef0'
ME_PREFIX = 'docs/mcr/evidence/cozy-backgrounds-v1/native-02/'
DARK, PAPER = (37, 16, 23), (233, 225, 212)
RING, TEXT = (162, 35, 43), (242, 234, 223)


def blob(revision, path):
    return subprocess.check_output(['git', 'show', f'{revision}:{path}'], cwd=ROOT)


def split_top(value):
    result, start, depth = [], 0, 0
    for i, char in enumerate(value):
        depth += (char == '(') - (char == ')')
        if char == ',' and depth == 0:
            result.append(value[start:i].strip())
            start = i + 1
    assert depth == 0
    return result + [value[start:].strip()]


def position(value, length):
    value = value.strip()
    match = re.fullmatch(r'calc\(100% - (\d+)px\)', value)
    if match:
        return length - int(match[1])
    assert re.fullmatch(r'\d+(?:px)?', value), value
    return float(value.removesuffix('px'))


def gradient(value, width, height):
    assert value.startswith('linear-gradient(') and value.endswith(')')
    direction, *stops = split_top(value[len('linear-gradient('):-1])
    assert direction in ('to right', 'to bottom')
    length = width if direction == 'to right' else height
    parsed = []
    for stop in stops:
        match = re.fullmatch(r'(#[0-9a-f]{6}|transparent)(?:\s+(.*))?', stop)
        assert match, stop
        color = None if match[1] == 'transparent' else tuple(bytes.fromhex(match[1][1:]))
        locations = re.findall(r'calc\([^)]*\)|\d+(?:px)?', match[2] or '')
        # The only unpositioned stop is the terminal color; CSS places it at100%.
        if not locations:
            assert stop == stops[-1]
            locations = [str(length)]
        for location in locations:
            offset = position(location, length)
            # CSS gradient color-stop fixup: descending positions clamp to prior.
            parsed.append((max(offset, parsed[-1][0]) if parsed else offset, color))

    def sample(x, y):
        coordinate = x if direction == 'to right' else y
        if coordinate < parsed[0][0]:
            return parsed[0][1]
        previous_offset, previous_color = parsed[0]
        for offset, color in parsed[1:]:
            if coordinate < offset:
                # Our hard-stop gradients only have solid spans, never blending.
                assert previous_color == color, (previous_offset, offset)
                return color
            previous_offset, previous_color = offset, color
        return parsed[-1][1]

    return sample


raw = (ROOT / CSS).read_bytes()
source = raw.decode()
base = blob(BASE, CSS).decode()
pattern = re.escape(SELECTOR) + r'\s*\{([^{}]*)\}\n'
matches = list(re.finditer(pattern, source))
assert len(matches) == 1, 'Exactly the leased focus selector is required'
match = matches[0]
assert source[:match.start()] + source[match.end():] == base, 'All other source must remain byte-exact'
declarations = dict(part.strip().split(':', 1) for part in match[1].split(';') if part.strip())
assert set(declarations) <= {'background-color', 'background-image', 'background-size', 'background-position', 'background-repeat'}
declarations = {key: value.strip() for key, value in declarations.items()}
assert declarations['background-size'] == '100% 100%'
assert declarations['background-repeat'] == 'no-repeat'
assert 'background-position' not in declarations  # retained default0%0%
assert 'background-color' not in declarations  # retained dark fallback
images = split_top(declarations['background-image'])
assert len(images) == 3 and all('url(' not in item for item in images)


def paint(width, focused):
    layers = [gradient(item, width - 24, 44) for item in images]

    def sample(x, y):
        if focused:
            # Native screenshots are DPR1; evaluate the center of each pixel in
            # the retained header's padding-origin coordinate system.
            for layer in layers:
                color = layer(x - 12 + .5, y - 10 + .5)
                if color is not None:
                    return color
        return DARK

    return sample


report = json.loads(blob(MAP_PACKET, MAP_PREFIX + 'report.json'))
cases = []
for case in report['cases']:
    if case['mode'] != 'candidate':
        continue
    width = case['width']
    pin = next(screen for screen in case['screens'] if screen['id'] == 'map')
    data = blob(MAP_PACKET, MAP_PREFIX + pin['path'])
    assert len(data) == pin['bytes'] and sha256(data).hexdigest() == pin['sha256']
    image = Image.open(BytesIO(data)).convert('RGB')
    focused, unfocused = paint(width, True), paint(width, False)
    ring_count = glyph_count = 0
    for y in range(21, 44):
        for x in range(20, width - 146):
            rgb = image.getpixel((x, y))
            if rgb == RING:
                assert focused(x, y) == PAPER, (width, x, y, 'ring')
                ring_count += 1
            elif rgb != DARK:
                assert focused(x, y) == DARK, (width, x, y, 'visible glyph')
                glyph_count += 1
    assert glyph_count == 556
    # One exposed paper pixel outside each existing ring edge makes the backing
    # visible. Painting only underneath opaque red would leave no visible gain.
    for x in range(19, width - 145):
        assert focused(x, 20) == PAPER and focused(x, 44) == PAPER
    for y in range(20, 45):
        assert focused(19, y) == PAPER and focused(width - 146, y) == PAPER
    # Control interiors stay dark; the first transparent padding column beside
    # the title holds the right exposed paper pixel. All unfocused paint is dark.
    for y in range(10, 54):
        for x in range(12, width - 12):
            assert unfocused(x, y) == DARK
            if x < 19 or y < 20 or y > 44:
                assert focused(x, y) == DARK
            if x >= width - 145:
                assert focused(x, y) == DARK
    cases.append({'era': case['era'], 'width': width, 'retainedSha256': pin['sha256'],
                  'paperBackedRingPixels': ring_count, 'darkBackedVisibleGlyphPixels': glyph_count,
                  'controlInteriorDark': True, 'fourExposedOuterPaperEdges': True,
                  'unfocusedPaddingBoxDark': True})
assert len(cases) == 9

me_path = ME_PREFIX + 'candidate-me-focus-0-390.png'
me = blob(ME_PACKET, me_path)
assert sha256(me).hexdigest() == '497fec861655a6747e978af43bd74c4749b288fe92da0dd3b57ca67d300b2a9a'
me_image = Image.open(BytesIO(me)).convert('RGB')
for x in (21, 22):
    for y in range(24, 41):
        assert me_image.getpixel((x, y)) == TEXT
        assert paint(390, False)(x, y) == DARK

result = {'task': 'TITLE-FOCUS-01', 'status': 'SOURCE_MODEL_PASS_ACTUAL_RENDER_NOT_RUN',
          'base': BASE, 'cssSha256': sha256(raw).hexdigest(), 'cssBytes': len(raw),
          'selector': SELECTOR, 'allOutsideSelectorByteExact': True,
          'declarations': declarations, 'retainedMapPacket': MAP_PACKET, 'cases': cases,
          'retainedMePacket': ME_PACKET, 'retainedMePath': me_path,
          'unfocusedMeCreamCoreDarkBacking': True,
          'forcedColors': 'Non-URL gradients are forced to none by CSS Color Adjustment; actual browser check remains NOT_RUN',
          'limits': ['Analytical evaluation of authored hard-stop gradients, not candidate raster output',
                     'Retained native coordinate system only; actual DOM/raster, focus transitions and forced colors need Root capture',
                     'Synthetic route/transport evidence cannot certify live or multiuser acceptance']}
output = Path(__file__).with_name('paint-check.json')
output.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'status': result['status'], 'cssSha256': result['cssSha256'],
                  'retainedCases': len(cases), 'outputBytes': output.stat().st_size}))
