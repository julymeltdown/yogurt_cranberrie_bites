"""Original After-show Bloom recipe. Run only inside an admitted source slot.

All coordinates are on the 128-cell grid. No image input is traced or sampled.
The SVG contains editable integer rectangles, not an embedded raster.
"""
from pathlib import Path
import binascii
import hashlib
import json
import struct
import zlib

ROOT = Path(__file__).resolve().parents[5]
PACK = ROOT / 'frontend/public/wall/initial-motif-v1'
OUT = Path(__file__).resolve().parent
PALETTE = ['#171617', '#F1E8D8', '#C72D3B', '#30282B',
           '#D8C9B5', '#821F30', '#FFF7E7', '#AD9886',
           '#4A2B34', '#E3555A', '#685058', '#D77769',
           '#827873', '#D5AD95', '#242024', '#E8D9BE']
COLORS = [bytes.fromhex(s[1:]) for s in PALETTE]
W = H = 128
grid = bytearray(W * H)


def rect(x, y, w, h, ink):
    assert 0 <= x < x+w <= W and 0 <= y < y+h <= H
    for yy in range(y, y+h):
        grid[yy*W+x:yy*W+x+w] = bytes([ink])*w


def polygon(points, ink):
    # Pixel-center even/odd fill; no fractional coverage or antialiasing.
    for y in range(max(0, min(p[1] for p in points)), min(H, max(p[1] for p in points))):
        crossings = []
        for (ax, ay), (bx, by) in zip(points, points[1:]+points[:1]):
            if (ay <= y+.5 < by) or (by <= y+.5 < ay):
                crossings.append(ax+(y+.5-ay)*(bx-ax)/(by-ay))
        crossings.sort()
        for left, right in zip(crossings[::2], crossings[1::2]):
            for x in range(max(0, int(left)), min(W, int(right)+1)):
                if left <= x+.5 < right:
                    grid[y*W+x] = ink


def star(x, y, ink):
    rect(x+2, y, 2, 2, ink); rect(x, y+2, 6, 2, ink); rect(x+2, y+4, 2, 2, ink)


def compose():
    # Broken crimson corner rhythms; a broad quiet dark field remains editable.
    polygon([(0,8),(8,8),(8,20),(20,20),(20,28),(14,28),(14,24),(4,24),(4,16),(0,16)],5)
    polygon([(0,8),(6,8),(6,20),(18,20),(18,25),(12,25),(12,22),(2,22),(2,14),(0,14)],2)
    polygon([(107,0),(117,0),(117,12),(110,12),(110,20),(103,20),(103,28),(97,28),(97,22),(101,22),(101,14),(107,14)],2)
    rect(111,2,3,9,9);rect(98,24,3,4,5)
    polygon([(0,105),(9,105),(9,98),(16,98),(16,90),(22,90),(22,98),(18,98),(18,106),(10,106),(10,114),(0,114)],2)
    rect(2,108,5,3,9)
    polygon([(103,111),(109,111),(109,102),(116,102),(116,94),(123,94),(123,86),(128,86),(128,98),(120,98),(120,107),(113,107),(113,119),(103,119)],5)
    polygon([(106,110),(112,110),(112,101),(119,101),(119,93),(126,93),(126,89),(128,89),(128,96),(122,96),(122,105),(115,105),(115,116),(106,116)],2)
    # The stem bends, avoiding an emblem-like closed or mirrored composition.
    polygon([(61,73),(67,73),(67,87),(61,93),(61,103),(56,111),(51,111),(56,101),(56,91),(61,85)],4)
    polygon([(61,74),(64,74),(64,86),(59,92),(59,103),(55,109),(53,109),(57,102),(57,91),(61,85)],1)
    polygon([(58,97),(48,95),(41,88),(40,82),(47,84),(56,90),(60,93)],2)
    polygon([(59,98),(49,96),(42,89),(43,87),(50,91),(57,93)],5)
    polygon([(62,91),(68,82),(77,79),(83,79),(80,87),(73,93),(64,95)],2)
    polygon([(65,91),(71,85),(78,82),(77,86),(71,91)],9)
    # Eight separately editable stepped petals around a generous dark aperture.
    petals = [
        [(57,38),(53,29),(53,23),(58,18),(68,18),(73,23),(73,30),(68,39)],
        [(73,41),(75,32),(84,25),(92,25),(97,30),(97,37),(89,44),(78,48)],
        [(79,48),(91,45),(101,49),(105,55),(105,62),(99,67),(89,67),(78,61)],
        [(76,64),(87,67),(94,75),(94,82),(89,87),(82,87),(73,79),(70,69)],
        [(59,69),(68,69),(73,80),(72,88),(67,93),(59,93),(54,88),(54,80)],
        [(49,64),(57,69),(53,79),(44,87),(37,87),(32,82),(32,75),(39,68)],
        [(48,48),(48,61),(37,67),(27,67),(21,62),(21,55),(25,49),(35,45)],
        [(49,41),(48,48),(37,44),(29,37),(29,30),(34,25),(42,25),(51,32)],
    ]
    for points in petals: polygon(points,1)
    # Angular fold shadows stop short of the tips, keeping the flower legible.
    polygon([(58,22),(61,22),(61,34),(58,31)],6)
    polygon([(68,23),(71,25),(71,30),(67,36),(65,35)],4)
    polygon([(89,29),(93,31),(93,35),(86,41),(81,42)],4)
    polygon([(94,50),(100,53),(101,58),(98,60),(96,54)],6)
    polygon([(98,63),(90,64),(82,60),(84,58),(91,61)],4)
    polygon([(88,76),(91,79),(88,84),(84,84),(78,78)],4)
    polygon([(69,83),(69,87),(65,90),(60,90),(58,87),(59,85),(64,87)],4)
    polygon([(39,79),(41,82),(45,80),(49,74),(46,75)],6)
    polygon([(27,55),(29,51),(35,49),(40,51),(33,52),(30,56)],6)
    polygon([(34,31),(38,29),(45,34),(46,38),(42,37)],6)
    # Open, stepped dark center and crimson seed. No letters, faces or logos.
    polygon([(59,36),(68,36),(68,42),(76,42),(76,49),(81,49),(81,61),(75,61),(75,68),(68,68),(68,74),(58,74),(58,68),(51,68),(51,61),(46,61),(46,49),(51,49),(51,42),(59,42)],0)
    polygon([(60,44),(66,44),(66,48),(71,48),(75,53),(75,59),(69,65),(64,69),(57,64),(52,59),(52,52),(57,47),(60,47)],5)
    polygon([(59,46),(64,46),(64,49),(69,49),(73,53),(73,58),(68,63),(63,66),(57,61),(54,57),(54,53)],2)
    rect(58,50,4,3,9);rect(62,53,3,3,9);rect(57,53,2,2,11)
    # Sparse original sparkle marks and detached chips, not a decorative rail.
    star(15,39,1);star(108,47,1);star(27,110,4);star(83,104,1)
    rect(30,12,3,3,4);rect(37,14,2,2,2);rect(84,12,3,3,2)
    rect(108,73,3,3,2);rect(18,77,2,3,4);rect(93,96,2,2,4)
    rect(77,116,3,2,5);rect(43,115,2,2,2)


def png_bytes(width, height, rgb):
    def chunk(tag, body):
        return struct.pack('>I',len(body))+tag+body+struct.pack('>I',binascii.crc32(tag+body)&0xffffffff)
    raw=b''.join(b'\0'+rgb[y*width*3:(y+1)*width*3] for y in range(height))
    return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(raw,9))+chunk(b'IEND',b'')


FONT = {
'A':'01110/10001/10001/11111/10001/10001/10001','B':'11110/10001/10001/11110/10001/10001/11110',
'C':'01111/10000/10000/10000/10000/10000/01111','D':'11110/10001/10001/10001/10001/10001/11110',
'E':'11111/10000/10000/11110/10000/10000/11111','F':'11111/10000/10000/11110/10000/10000/10000',
'G':'01111/10000/10000/10111/10001/10001/01111','H':'10001/10001/10001/11111/10001/10001/10001',
'I':'111/010/010/010/010/010/111','K':'10001/10010/10100/11000/10100/10010/10001',
'L':'10000/10000/10000/10000/10000/10000/11111','M':'10001/11011/10101/10101/10001/10001/10001',
'N':'10001/11001/10101/10011/10001/10001/10001','O':'01110/10001/10001/10001/10001/10001/01110',
'P':'11110/10001/10001/11110/10000/10000/10000','R':'11110/10001/10001/11110/10100/10010/10001',
'S':'01111/10000/10000/01110/00001/00001/11110','T':'11111/00100/00100/00100/00100/00100/00100',
'U':'10001/10001/10001/10001/10001/10001/01110','V':'10001/10001/10001/10001/10001/01010/00100',
'W':'10001/10001/10001/10101/10101/10101/01010','X':'10001/10001/01010/00100/01010/10001/10001',
'Y':'10001/10001/01010/00100/00100/00100/00100',
'0':'01110/10001/10011/10101/11001/10001/01110','1':'010/110/010/010/010/010/111',
'2':'01110/10001/00001/00010/00100/01000/11111','3':'11110/00001/00001/01110/00001/00001/11110',
'6':'01110/10000/10000/11110/10001/10001/01110','8':'01110/10001/10001/01110/10001/10001/01110',
'9':'01110/10001/10001/01111/00001/00001/01110','-':'000/000/000/111/000/000/000',
'/':'00001/00001/00010/00100/01000/10000/10000',' ':'000/000/000/000/000/000/000'}


def proof(rgb):
    width,height=390,566
    canvas=bytearray(bytes.fromhex('F1E8D8')*(width*height))
    def box(x,y,w,h,color):
        c=bytes.fromhex(color)
        for yy in range(y,y+h):canvas[(yy*width+x)*3:(yy*width+x+w)*3]=c*w
    def text(x,y,string,scale=1,color='171617'):
        for ch in string:
            glyph=FONT[ch].split('/')
            for yy,row in enumerate(glyph):
                for xx,value in enumerate(row):
                    if value=='1':box(x+xx*scale,y+yy*scale,scale,scale,color)
            x+=(len(glyph[0])+1)*scale
    text(24,22,'ART PACK ONLY / 390 PX',2,'821F30')
    text(24,48,'AFTER-SHOW',3)
    text(24,78,'BLOOM',3)
    box(24,115,342,1,'AD9886')
    for y in range(128):
        for x in range(128):box(67+x*2,137+y*2,2,2,rgb[(y*128+x)*3:(y*128+x+1)*3].hex())
    text(67,409,'128 X 128 / 16 COLORS',2)
    for i,color in enumerate(PALETTE):box(51+i*18,438,16,20,color[1:])
    text(24,485,'ORIGINAL EDITABLE PIXEL ART',2)
    text(24,506,'NOT A LIVE WALL',2,'821F30')
    text(24,540,'NO EVENT OR OPERATOR BINDING',2)
    return png_bytes(width,height,canvas)


def main():
    PACK.mkdir(parents=True,exist_ok=True)
    compose()
    source=['<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128" shape-rendering="crispEdges">',
            '<title>After-show Bloom</title>',
            '<desc>Original stepped cream flower, crimson center and leaves, sparse concert-night sparks. No character imagery.</desc>']
    for ink,color in enumerate(PALETTE):
        source.append(f'<g id="palette-{ink}" fill="{color}">')
        for y in range(H):
            x=0
            while x<W:
                if grid[y*W+x]!=ink:x+=1;continue
                start=x
                while x<W and grid[y*W+x]==ink:x+=1
                source.append(f'<rect x="{start}" y="{y}" width="{x-start}" height="1"/>')
        source.append('</g>')
    source.append('</svg>')
    (PACK/'source.svg').write_text('\n'.join(source)+'\n',encoding='utf-8')
    (PACK/'pixels.bin').write_bytes(grid)
    (PACK/'palette.json').write_text(json.dumps(PALETTE,indent=2)+'\n',encoding='utf-8')
    rgb=b''.join(COLORS[i] for i in grid)
    (PACK/'preview.png').write_bytes(png_bytes(W,H,rgb))
    (OUT/'art-pack-390.png').write_bytes(proof(rgb))
    manifest={'format':'mcr-mural-initial-seed-art/1','asset_id':'wall-initial-motif-art-01','asset_revision':1,
              'event_binding':'OPERATOR_BOUND_AT_PROVISION','width':128,'height':128,'layout':'ROW_MAJOR_TOP_LEFT_U8','background_index':0,
              'files':{k:{'path':name,'sha256':hashlib.sha256((PACK/name).read_bytes()).hexdigest()} for k,name in [('source','source.svg'),('pixels','pixels.bin'),('palette','palette.json'),('preview','preview.png')]},
              'provenance':{'origin':'ORIGINAL_MCR_ARTWORK','reference_path':'docs/mcr/design/reference-target-2026-11-04.png',
                            'reference_sha256':'d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617',
                            'rights_statement':'Original geometric pixel artwork authored for this MCR task. The supplied reference informed contrast and pixel rhythm only; no reference pixels, lettering, logos, figures or character assets were copied. No third-party approval, exclusive rights warranty, operator identity or provisioning authority is claimed.',
                            'qualification_evidence':'README.md'}}
    (PACK/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'result':'AUTHORED_NOT_YET_QUALIFIED','manifest_sha256':hashlib.sha256((PACK/'manifest.json').read_bytes()).hexdigest(),'used_indices':sorted(set(grid))}))


if __name__=='__main__':main()
