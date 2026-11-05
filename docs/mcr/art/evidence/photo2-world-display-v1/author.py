"""Original editable vector scenery. Execute only under the finite asset grant."""
from copy import deepcopy
import hashlib
import html
import json
import math
from pathlib import Path
import platform
import random
import subprocess
import sys

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
PUBLIC=ROOT/'frontend/public/world/reference-scene-v2/photo2-world-display-v1/parade'
CANONICAL='33491ccf9b0ae3835ad691072d8479b299f543d8'
FOLDER='docs/mcr/evidence/pi08-finish-01/photo2-world-contract/'
CONTRACT_SHA='3fce374b7f60d46e8561559c42470a3343708f9eb3236a1d01050c4f0aa3a12d'
REFERENCE_SHA='9e7f17a666e1ecfc738a14b75bb3a166235a05c401f206aa24f26cf7463cbc27'
REFERENCE=Path('/private/tmp/codex-remote-attachments/01a19e5c-e9c6-7973-a53b-a6d19141620f/9FC49E03-6C0E-4776-A954-2CA5C12CB495/2-Photo-2.jpg')
NATIVE_PATH='frontend/src/world/reference-scene/spatial-v3/source-inputs.json'
INK='#282725'; DARK='#3c3935'; PAPER='#e4dcd2'

def sha(b):return hashlib.sha256(b).hexdigest()
def git_bytes(revision,path):return subprocess.check_output(['git','show',revision+':'+path],cwd=ROOT)
def inputs():
 raw=git_bytes(CANONICAL,FOLDER+'producer-contract-v2.json');assert sha(raw)==CONTRACT_SHA
 contract=json.loads(raw);n=contract['nativeSource'];raw_native=git_bytes(n['commit'],n['path']);assert sha(raw_native)==n['sha256']
 assert sha(REFERENCE.read_bytes())==REFERENCE_SHA
 return contract,json.loads(raw_native)
def write_json(path,value):path.write_text(json.dumps(value,indent=2)+'\n')
def public_path(path):return str(path.relative_to(ROOT/'frontend/public'))
def file_pin(path):
 b=path.read_bytes();return dict(path=str(path.relative_to(ROOT)),bytes=len(b),sha256=sha(b))

class Canvas:
 def __init__(self,w,h):self.width=w;self.height=h;self.items=[]
 def poly(self,points,fill,stroke=None,width=1,tag=None):self.items.append(dict(type='polygon',points=[[float(x),float(y)] for x,y in points],fill=fill,stroke=stroke,strokeWidth=width,tag=tag))
 def rect(self,x,y,w,h,fill,stroke=None,width=1,tag=None):self.poly([(x,y),(x+w-1,y),(x+w-1,y+h-1),(x,y+h-1)],fill,stroke,width,tag)
 def line(self,points,fill,width=1):self.items.append(dict(type='line',points=[[float(x),float(y)] for x,y in points],fill=fill,strokeWidth=width))
 def render(self,origin=(0,0),background=(0,0,0,0)):
  from PIL import Image,ImageDraw
  image=Image.new('RGBA',(self.width,self.height),background);d=ImageDraw.Draw(image)
  for item in self.items:
   points=[(round(x-origin[0]),round(y-origin[1])) for x,y in item['points']]
   if item['type']=='polygon':
    d.polygon(points,fill=item['fill'])
    if item.get('stroke'):d.line(points+[points[0]],fill=item['stroke'],width=item['strokeWidth'])
   else:d.line(points,fill=item['fill'],width=item['strokeWidth'])
  return image
 def svg(self,origin=(0,0)):
  parts=[f'<svg xmlns="http://www.w3.org/2000/svg" width="{self.width}" height="{self.height}" viewBox="{origin[0]} {origin[1]} {self.width} {self.height}" shape-rendering="crispEdges">', '<title>Original Photo 2 PARADE World paving and frozen projected support</title>']
  for item in self.items:
   points=' '.join(f'{x:.6f},{y:.6f}' for x,y in item['points']);tag=item.get('tag');identity=f' data-source="{html.escape(tag)}"' if tag else ''
   if item['type']=='polygon':parts.append(f'<polygon points="{points}" fill="{item["fill"]}" stroke="{item.get("stroke") or "none"}" stroke-width="{item["strokeWidth"]}"{identity}/>')
   else:parts.append(f'<polyline points="{points}" fill="none" stroke="{item["fill"]}" stroke-width="{item["strokeWidth"]}"/>')
  return '\n'.join(parts+['</svg>'])+'\n'

FONT={
'M':['10001','11011','10101','10101','10001','10001','10001'],
'C':['01111','11000','10000','10000','10000','11000','01111'],
'R':['11110','10011','10001','11110','10100','10010','10001'],
'E':['11111','10000','10000','11110','10000','10000','11111'],
'T':['11111','00100','00100','00100','00100','00100','00100'],
'W':['10001','10001','10001','10101','10101','11011','10001'],
'A':['01110','10001','10001','11111','10001','10001','10001'],
'L':['10000','10000','10000','10000','10000','10000','11111'],
'P':['11110','10001','10001','11110','10000','10000','10000'],
'K':['10001','10010','10100','11000','10100','10010','10001'],
' ':['00000']*7}
def lettering(c,text,x,y,scale=2,color=INK,slant=0):
 for index,character in enumerate(text):
  for row,bits in enumerate(FONT[character]):
   for col,value in enumerate(bits):
    if value=='1':
     xx=x+(index*6+col)*scale;yy=y+row*scale+slant*(xx-x)
     c.rect(xx,yy,scale,scale,color)

def stage(c):
 # The diagonal front, curtain and exposed box truss are authored polygons.
 c.poly([(8,194),(171,146),(205,160),(42,210)],'#5b5550',INK,2)
 c.poly([(9,191),(171,143),(171,165),(10,211)],'#3c3937',INK,2)
 c.poly([(15,181),(160,138),(171,145),(22,191)],'#a7a19a',INK,2)
 for x in range(20,158,17):
  y=190-(x-20)*.285;c.line([(x,y),(x,y+17)],'#b5ada3',2)
 c.poly([(22,62),(148,27),(148,139),(22,176)],'#1b1b1c',INK,2)
 c.poly([(24,63),(42,58),(42,166),(25,172)],'#812b36',INK,1)
 c.poly([(26,64),(33,62),(33,168),(27,170)],'#b23b48')
 c.poly([(35,61),(40,60),(40,164),(35,166)],'#64212d')
 c.poly([(129,33),(146,29),(146,139),(128,144)],'#902d3b',INK,1)
 c.poly([(134,33),(140,31),(140,139),(134,141)],'#c14350')
 lettering(c,'MCR',51,94,4,'#ddd9cd',-.22)
 for x,y in [(39,69),(62,62),(85,55),(109,48),(133,41)]:
  c.rect(x-5,y-6,10,10,'#541f28',INK);c.rect(x-3,y-4,6,6,'#bd4852');c.rect(x-1,y-3,3,3,'#e48b83')
 # Silver uprights and diagonal bracing, with a readable open roof grid.
 for x,y,base in [(6,47,183),(153,7,147),(173,26,150)]:
  c.line([(x,y),(x,base)],'#242625',6);c.line([(x+2,y),(x+2,base)],'#c5c0b7',3)
  for yy in range(int(y),int(base)-15,17):c.line([(x-4,yy),(x+6,yy+17),(x-4,yy+17)],'#b4b1a8',1)
 for a,b in [((6,47),(153,7)),((27,68),(174,27)),((6,47),(27,68)),((153,7),(174,27))]:
  c.line([a,b],INK,5);c.line([a,b],'#c9c5bb',2)
 for i in range(7):
  x=7+i*24;y=47-i*6.5
  c.line([(x,y),(x+21,y+20),(x+24,y-6.5)],'#c2beb4',2)
  if i<6:c.line([(x+21,y+20),(x+45,y+13.5)],'#a9a79e',1)
 for x,y,w,h in [(1,122,18,48),(1,175,23,35),(177,61,24,45),(177,109,30,53)]:
  c.rect(x,y,w,h,'#282827',INK,2);c.rect(x+4,y+4,w-8,h-8,'#383735');c.rect(x+6,y+7,3,3,'#999388')
  for yy in range(int(y)+15,int(y+h)-4,6):c.line([(x+5,yy),(x+w-5,yy)],'#1e2020')

def vendor(c):
 c.poly([(7,66),(76,43),(101,59),(101,128),(33,141),(7,125)],'#a88875',INK,2)
 c.poly([(10,65),(74,45),(74,103),(10,121)],'#282827',INK,2)
 c.poly([(0,43),(67,2),(108,28),(44,68)],'#e7dad4',INK,2)
 c.poly([(0,43),(44,64),(108,28),(108,43),(44,80),(0,57)],'#c4b5b0',INK,2)
 c.poly([(3,42),(66,6),(81,25),(44,63)],'#f2e8df')
 c.line([(67,6),(44,64)],'#c4b6af',2)
 for x,y in [(5,57),(43,75),(104,43)]:c.line([(x,y),(x,132 if x<60 else 122)],INK,3)
 for x,y in [(29,84),(59,75)]:
  c.poly([(x-5,y),(x+1,y-5),(x+7,y-5),(x+13,y),(x+9,y+7),(x+7,y+4),(x+7,y+20),(x+1,y+20),(x+1,y+4),(x-2,y+8)],'#dad7cd',INK)
 c.poly([(9,113),(77,94),(102,108),(34,130)],'#d5b7a2',INK,2)
 for x in range(12,99,9):
  y=124-(x-12)*.22;c.line([(x,y),(x,y+12)],'#67564c',2)
 c.line([(12,135),(34,140),(101,124)],'#625047',2)

def bench(c):
 c.poly([(1,13),(52,1),(56,27),(5,40)],'#80645a',INK,2)
 for y in [14,22,30]:c.line([(5,y),(52,y-12)],'#c1a18b',4)
 c.poly([(0,40),(54,26),(71,35),(17,51)],'#ad8a73',INK,2)
 c.line([(4,42),(55,29)],'#d1b39b',3);c.line([(14,48),(65,35)],'#7c6154',2)
 for x,y in [(8,42),(57,34),(21,49),(66,39)]:c.rect(x,y,4,63-y,INK)

def lamp(c):
 w,h=c.width,c.height;mid=w//2
 c.poly([(mid-5,h-8),(mid+5,h-8),(mid+7,h-2),(mid-7,h-2)],'#55534d',INK)
 c.rect(mid-2,23,5,h-29,'#68655e',INK)
 c.line([(mid-1,27),(mid-1,h-10)],'#a19c8e')
 c.poly([(1,17),(mid,8),(w-2,17),(w-4,24),(4,24)],'#34342f',INK)
 c.poly([(4,23),(w-5,23),(w-6,36),(5,36)],'#d3c7a7',INK)
 c.line([(mid,23),(mid,36)],'#69665b')
 c.poly([(2,18),(mid,3),(w-3,18)],'#62645d',INK);c.rect(mid-1,0,3,4,INK)

def old_tree_source03(c,seed):
 rng=random.Random(seed);w,h=c.width,c.height;cx=w*.51;trunk=max(6,round(w*.095));canopy=h*.65
 c.poly([(cx-trunk*.6,canopy*.6),(cx+trunk*.5,canopy*.6),(cx+trunk*.7,h-4),(cx-trunk*.6,h-4)],'#625145',INK,2)
 c.line([(cx-1,canopy*.72),(cx-2,h-8)],'#a4896a',2)
 for direction in [-1,1]:c.line([(cx,h*.72),(cx+direction*w*.23,h*.44)],INK,max(3,round(w*.04)))
 # Block-cluster crown: intentionally stepped contours, no smooth circles.
 clusters=[]
 for row in range(5):
  for col in range(7):
   xx=w*(.08+col*.13)+rng.randint(-4,4);yy=canopy*(.13+row*.17)+rng.randint(-3,3)
   if ((xx-w*.5)/(w*.51))**2+((yy-canopy*.5)/(canopy*.52))**2<1.05:clusters.append((xx,yy))
 for xx,yy in clusters:
  sx=w*.19;sy=canopy*.23;c.poly([(xx-sx*.4,yy-sy*.5),(xx+sx*.3,yy-sy*.5),(xx+sx*.3,yy-sy*.3),(xx+sx*.6,yy-sy*.3),(xx+sx*.6,yy+sy*.3),(xx+sx*.3,yy+sy*.3),(xx+sx*.3,yy+sy*.5),(xx-sx*.5,yy+sy*.5),(xx-sx*.5,yy+sy*.2),(xx-sx*.65,yy+sy*.2),(xx-sx*.65,yy-sy*.2),(xx-sx*.4,yy-sy*.2)],'#34392b')
 colors=['#535d37','#697443','#858a50','#999c62','#747d48']
 for index,(xx,yy) in enumerate(clusters):
  c.poly([(xx-7,yy-7),(xx+4,yy-7),(xx+4,yy-4),(xx+9,yy-4),(xx+9,yy+5),(xx+5,yy+5),(xx+5,yy+9),(xx-7,yy+9),(xx-7,yy+4),(xx-10,yy+4),(xx-10,yy-2),(xx-7,yy-2)],colors[index%len(colors)])
  c.rect(xx-5,yy-5,4,3,'#a6a76c' if index%3==0 else '#8d955a')
 c.poly([(cx-10,h-7),(cx+9,h-7),(cx+13,h-2),(cx-13,h-2)],'#777364',INK)

def tree(c,seed):
 rng=random.Random(seed);w,h=c.width,c.height;cx=w*.51
 c.poly([(cx-3,h*.43),(cx+3,h*.43),(cx+5,h-4),(cx-5,h-4)],'#625145',INK,2)
 c.line([(cx-1,h*.51),(cx-1,h-8)],'#a4896a',2)
 colors=['#525b3d','#69734a','#818454','#93915c','#737c4d']
 for i,(u,v) in enumerate([(.30,.24),(.51,.17),(.70,.26),(.24,.42),(.47,.36),(.72,.43),(.38,.54),(.61,.55)]):
  xx=w*u;yy=h*v;rx=w*.23;ry=h*.16
  contour=[(-.8,-.5),(-.5,-.5),(-.5,-.85),(.1,-.85),(.1,-1),(.5,-.8),(.5,-.6),(.85,-.6),(.85,-.2),(1,-.2),(1,.3),(.75,.3),(.75,.65),(.3,.65),(.3,.85),(-.3,.85),(-.3,.65),(-.7,.65),(-.7,.3),(-.95,.3),(-.95,-.2),(-.8,-.2)]
  c.poly([(xx+a*rx,yy+b*ry) for a,b in contour],colors[i%5],'#39402d',1)
  for _ in range(12):
   a=rng.uniform(-.65,.65);b=rng.uniform(-.6,.5)
   if a*a+b*b<.55:c.rect(xx+a*rx,yy+b*ry,max(1,w*.025),max(1,h*.018),'#9a9b66' if i%2 else '#626f43')
 c.poly([(cx-10,h-7),(cx+9,h-7),(cx+13,h-2),(cx-13,h-2)],'#777364',INK)

def planter(c):
 w,h=c.width,c.height
 c.poly([(1,h*.60),(w*.55,h*.48),(w-2,h*.61),(w-2,h-6),(w*.43,h-1),(2,h-8)],'#9a9482',INK)
 c.poly([(3,h*.59),(w*.55,h*.48),(w-3,h*.60),(w*.43,h*.72)],'#b5b19c',INK)
 for x in range(4,w-3,5):c.rect(x,h*.50,4,5,'#707647')
 c.rect(w*.40,3,w*.26,h*.48,'#b1a38b',INK);c.rect(w*.44,6,w*.18,h*.29,'#4b4a3a')
 c.line([(w*.44,11),(w*.58,11)],'#ddd1ae')

def pavilion(c,variant):
 w,h=c.width,c.height
 c.poly([(5,h*.39),(w*.57,h*.20),(w-5,h*.40),(w*.42,h*.61)],'#e2d8c9',INK,2)
 c.poly([(8,h*.42),(w*.43,h*.59),(w*.43,h-5),(8,h*.80)],'#b3a18c',INK,2)
 c.poly([(w*.43,h*.59),(w-8,h*.42),(w-8,h*.79),(w*.43,h-5)],'#827d6c',INK,2)
 c.poly([(0,h*.38),(w*.54,3),(w-1,h*.35),(w*.43,h*.59)],'#d6c9b7',INK,2)
 c.poly([(w*.54,3),(w-1,h*.35),(w*.43,h*.59)],'#a99a84',INK)
 for xx in [w*.19,w*.65,w*.82]:c.rect(xx,h*.58,w*.09,h*.20,'#46453c',INK)
 c.rect(w*.40,h*.72,w*.14,h*.26,'#4c4b43',INK)
 if variant%2:c.line([(w*.54,6),(w*.43,h*.58)],'#e6dbc7',2)

def board(c,name):
 w,h=c.width,c.height
 c.poly([(8,9),(w-10,4),(w-2,h-1),(w-10,h-1),(w-15,17),(15,21),(9,h-2),(1,h-2)],'#80665a',INK,2)
 c.poly([(11,3),(w-12,1),(w-8,h-17),(7,h-12)],'#b89e88',INK,2)
 c.poly([(15,8),(w-17,6),(w-13,h-22),(12,h-18)],'#e7dfd4',INK,1)
 lettering(c,name,15 if name=='MEET' else 17,14,2,INK)
 if name=='MEET':
  # A board pictogram, not baked World actors or character artwork.
  for xx,yy in [(26,43),(40,40),(54,43)]:
   c.rect(xx,yy,7,8,INK);c.poly([(xx-2,yy+10),(xx+8,yy+10),(xx+9,yy+22),(xx-3,yy+22)],INK)
 else:
  c.poly([(34,54),(53,33),(60,40),(41,61),(30,64)],'#ddd6c9',INK,2)
  c.line([(39,55),(55,38)],'#59554c',2);c.poly([(30,64),(34,54),(41,61)],INK)
 c.line([(8,h-13),(w-8,h-18)],'#65534b',2)

def frame_canvas(frame,index):
 w=frame.get('paintWidth',frame.get('width'));h=frame.get('paintHeight',frame.get('height'));c=Canvas(w,h);name=frame['id']
 if name=='main_stage':stage(c)
 elif name=='north_white_food':vendor(c)
 elif name=='west_white_merch':bench(c)
 elif name=='culture_park_plaque':planter(c)
 elif name=='cross_banner_east':lamp(c)
 elif name=='rooted_tree_06':
  # Trimmed foreground planting stays below the WALL board; frozen foot/frame remain exact.
  c.poly([(2,h-12),(w*.18,h-19),(w*.42,h-16),(w*.65,h-20),(w-3,h-12),(w-2,h-5),(3,h-5)],'#666d47',INK)
  c.poly([(4,h-7),(w-3,h-7),(w-5,h-1),(7,h-1)],'#989381',INK)
 elif name.startswith('rooted_tree_'):tree(c,20261105+index*31)
 elif name.startswith('photo2-shortcut-'):board(c,'MEET' if name.endswith('meet') else 'WALL')
 else:pavilion(c,index)
 return c

def cross(a,b):return a[0]*b[1]-a[1]*b[0]
def clip(poly,axis,bound,positive):
 out=[]
 if not poly:return out
 for a,b in zip(poly,poly[1:]+poly[:1]):
  inside_a=(a[axis]-bound)*positive>=-1e-9;inside_b=(b[axis]-bound)*positive>=-1e-9
  if inside_a:out.append(a)
  if inside_a!=inside_b:
   t=(bound-a[axis])/(b[axis]-a[axis]);out.append((a[0]+t*(b[0]-a[0]),a[1]+t*(b[1]-a[1])))
 return out
def triangle_point(point,native,display):
 a,b,c=native;v=(point[0]-a[0],point[1]-a[1]);ab=(b[0]-a[0],b[1]-a[1]);ac=(c[0]-a[0],c[1]-a[1]);den=cross(ab,ac)
 u=cross(v,ac)/den;t=cross(ab,v)/den
 return [display[0][0]+u*(display[1][0]-display[0][0])+t*(display[2][0]-display[0][0]),display[0][1]+u*(display[1][1]-display[0][1])+t*(display[2][1]-display[0][1])]
def support_pieces(contract,native):
 mesh=contract['mesh'];columns=mesh['columns'];rows=mesh['rows'];vertices=mesh['display'];pieces=[];cells=[]
 geometry=native['geometry'];grid=geometry['collision_grid'];tw=geometry.get('tile_width_px',16)
 assert tw==16 and len(grid)==81 and len(grid[0])==48
 for cy,row in enumerate(grid):
  for cx,blocked in enumerate(row):
   if not blocked:continue
   index=cy*48+cx;cells.append(index);x0=cx*16;y0=cy*16;x1=x0+16;y1=y0+16
   for ry in range(len(rows)-1):
    if rows[ry+1]<=y0 or rows[ry]>=y1:continue
    for rx in range(len(columns)-1):
     if columns[rx+1]<=x0 or columns[rx]>=x1:continue
     ids=[ry*len(columns)+rx,ry*len(columns)+rx+1,(ry+1)*len(columns)+rx,(ry+1)*len(columns)+rx+1]
     native_corners=[(columns[rx],rows[ry]),(columns[rx+1],rows[ry]),(columns[rx],rows[ry+1]),(columns[rx+1],rows[ry+1])]
     for choice,t in enumerate([(0,1,3),(0,3,2)]):
      triangle=[native_corners[k] for k in t];p=triangle
      for axis,bound,direction in [(0,x0,1),(0,x1,-1),(1,y0,1),(1,y1,-1)]:p=clip(p,axis,bound,direction)
      if len(p)<3:continue
      area=abs(sum(a[0]*b[1]-b[0]*a[1] for a,b in zip(p,p[1:]+p[:1])))/2
      if area<1e-8:continue
      destination=[[vertices[ids[k]]['x'],vertices[ids[k]]['y']] for k in t]
      q=[triangle_point(v,triangle,destination) for v in p]
      pieces.append(dict(cell=index,triangle=(ry*(len(columns)-1)+rx)*2+choice,nativePolygon=p,displayPolygon=q,nativeArea=area))
 return cells,pieces

def ground_canvas(contract,native):
 c=Canvas(499,1200);c.rect(-112,-20,499,1200,'#c1b3b1')
 palette=['#c7b8b6','#beb0af','#c3b4b2','#bbaead','#c4b7b4'];rng=random.Random(23104)
 for row,y in enumerate(range(-36,1208,7)):
  for column,x in enumerate(range(-144,415,12)):
   xx=x+(6 if row%2 else 0);c.poly([(xx,y),(xx+6,y-4),(xx+12,y),(xx+6,y+4)],rng.choice(palette),'#b1a3a2',1)
 cells,pieces=support_pieces(contract,native)
 # Every blocked native cell is represented, including support outside sprite alpha.
 colors=['#b4aea3','#aaa69a','#b0ab9e']
 for p in pieces:
  color=colors[p['cell']%3];c.poly(p['displayPolygon'],color,'#938f82',1,tag='blocked-native-cell-'+str(p['cell']))
 return c,dict(nativeGridWidth=48,nativeGridHeight=81,cellSize=16,blockedCells=cells,pieces=pieces,supportPalette=colors+['#938f82'],geometrySHA256=contract['nativeSource']['geometryHash'],contractSHA256=CONTRACT_SHA)

def build():
 from PIL import Image,ImageDraw,ImageFont,__version__ as pillow_version
 contract,native=inputs();PUBLIC.mkdir(parents=True,exist_ok=True)
 frames=contract['frames']+contract['boards'];assert len(frames)==24
 ground,support=ground_canvas(contract,native);(PUBLIC/'ground.svg').write_text(ground.svg((-112,-20)))
 ground_image=ground.render((-112,-20)).convert('RGB');ground_image.save(PUBLIC/'ground.png',optimize=True,compress_level=9)
 atlas=Image.new('RGBA',(512,1024));editable=[];images={}
 for index,frame in enumerate(frames):
  c=frame_canvas(frame,index);image=c.render();r=frame['atlasRect'];assert image.size==(r['width'],r['height'])
  atlas.alpha_composite(image,(r['x'],r['y']));images[frame['id']]=image
  editable.append(dict(id=frame['id'],atlasRect=r,width=image.width,height=image.height,source='ORIGINAL_VECTOR_PRIMITIVES',primitives=c.items))
 atlas.save(PUBLIC/'atlas.png',optimize=True,compress_level=9)
 write_json(PUBLIC/'atlas-editable.json',dict(schema='mcr-original-vector-atlas-v1',version=1,width=512,height=1024,rasterAuthority='author.py/Pillow integer-rounded original primitives; SVG rasterizer parity is not claimed',frames=editable))
 write_json(HERE/'collision-support.json',support)
 ground_pin=file_pin(PUBLIC/'ground.png');atlas_pin=file_pin(PUBLIC/'atlas.png')
 def resource(pin,identity,width,height):return dict(id=identity,path=pin['path'].removeprefix('frontend/public/'),sha256='sha256:'+pin['sha256'],bytes=pin['bytes'],width_px=width,height_px=height)
 g=resource(ground_pin,'parade_photo2_display_v1_ground',499,1200);a=resource(atlas_pin,'parade_photo2_display_v1_atlas',512,1024)
 objects=deepcopy(native['scenes']['PARADE']['scenery']['objects']);by_id={f['id']:f for f in contract['frames']}
 for obj in objects:obj['atlas_id']=a['id'];obj['frame']=by_id[obj['id']]['atlasRect']
 interface=dict(schemaVersion='mcr-photo2-world-art-source-v1',version=1,variantId='photo2-parade-display-v1',themeId='PARADE',status='SOURCE_ARTIFACT_NOT_RELEASE_ADMITTED',sourceInput=dict(canonicalCommit=CANONICAL,contractPath=FOLDER+'producer-contract-v2.json',contractSHA256=CONTRACT_SHA,referenceSHA256=REFERENCE_SHA,nativeSource=contract['nativeSource']),scenery=dict(version=1,ground=g,atlases=[a],objects=objects),sceneDescriptor=dict(schema='mcr-photo2-world-display-v1',version=1,mesh=contract['mesh'],displayBounds=contract['ground']['displayBounds'],frames=contract['frames'],boards=contract['boards'],occlusion=contract['occlusion'],nativeTileTexture='SEPARATE_EXISTING_PINNED_C06_TILED_TEXTURE_UNCHANGED',collisionSupport=dict(source='EXACT_FROZEN_NATIVE_GRID_PROJECTED_BY_CONTRACT_MESH',blockedCellCount=len(support['blockedCells']),proofSHA256=sha((HERE/'collision-support.json').read_bytes()))),resourceUnits=dict(sourcePixelCount=1123088,decodedRGBABytes=4492352,sourcePixelCountLimit=2097152,transferByteLimit=2097152,actualPngTransferBytes=ground_pin['bytes']+atlas_pin['bytes']),activation=dict(state='NOT_ADMITTED',required='2162/Root coherent new PARADE theme_revision/content_hash/catalog tuple; exact new resources and descriptor SHA before activation',otherEras='REVENGE/DANGER unchanged'))
 write_json(PUBLIC/'asset-interface.json',interface)
 outputs=[file_pin(PUBLIC/p) for p in ['ground.svg','ground.png','atlas.png','atlas-editable.json','asset-interface.json','README.md']]
 provenance=dict(schema='mcr-photo2-world-display-provenance-v1',version=1,authorship='Original project-authored editable vector primitives, deterministic Pillow rasterization; no generated or sampled character/reference pixels',reference=dict(kind='USER_SUPPLIED_VISUAL_REFERENCE_ONLY',filename='2-Photo-2.jpg',upload='01a19e5c-e9c6-7973-a53b-a6d19141620f/9FC49E03-6C0E-4776-A954-2CA5C12CB495',sha256=REFERENCE_SHA,sourcePixelSampling=False),producerContract=dict(canonicalCommit=CANONICAL,path=FOLDER+'producer-contract-v2.json',sha256=CONTRACT_SHA),nativeSource=contract['nativeSource'],sourceCode=file_pin(HERE/'author.py'),outputs=outputs,dependencies=dict(python=platform.python_version(),pillow=pillow_version,network=False,newDependencies=False),preserved=['user originals','selected characters and all avatar bytes','22 native IDs/transforms/feet/depths','geometry/collision/spawns/proximity/room identity','REVENGE/DANGER','old quality-v6 pixels/hash/release tuple'],limits=['Source asset and producer checks do not admit a World release','Consumer renderer/projection/occlusion/native/whole Photo2 acceptance remains separate','No physical availability or attendance authority','SVG/PNG external-rasterizer parity not claimed'])
 write_json(PUBLIC/'source-provenance.json',provenance)
 # Honest actor-free source specimens; the original reference is untouched.
 scene=ground_image.convert('RGBA')
 for frame in sorted(frames,key=lambda f:(f.get('displayFoot',{}).get('y',f['displayRect']['y']+f['displayRect']['height']),f['id'])):
  r=frame['displayRect'];scene.alpha_composite(images[frame['id']],(round(r['x']+112),round(r['y']+20)))
 scene.convert('RGB').save(HERE/'full-scene.png')
 viewport=scene.crop((112,20,496,524)).convert('RGB');viewport.save(HERE/'scene-viewport.png')
 reference=Image.open(REFERENCE).convert('RGB').crop((441,173,825,677));pair=Image.new('RGB',(768,504));pair.paste(reference,(0,0));pair.paste(viewport,(384,0));pair.save(HERE/'reference-comparison.png')
 contact=Image.new('RGB',(1040,1560),'#ddd7cf');d=ImageDraw.Draw(contact)
 for index,frame in enumerate(frames):
  x=(index%4)*260;y=(index//4)*260;d.text((x+7,y+5),frame['id'],fill='#202020')
  im=images[frame['id']];contact.paste(im,(x+(260-im.width)//2,y+30),im)
 contact.save(HERE/'atlas-contact.png')
 support_preview=ground_image.convert('RGB');d=ImageDraw.Draw(support_preview)
 for p in support['pieces']:d.line([(round(x+112),round(y+20)) for x,y in p['displayPolygon']]+[(round(p['displayPolygon'][0][0]+112),round(p['displayPolygon'][0][1]+20))],fill='#8c315e',width=1)
 support_preview.save(HERE/'support-diagnostic.png')
 summary=dict(status='GENERATED_NOT_YET_QUALIFIED',outputs=[file_pin(PUBLIC/p) for p in ['ground.svg','ground.png','atlas.png','atlas-editable.json','asset-interface.json','source-provenance.json','README.md']],blockedCells=len(support['blockedCells']),supportPieces=len(support['pieces']),frames=len(frames),units=interface['resourceUnits'])
 write_json(HERE/'build-metadata.json',summary);print(json.dumps(summary))

if __name__=='__main__':
 assert sys.argv[1:]==['--build'],'Explicit finite execution only'
 build()
