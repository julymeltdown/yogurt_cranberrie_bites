#!/usr/bin/env python3
"""Original AR04 integer-grid artwork → native editable Tesseract rectangles.

No input imagery is sampled, traced, extracted, or embedded. Run from any cwd.
Requires Tesseract 0.4.0. All writes stay inside the assigned theme/evidence paths.
"""
from pathlib import Path
import argparse
import json
import subprocess

ROOT = Path(__file__).resolve().parents[5]
OUT = ROOT / 'frontend/public/world/themes'
HERE = Path(__file__).resolve().parent
TSRCT = Path.home() / 'Library/Application Support/Tesseract/bin/tsrct'
NAMES = ['lawn', 'paving', 'alternate_paving', 'gravel', 'stage_deck', 'roof',
         'wall', 'water', 'shrub_canopy', 'flower', 'banner', 'truss', 'sign', 'lamp']
PALETTES = {
    'REVENGE': dict(ink='#211F28', metal='#62616B', light='#C6BFBA', paper='#EFE2CD',
                    accent='#BE435B', bright='#DF6A77', grass='#6A7C56', grass2='#7F8E60',
                    grass3='#536A49', pave='#AFA394', pave2='#C4B8A6', joint='#8A827B',
                    leaf='#405D49', leaf2='#587956', leaf3='#849262', water='#496779',
                    water2='#698392', roof='#696271', roof2='#8B7C87'),
    'PARADE': dict(ink='#25272D', metal='#747B83', light='#C1C4BE', paper='#F1E9D5',
                   accent='#DFD4B4', bright='#FFF3CF', grass='#788475', grass2='#8E9880',
                   grass3='#637365', pave='#B5B6AE', pave2='#CBCCC1', joint='#8D9690',
                   leaf='#4C665C', leaf2='#6D8470', leaf3='#9CA88B', water='#5E7682',
                   water2='#8A9BA4', roof='#646D78', roof2='#92989A'),
    'DANGER': dict(ink='#223340', metal='#4E7382', light='#A9CED0', paper='#FFF4D2',
                   accent='#F6D447', bright='#FF8A68', grass='#91A95B', grass2='#AABC68',
                   grass3='#708D50', pave='#C6BBA1', pave2='#DECFAD', joint='#9F987F',
                   leaf='#46775B', leaf2='#70A364', leaf3='#A9C96E', water='#337F93',
                   water2='#63B5C0', roof='#4E8F9B', roof2='#79B8BF'),
}


def draw(theme):
    """14 authored cells, 18 explicitly blank reserved cells. Tokens are editable."""
    tiles = []
    t = None

    def new(base=None):
        nonlocal t
        t = [[base for _ in range(32)] for _ in range(32)]
        tiles.append(t)

    def r(x, y, w, h, c):
        assert x >= 0 and y >= 0 and x+w <= 32 and y+h <= 32
        for yy in range(y, y+h):
            t[yy][x:x+w] = [c]*w

    def dot(x, y, c):
        r(x, y, 1, 1, c)

    new('grass')
    for x, y in [(4, 4), (17, 3), (26, 8), (10, 14), (22, 21), (5, 26), (14, 27)]:
        r(x, y, 1, 2, 'grass3'); dot(x+1, y+1, 'grass2')
    for x, y in [(7, 8), (23, 13), (14, 22), (28, 28)]:
        r(x, y, 2, 1, 'grass2')

    new('joint')
    for y in [1, 17]:
        for x in [1, 17]:
            r(x, y, 14, 14, 'pave'); r(x, y, 14, 1, 'pave2')
            r(x+2, y+12, 10, 1, 'joint')
    new('joint')
    for y in [1, 9, 17, 25]:
        for x in ([1, 17] if y % 16 == 1 else [1, 9, 25]):
            w = min(14 if y % 16 == 1 else 6 if x != 9 else 14, 31-x)
            r(x, y, w, 6, 'pave2'); r(x, y+5, w, 1, 'pave')

    new('pave')
    for x, y in [(4, 4), (12, 3), (23, 5), (7, 13), (17, 12), (27, 17), (4, 24), (14, 26), (24, 25)]:
        r(x, y, 2, 1, 'joint'); dot(x+1, y+1, 'pave2')
    new('ink')
    for y in [3, 11, 19, 27]:
        r(1, y, 30, 1, 'metal'); r(2, y+1, 28, 1, 'roof')
        for x in [5, 25]:
            dot(x, y+3 if y < 27 else y-2, 'light')
    new('roof')
    for y in [2, 10, 18, 26]:
        r(1, y, 30, 1, 'roof2'); r(1, y+1, 30, 1, 'metal')
        for x in [6, 22]:
            r(x, y+3, 1, 2, 'ink')
    new('joint')
    for y in [1, 9, 17, 25]:
        for x in ([1, 17] if y % 16 == 1 else [1, 9, 25]):
            w = min(14 if y % 16 == 1 else 6 if x != 9 else 14, 31-x)
            r(x, y, w, 6, 'light'); r(x, y+5, w, 1, 'metal')
    new('water')
    for x, y, w in [(4, 6, 10), (19, 12, 8), (7, 20, 9), (22, 27, 5)]:
        r(x, y, w, 1, 'water2'); r(x+2, y+2, max(2, w-5), 1, 'roof')

    # Shrub canopy: stepped silhouette, sparse light clusters, no external shadow.
    new()
    for x, y, w, h in [(7, 4, 18, 24), (3, 9, 26, 15), (5, 6, 22, 20), (1, 13, 30, 7)]:
        r(x, y, w, h, 'ink')
    for x, y, w, h in [(8, 6, 16, 19), (5, 10, 22, 12), (3, 14, 26, 4)]:
        r(x, y, w, h, 'leaf')
    for x, y, w, h in [(8, 7, 7, 5), (17, 9, 6, 6), (5, 14, 7, 5), (14, 17, 7, 5), (22, 15, 4, 4)]:
        r(x, y, w, h, 'leaf2'); r(x, y, max(2, w-3), 1, 'leaf3')

    # Three flowers: precisely identical masks, no loose petals outside the cell.
    new()
    for x, y in [(7, 9), (21, 7), (15, 19)]:
        r(x, y+3, 1, min(9, 30-y-3), 'leaf'); r(x+1, y+7, 3, 1, 'leaf2')
        r(x-3, y-1, 7, 3, 'ink'); r(x-1, y-3, 3, 7, 'ink')
        r(x-2, y, 5, 1, 'accent'); r(x, y-2, 1, 5, 'bright'); dot(x, y, 'paper')

    # Banner has one stable pole and stepped fabric; motifs are original geometry.
    new()
    r(5, 2, 2, 28, 'ink'); r(6, 3, 1, 26, 'metal'); r(3, 29, 7, 2, 'ink')
    r(7, 4, 20, 21, 'ink'); r(9, 6, 16, 16, 'accent')
    r(8, 24, 6, 2, 'ink'); r(20, 24, 7, 2, 'ink')
    # Confined emblem differences do not alter the silhouette.
    if theme == 'REVENGE':
        r(14, 10, 6, 9, 'ink'); r(12, 12, 10, 5, 'ink')
        r(15, 11, 4, 7, 'paper'); r(13, 13, 8, 3, 'paper'); r(16, 13, 2, 3, 'accent')
    elif theme == 'PARADE':
        r(11, 8, 12, 2, 'ink'); r(11, 18, 12, 2, 'ink')
        for x in range(12, 21, 3):
            r(x, 12, 2, 3, 'ink'); r(x+1, 15, 2, 1, 'ink')
    else:
        for x, y, w, h in [(17, 8, 4, 3), (15, 11, 5, 3), (13, 14, 7, 2), (16, 16, 3, 2), (14, 18, 3, 2)]:
            r(x, y, w, h, 'ink')

    new()
    r(3, 2, 26, 3, 'ink'); r(4, 2, 24, 1, 'light')
    for x in [3, 25]:
        r(x, 5, 4, 25, 'ink'); r(x, 5, 1, 24, 'light')
        for y in [7, 15, 23]:
            for step in range(3):
                dot(x+step, y+step, 'metal'); dot(x+3-step, y+step+3, 'light')
    r(2, 30, 6, 2, 'ink'); r(24, 30, 6, 2, 'ink')
    for x in [10, 20]:
        r(x, 5, 3, 2, 'ink'); r(x, 7, 3, 2, 'accent')

    new()
    r(6, 4, 20, 20, 'ink'); r(4, 6, 24, 16, 'ink')
    r(6, 6, 20, 16, 'paper'); r(7, 7, 18, 1, 'light')
    r(9, 24, 3, 7, 'ink'); r(20, 24, 3, 7, 'ink')
    r(10, 25, 1, 5, 'metal'); r(21, 25, 1, 5, 'metal')
    # A universal abstract landmark diamond, never an arrow or operational label.
    for x, y, w in [(14, 10, 4), (12, 12, 8), (10, 14, 12), (12, 16, 8), (14, 18, 4)]:
        r(x, y, w, 2, 'ink')
    r(14, 13, 4, 4, 'accent')

    new()
    r(14, 9, 3, 21, 'ink'); r(15, 10, 1, 19, 'metal'); r(11, 30, 9, 2, 'ink')
    r(10, 4, 11, 2, 'ink'); r(8, 6, 15, 8, 'ink'); r(10, 14, 11, 2, 'ink')
    r(10, 7, 11, 5, 'accent'); r(11, 7, 2, 4, 'paper'); r(15, 7, 1, 5, 'ink')
    return tiles


def document(theme, tiles):
    layers = []
    p = PALETTES[theme]
    # Native one-pixel-high runs preserve exact pixel identity and editable color.
    for index, tile in enumerate(tiles):
        for y, row in enumerate(tile):
            x = 0
            while x < 32:
                c, end = row[x], x+1
                while end < 32 and row[end] == c:
                    end += 1
                if c is not None:
                    color = [int(p[c][i:i+2], 16)/255 for i in (1, 3, 5)] + [1]
                    layers.append(dict(type='Rect', id=len(layers)+1,
                        name=f'{index+1:02} {NAMES[index]} / {c} / {x},{y}',
                        blendMode='normal', activeRange=dict(start=0, duration=3000),
                        transform=dict(anchorPoint=[0, 0], position=[(index%8)*32+x, (index//8)*32+y],
                                       scale=[100, 100], rotation=0, opacity=100),
                        rect=dict(size=[end-x, 1], fillColor=color)))
                x = end
    return {'$schema':'https://jerboa.dev/schemas/fx-composition/editable/v1/document.schema.json',
            'formatVersion':1, 'dimensions':{'width':256, 'height':128},
            'colorManagement':{'workingSpace':'legacyDisplay'}, 'duration':3,
            'composition':{'id':'main', 'name':f'AR04 {theme} original pixels', 'layers':layers[::-1]}}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--render', action='store_true')
    args = parser.parse_args()
    for theme in PALETTES:
        name = theme.lower()
        doc = document(theme, draw(theme))
        path = HERE / f'{name}.editable.json'
        path.write_text(json.dumps(doc, separators=(',', ':'))+'\n')
        if args.render:
            subprocess.run([str(TSRCT), 'project', 'commit', '--project', str(OUT/f'{name}.tsrct'), '--file', str(path)], check=True)
            subprocess.run([str(TSRCT), 'preview', '--project', str(OUT/f'{name}.tsrct'), '--time', '1', '--output', str(OUT/f'{name}.png')], check=True)
        print(theme, len(doc['composition']['layers']), 'native editable rectangles')


if __name__ == '__main__':
    main()
