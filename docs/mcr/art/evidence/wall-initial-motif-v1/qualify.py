"""Read-only closed-format checker for the frozen v1 producer input.

Uses independently parsed SVG rectangles and CRC-checked PNG RGB scanlines.
Negative probes mutate in-memory values only; they never mutate the pack.
"""
from pathlib import Path
import binascii
import collections
import hashlib
import json
import re
import struct
import xml.etree.ElementTree as ET
import zlib

ROOT=Path(__file__).resolve().parents[5]
PACK=ROOT/'frontend/public/wall/initial-motif-v1'
EVIDENCE=Path(__file__).resolve().parent


def require(test,message):
    if not test:raise ValueError(message)


def png_decode(data):
    require(data[:8]==b'\x89PNG\r\n\x1a\n','PNG signature')
    pos=8;chunks=[];compressed=b'';dimensions=None
    while pos<len(data):
        require(pos+12<=len(data),'truncated PNG')
        n=struct.unpack('>I',data[pos:pos+4])[0];tag=data[pos+4:pos+8];body=data[pos+8:pos+8+n]
        require(pos+12+n<=len(data),'truncated chunk')
        crc=struct.unpack('>I',data[pos+8+n:pos+12+n])[0]
        require(binascii.crc32(tag+body)&0xffffffff==crc,'PNG CRC')
        chunks.append(tag)
        if tag==b'IHDR':
            require(n==13,'IHDR size');w,h,depth,mode,compression,filtering,interlace=struct.unpack('>IIBBBBB',body)
            require((depth,mode,compression,filtering,interlace)==(8,2,0,0,0),'opaque RGB8 only')
            dimensions=(w,h)
        elif tag==b'IDAT':compressed+=body
        elif tag==b'IEND':require(n==0 and pos+12==len(data),'IEND/trailing bytes')
        else:raise ValueError('unexpected PNG chunk')
        pos+=n+12
    require(chunks==[b'IHDR',b'IDAT',b'IEND'],'closed PNG structure')
    w,h=dimensions;raw=zlib.decompress(compressed);stride=w*3
    require(len(raw)==h*(stride+1),'PNG inflated length')
    # The author deliberately stores filter 0, making every source pixel inspectable.
    require(all(raw[y*(stride+1)]==0 for y in range(h)),'unexpected PNG row filter')
    return w,h,b''.join(raw[y*(stride+1)+1:(y+1)*(stride+1)] for y in range(h))


def palette_decode(raw):
    values=json.loads(raw.decode('utf-8'))
    require(type(values) is list and len(values)==16,'16-entry palette')
    require(all(type(x) is str and re.fullmatch(r'#[0-9A-F]{6}',x) for x in values),'uppercase RGB palette')
    return values


def pixels_decode(raw):
    require(len(raw)==128*128,'16384 pixel bytes')
    require(max(raw)<=15,'palette index range')
    return raw


def svg_decode(raw,palette):
    require(b'<!' not in raw and b'<?' not in raw,'no XML directives/entities')
    root=ET.fromstring(raw);ns='{http://www.w3.org/2000/svg}'
    require(root.tag==ns+'svg','SVG namespace')
    require(root.attrib=={'width':'128','height':'128','viewBox':'0 0 128 128','shape-rendering':'crispEdges'},'closed SVG root')
    cells=[None]*(128*128);groups=0;rectangles=0
    for child in root:
        if child.tag in (ns+'title',ns+'desc'):
            require(not child.attrib and len(child)==0,'closed SVG description');continue
        require(child.tag==ns+'g','SVG element allowlist')
        require(child.attrib=={'id':f'palette-{groups}','fill':palette[groups]},'SVG palette order/fill')
        ink=groups;groups+=1
        for element in child:
            require(element.tag==ns+'rect' and len(element)==0,'rectangles only')
            require(set(element.attrib)=={'x','y','width','height'},'closed rectangle attributes')
            require(all(re.fullmatch(r'0|[1-9][0-9]*',v) for v in element.attrib.values()),'integer grid geometry')
            x,y,w,h=(int(element.attrib[n]) for n in ['x','y','width','height'])
            require(w>0 and h>0 and x+w<=128 and y+h<=128,'rectangle bounds')
            for yy in range(y,y+h):
                for xx in range(x,x+w):
                    offset=yy*128+xx
                    require(cells[offset] is None,'no hidden/overpainted pixels')
                    cells[offset]=ink
            rectangles+=1
    require(groups==16 and all(x is not None for x in cells),'complete opaque SVG coverage')
    return bytes(cells),rectangles


def manifest_decode(raw):
    m=json.loads(raw.decode('utf-8'))
    require(set(m)=={'format','asset_id','asset_revision','event_binding','width','height','layout','background_index','files','provenance'},'closed manifest keys')
    for key,value in {'format':'mcr-mural-initial-seed-art/1','asset_id':'wall-initial-motif-art-01','asset_revision':1,'event_binding':'OPERATOR_BOUND_AT_PROVISION','width':128,'height':128,'layout':'ROW_MAJOR_TOP_LEFT_U8','background_index':0}.items():
        require(type(m[key]) is type(value) and m[key]==value,'manifest '+key)
    require(set(m['files'])=={'source','pixels','palette','preview'},'closed file keys')
    for key,name in [('source','source.svg'),('pixels','pixels.bin'),('palette','palette.json'),('preview','preview.png')]:
        pin=m['files'][key];require(set(pin)=={'path','sha256'} and pin['path']==name,'closed file pin')
        require(re.fullmatch('[a-f0-9]{64}',pin['sha256']) is not None,'lowercase SHA256')
        require(hashlib.sha256((PACK/name).read_bytes()).hexdigest()==pin['sha256'],'file digest '+name)
    p=m['provenance'];require(set(p)=={'origin','reference_path','reference_sha256','rights_statement','qualification_evidence'},'closed provenance')
    require(p['origin']=='ORIGINAL_MCR_ARTWORK','origin')
    require(p['reference_path']=='docs/mcr/design/reference-target-2026-11-04.png','reference path')
    require(p['reference_sha256']=='d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617','reference hash')
    require(type(p['rights_statement']) is str and bool(p['rights_statement'].strip()) and p['rights_statement'].isascii(),'truthful English statement')
    require(p['qualification_evidence']=='README.md' and (PACK/'README.md').is_file(),'actual pack-relative evidence')
    return m


def main():
    require({p.name for p in PACK.iterdir()}=={'source.svg','pixels.bin','palette.json','preview.png','manifest.json','README.md'},'exact six pack files')
    palette=palette_decode((PACK/'palette.json').read_bytes());pixels=pixels_decode((PACK/'pixels.bin').read_bytes())
    source_pixels,rectangles=svg_decode((PACK/'source.svg').read_bytes(),palette)
    require(source_pixels==pixels,'every SVG cell equals binary index')
    rgb=b''.join(bytes.fromhex(palette[i][1:]) for i in pixels)
    width,height,preview=png_decode((PACK/'preview.png').read_bytes())
    require((width,height)==(128,128) and preview==rgb,'every preview RGB equals indexed palette reconstruction')
    m=manifest_decode((PACK/'manifest.json').read_bytes())
    pw,ph,proof=png_decode((EVIDENCE/'art-pack-390.png').read_bytes())
    require((pw,ph)==(390,566),'390px art-only proof')
    for y in range(256):
        for x in range(256):
            a=((137+y)*390+67+x)*3;b=((y//2)*128+x//2)*3
            require(proof[a:a+3]==rgb[b:b+3],'proof nearest-neighbor pixel identity')
    # Concrete rejection probes exercise producer-critical failure boundaries.
    probes=[('short binary',lambda:pixels_decode(pixels[:-1])),
            ('out-of-range index',lambda:pixels_decode(bytes([16])+pixels[1:])),
            ('lowercase palette',lambda:palette_decode(json.dumps([palette[0],palette[1].lower()]+palette[2:]).encode())),
            ('reordered palette/source mismatch',lambda:svg_decode((PACK/'source.svg').read_bytes(),list(reversed(palette)))),
            ('extra manifest key',lambda:manifest_decode(json.dumps({**m,'operator':'invented'}).encode())),
            ('corrupt preview CRC',lambda:png_decode((PACK/'preview.png').read_bytes()[:-13]+b'x'*13)),
            ('external SVG resource',lambda:svg_decode((PACK/'source.svg').read_bytes().replace(b'<title>',b'<image href="https://invalid.example/"/><title>'),palette))]
    rejected=[]
    for name,call in probes:
        try:call()
        except (ValueError,ET.ParseError):rejected.append(name)
        else:raise ValueError('negative probe accepted: '+name)
    pins={p.name:{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in sorted(PACK.iterdir())}
    result={'result':'SOURCE_FORMAT_PASS_VISUAL_INSPECTION_SEPARATE','contract_revision':'1607fbf8','manifest_sha256':pins['manifest.json']['sha256'],'files':pins,
            'opaque_native_dimensions':[128,128],'proof_dimensions':[pw,ph],'editable_svg_rectangles':rectangles,'svg_binary_index_identity':True,'preview_rgb_identity':True,'proof_2x_rgb_identity':True,
            'background_index':0,'background_color':palette[0],'index_counts':dict(sorted(collections.Counter(pixels).items())),'negative_rejection_probes':rejected,
            'limits':['ART-PACK ONLY; no authoritative or live Wall capture.','No event/operator/rights approval or server provisioning is asserted.','Visual inspection is recorded separately after viewing actual preview and proof.']}
    (EVIDENCE/'qualification.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'result':result['result'],'manifest_sha256':result['manifest_sha256'],'negative_probes':len(rejected)}))


if __name__=='__main__':main()
