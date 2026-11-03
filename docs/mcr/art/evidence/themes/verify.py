#!/usr/bin/env python3
"""Verify AR04 resource bytes and native pixel sources; no product release claims."""
from io import BytesIO
from pathlib import Path
import hashlib
import json
import platform
from PIL import Image, __version__ as pillow_version
from jsonschema import Draft202012Validator
from referencing import Registry, Resource
from author import ROOT, OUT, HERE, NAMES, PALETTES, draw


def sha(data):
    return 'sha256:'+hashlib.sha256(data).hexdigest()


def ratio(a, b):
    def luminance(h):
        rgb = [int(h[i:i+2], 16)/255 for i in (1, 3, 5)]
        linear = [c/12.92 if c <= .04045 else ((c+.055)/1.055)**2.4 for c in rgb]
        return sum(c*w for c, w in zip(linear, [.2126, .7152, .0722]))
    low, high = sorted([luminance(a), luminance(b)])
    return (high+.05)/(low+.05)


def check_image(data, pin):
    assert sha(data) == pin['sha256'], 'resource hash mismatch'
    assert len(data) <= 16384, 'AR04 per-texture transfer budget exceeded'
    im = Image.open(BytesIO(data))
    assert im.format == 'PNG' and im.mode == 'RGBA' and im.size == (256, 128)
    im.load()
    assert im.size == (pin['width_px'], pin['height_px'])
    assert set(im.getchannel('A').get_flattened_data()) == {0, 255}, 'subpixel alpha/antialiasing'
    assert im.crop((0, 64, 256, 128)).getbbox() is None, 'reserved row contains art'
    assert im.crop((192, 32, 256, 64)).getbbox() is None, 'reserved GIDs15/16 contain art'
    return im


def must_fail(name, action):
    try:
        action()
    except AssertionError:
        return name
    raise AssertionError(f'negative probe silently passed: {name}')


def main():
    record = json.loads((OUT/'binding-inputs.json').read_text())
    common = json.loads((ROOT/'contracts/assets/v1/common.schema.json').read_text())
    registry = Registry().with_resource(common['$id'], Resource.from_contents(common))
    assert record['kind'] == 'ar04_theme_source_inputs' and record['release_binding_state'] == 'NOT_RUN'
    assert record['shared_tile_layout']['tiles'] == [dict(gid=i+1, identity=n,
        layer='terrain' if i < 8 else 'overdraw') for i, n in enumerate(NAMES)]
    assert record['shared_tile_layout']['reserved_gids'] == list(range(15, 33))
    assert record['shared_tile_layout']['columns'] == 8 and record['shared_tile_layout']['rows'] == 4
    assert record['shared_tile_layout']['tile_width_px'] == record['shared_tile_layout']['tile_height_px'] == 32
    reports, images, all_data = [], [], []
    for entry in record['themes']:
        for key, definition in [('texture', 'TextureMetadata'), ('provenance', 'Provenance'), ('theme_id', 'ThemeId')]:
            Draft202012Validator({'$ref':common['$id']+'#/$defs/'+definition}, registry=registry).validate(entry[key])
        theme, pin = entry['theme_id'], entry['texture']
        data = (ROOT/'frontend/public'/pin['path']).read_bytes()
        im = check_image(data, pin)
        assert sha((OUT/f'{theme.lower()}.tsrct').read_bytes()) == entry['editable_sha256']
        assert entry['provenance']['source_revision'].endswith(sha((HERE/'author.py').read_bytes()))
        expected = draw(theme)
        p = PALETTES[theme]
        # Compare rendered output to the integer-grid design, independent of the
        # Tesseract GPU renderer. This catches color transforms, offsets and bleed.
        for index, tile in enumerate(expected):
            actual = im.crop((index%8*32, index//8*32, index%8*32+32, index//8*32+32))
            pixels = []
            for row in tile:
                for token in row:
                    h = p[token] if token else '#000000'
                    pixels.append(tuple(int(h[i:i+2], 16) for i in (1, 3, 5))+(255 if token else 0,))
            assert list(actual.get_flattened_data()) == pixels, f'{theme} tile{index+1}: source/render mismatch'
        for index in range(8):
            x, y = index%8*32, index//8*32
            assert all(im.getpixel((x, y+n)) == im.getpixel((x+31, y+n)) for n in range(32)), 'horizontal seam mismatch'
            assert all(im.getpixel((x+n, y)) == im.getpixel((x+n, y+31)) for n in range(32)), 'vertical seam mismatch'
            assert im.crop((x, y, x+32, y+32)).getchannel('A').getextrema() == (255, 255)
        for index in range(8, 14):
            x, y = index%8*32, index//8*32
            tile = im.crop((x, y, x+32, y+32))
            assert tile.getbbox() and 0 < sum(a > 0 for a in tile.getchannel('A').get_flattened_data()) < 1024
        contrast = {f'ink_on_{token}':round(ratio(p['ink'], p[token]), 3)
                    for token in ['grass', 'pave', 'paper', 'accent', 'light']}
        assert contrast['ink_on_paper'] >= 4.5
        assert min(contrast.values()) >= 3
        reports.append(dict(theme=theme, texture=pin, bytes=len(data), decoded_rgba_bytes=256*128*4,
            unique_rgba_colors=len(im.getcolors(32769)), tile_seams='PASS: opposing boundary pixels identical on all eight terrain cells',
            source_render_pixel_parity='PASS: all authored pixels exact', contrast=contrast,
            alpha_mask_sha256=sha(im.getchannel('A').tobytes())))
        images.append(im); all_data.append(data)
    assert len(images) == 3 and len({r['theme'] for r in reports}) == 3
    assert len({r['alpha_mask_sha256'] for r in reports}) == 1, 'theme-dependent transparent footprint'
    assert len({r['texture']['sha256'] for r in reports}) == 3, 'themes not visually distinct'
    assert sum(len(d) for d in all_data) <= 49152
    negatives = [must_fail('wrong resource byte hash rejected', lambda: check_image(all_data[0]+b'x', reports[0]['texture']))]
    badpin = dict(reports[0]['texture'], width_px=128)
    negatives.append(must_fail('wrong declared dimensions rejected', lambda: check_image(all_data[0], badpin)))
    tampered = images[0].copy(); tampered.putpixel((255, 127), (1, 1, 1, 255))
    encoded = BytesIO(); tampered.save(encoded, format='PNG'); bad = encoded.getvalue()
    negatives.append(must_fail('reserved-cell paint rejected even with fresh byte hash',
        lambda: check_image(bad, dict(reports[0]['texture'], sha256=sha(bad)))))
    tampered = images[0].copy(); tampered.putpixel((0, 0), (1, 1, 1, 127))
    encoded = BytesIO(); tampered.save(encoded, format='PNG'); bad = encoded.getvalue()
    negatives.append(must_fail('antialiasing alpha rejected even with fresh byte hash',
        lambda: check_image(bad, dict(reports[0]['texture'], sha256=sha(bad)))))
    result = dict(scope='AR04 original source texture validation only', result='PASS',
        python=platform.python_version(), pillow=pillow_version, themes=reports,
        checks=['C06 source texture/provenance/theme-ID schema fragments (full product manifests NOT_RUN)',
                'fixed row-major shared tile semantics', 'three distinct source PNGs',
                'all authored integer pixels equal native rendered pixels', 'no terrain edge seam mismatch',
                'opaque terrain, identical cross-theme alpha masks, reserved GIDs empty',
                '16KiB PNG budget per theme / 48KiB total, 128KiB decoded RGBA per theme',
                'ink/paper contrast >=4.5; ink/grass/paving/accent/light contrast >=3'],
        negative_probes=negatives, product_manifest_binding='NOT_RUN', selected_avatar_compatibility='NOT_RUN',
        real_device_qa06='NOT_RUN', two_account_in01='NOT_RUN')
    (HERE/'pixel-validation.json').write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
