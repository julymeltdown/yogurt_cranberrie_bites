#!/usr/bin/env python3
"""Native editable virtual scenery; no reference/source raster pixels are edited."""
import collections
import hashlib
import json
import math
from pathlib import Path
import random

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
W, H, CELL = 768, 1296, 16
BASE = '1e5022885525ab56f056aa2c24100e18c4bc67ed'
TARGET = 'docs/mcr/design/reference-target-2026-11-04.png'
TARGET_HASH = 'd9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617'


def canonical(value):
    return json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(',', ':')).encode('ascii')


def digest(value):
    return 'sha256:' + hashlib.sha256(canonical(value)).hexdigest()


def save(name, value):
    (HERE / name).write_text(json.dumps(value, separators=(',', ':')) + '\n' if name.endswith('-editable.json') else json.dumps(value, indent=2) + '\n')


def inside(x, y, polygon):
    hit = False
    for i, (ax, ay) in enumerate(polygon):
        bx, by = polygon[i-1]
        if (ay > y) != (by > y) and x < (bx-ax)*(y-ay)/(by-ay)+ax:
            hit = not hit
    return hit


def cell_intersects(cx, cy, polygon):
    """Conservative exact convex polygon/cell SAT; touching is solid."""
    rect=[(cx*CELL,cy*CELL),((cx+1)*CELL,cy*CELL),((cx+1)*CELL,(cy+1)*CELL),(cx*CELL,(cy+1)*CELL)]
    axes=[(1,0),(0,1)]+[(polygon[i-1][1]-p[1],p[0]-polygon[i-1][0]) for i,p in enumerate(polygon)]
    for ax,ay in axes:
        a=[x*ax+y*ay for x,y in polygon];b=[x*ax+y*ay for x,y in rect]
        if max(a)<min(b) or max(b)<min(a):return False
    return True


class Art:
    def __init__(self, width, height):
        self.width, self.height, self.items = width, height, []

    def poly(self, points, color, group='detail'):
        # Exported editable shape vertices are clipped, not left outside texture bounds.
        for axis, bound, keep_greater in [(0,0,True),(0,self.width,False),(1,0,True),(1,self.height,False)]:
            clipped=[]
            for i,point in enumerate(points):
                previous=points[i-1]
                pin=point[axis]>=bound if keep_greater else point[axis]<=bound
                qin=previous[axis]>=bound if keep_greater else previous[axis]<=bound
                if pin != qin:
                    t=(bound-previous[axis])/(point[axis]-previous[axis])
                    clipped.append([previous[0]+t*(point[0]-previous[0]),previous[1]+t*(point[1]-previous[1])])
                if pin:clipped.append(point)
            points=clipped
            if len(points)<3:return
        # Overlapping paths in a native nonzero fill must have consistent winding.
        if sum(points[i-1][0]*p[1]-p[0]*points[i-1][1] for i,p in enumerate(points)) < 0:
            points=list(reversed(points))
        self.items.append((group, color, [[round(x, 2), round(y, 2)] for x, y in points]))

    def rect(self, x, y, w, h, color, group='detail'):
        self.poly([[x,y],[x+w,y],[x+w,y+h],[x,y+h]], color, group)

    def line(self, ax, ay, bx, by, color, width=1, group='detail'):
        length = max(1, math.hypot(bx-ax, by-ay)); dx, dy = -(by-ay)*width/length/2, (bx-ax)*width/length/2
        self.poly([[ax+dx,ay+dy],[bx+dx,by+dy],[bx-dx,by-dy],[ax-dx,ay-dy]], color, group)

    def oval(self, x, y, w, h, color, group='detail', steps=12):
        self.poly([[x+w/2+math.cos(i*math.tau/steps)*w/2,y+h/2+math.sin(i*math.tau/steps)*h/2] for i in range(steps)], color, group)

    def svg(self, name):
        groups = collections.OrderedDict()
        for group, color, points in self.items:
            groups.setdefault(group, []).append(f'<polygon points="{" ".join(f"{x},{y}" for x,y in points)}" fill="{color}"/>')
        return f'<svg xmlns="http://www.w3.org/2000/svg" width="{self.width}" height="{self.height}" viewBox="0 0 {self.width} {self.height}"><title>{name}</title>' + ''.join(f'<g id="{group}">{"".join(items)}</g>' for group,items in groups.items()) + '</svg>'

    def document(self, name):
        # Aggregate same-color subpaths within a named semantic group. All geometry stays editable.
        layers, next_id = [], 1
        for group in dict.fromkeys(x[0] for x in self.items):
            colors = collections.OrderedDict()
            for item_group, color, points in self.items:
                if item_group == group:
                    commands = [{'type':'moveTo','x':points[0][0],'y':points[0][1]}]
                    commands += [{'type':'lineTo','x':x,'y':y} for x,y in points[1:]] + [{'type':'close'}]
                    colors.setdefault(color, []).extend(commands)
            for color, commands in colors.items():
                rgba = [int(color[i:i+2],16)/255 for i in (1,3,5)]+[1]
                layers.append({'id':next_id,'name':group+' '+color,'type':'Shape','blendMode':'normal','activeRange':{'start':0,'duration':3000},
                               'transform':{'anchorPoint':[0,0],'position':[0,0],'scale':[100,100],'rotation':0,'opacity':100},
                               'shape':{'path':{'commands':commands},'fills':[{'paint':{'type':'solid','color':rgba},'fillRule':'nonZeroWinding','blendMode':'normal','opacity':100}]}})
                next_id += 1
        return {'$schema':'https://jerboa.dev/schemas/fx-composition/editable/v1/document.schema.json','formatVersion':1,'dimensions':{'width':self.width,'height':self.height},'duration':3,
                'colorManagement':{'workingSpace':'legacyDisplay'},'composition':{'id':'main','name':name,'layers':list(reversed(layers))}}


FONT = {
 'A':['01110','10001','10001','11111','10001','10001','10001'], 'B':['11110','10001','10001','11110','10001','10001','11110'],
 'C':['01111','10000','10000','10000','10000','10000','01111'], 'D':['11110','10001','10001','10001','10001','10001','11110'],
 'E':['11111','10000','10000','11110','10000','10000','11111'], 'F':['11111','10000','10000','11110','10000','10000','10000'],
 'G':['01111','10000','10000','10111','10001','10001','01111'], 'H':['10001','10001','10001','11111','10001','10001','10001'],
 'I':['111','010','010','010','010','010','111'], 'K':['10001','10010','10100','11000','10100','10010','10001'],
 'L':['10000','10000','10000','10000','10000','10000','11111'], 'M':['10001','11011','10101','10101','10001','10001','10001'],
 'N':['10001','11001','11001','10101','10011','10011','10001'], 'O':['01110','10001','10001','10001','10001','10001','01110'],
 'P':['11110','10001','10001','11110','10000','10000','10000'], 'R':['11110','10001','10001','11110','10100','10010','10001'],
 'S':['01111','10000','10000','01110','00001','00001','11110'], 'T':['11111','00100','00100','00100','00100','00100','00100'],
 'U':['10001','10001','10001','10001','10001','10001','01110'], 'V':['10001','10001','10001','10001','10001','01010','00100'],
 'W':['10001','10001','10001','10101','10101','11011','10001'], 'Y':['10001','10001','01010','00100','00100','00100','00100'],
 'Z':['11111','00001','00010','00100','01000','10000','11111'], ' ':['000']*7,
}


def lettering(art, text, x, y, size, color, group='lettering'):
    for char in text:
        glyph=FONT.get(char, FONT[' '])
        for row,bits in enumerate(glyph):
            for col,bit in enumerate(bits):
                if bit=='1': art.rect(x+col*size,y+row*size,size,size,color,group)
        x += (len(glyph[0])+1)*size


def corridor(points, radius):
    result=[]
    for (ax,ay),(bx,by) in zip(points,points[1:]):
        length=math.hypot(bx-ax,by-ay);dx,dy=-(by-ay)*radius/length,(bx-ax)*radius/length
        result.append([[ax+dx,ay+dy],[bx+dx,by+dy],[bx-dx,by-dy],[ax-dx,ay-dy]])
    return result


PAVING = corridor([[16,1290],[270,980],[270,720],[500,470],[752,210]],48)+corridor([[16,190],[280,480],[530,780],[752,1260]],42)
PAVING += [[[150,310],[500,240],[710,405],[520,650],[170,610],[80,440]]]


def ground(theme):
    art=Art(W,H); r=random.Random(1406)
    art.rect(0,0,W,H,'#81814d','lawn-base')
    for n in range(9000):
        x,y=r.randrange(W),r.randrange(H)
        art.rect(x,y,r.choice([1,2,3]),r.choice([1,2]),r.choice(['#989a59','#6c773e','#a4a666','#75783c']),'lawn-blades')
    for points in PAVING: art.poly(points,'#9f9480','diagonal-paved-corridors')
    stone=['#c5b59c','#d1c0a4','#b3a58e','#d6c7af','#bcae97']
    for y in range(0,H,9):
        for x in range(-18,W,18):
            px=x+(9 if y//9%2 else 0);py=y
            if any(inside(px+8,py+4,p) for p in PAVING):
                art.poly([[px+1,py+1],[px+14,py],[px+17,py+4],[px+14,py+8],[px+2,py+8],[px,py+4]],r.choice(stone),'individual-paving-stones')
                if r.random()>.5:art.rect(px+3,py+1,6,1,'#e4d7be','paving-highlights')
    # Ground cover and shrubs belong to the actual islands, never scattered over walking paths.
    for n in range(180):
        x,y=r.randrange(10,W-10),r.randrange(10,H-10)
        if not any(inside(x,y,p) for p in PAVING):
            art.rect(x,y,2,2,'#ead3a0' if n%3 else '#af452f','island-wildflowers')
    return art


def stage(theme):
    a=Art(336,280); ink='#242128';steel='#b1a99b';red='#b63438' if theme!='DANGER' else '#d9bb44'
    a.poly([[8,262],[274,211],[334,238],[65,279]],'#5f5549','platform-shadow')
    a.poly([[21,235],[281,185],[330,215],[65,267]],'#363039','raised-deck')
    for i in range(8): a.line(24+i*6,239+i*3,284+i*6,188+i*3,'#62555b',2,'deck-planks')
    a.poly([[29,60],[282,12],[282,195],[22,246]],'#17171a','black-stage-backdrop')
    for i in range(11):
        x=32+i*22;y=60-i*4.2
        a.line(x,y,x-2,y+160,'#31262b',5,'backdrop-folds')
    # Deliberately original slanted rough letterforms, not an extracted band logo.
    for points in [ [(85,162),(86,96),(106,128),(124,87),(121,154)], [(183,83),(155,84),(145,99),(143,126),(155,143),(179,140)], [(202,137),(204,75),(228,72),(241,84),(233,101),(206,106),(242,132)] ]:
        for (x,y),(u,v) in zip(points,points[1:]):a.line(x,y,u,v,'#eee6d1',7,'original-MCR-lettering')
    for ax,ay,bx,by in [(25,58,282,10),(25,72,282,24),(22,243,279,193),(29,61,23,242),(45,57,39,239),(280,12,280,197),(297,19,296,203),(25,58,70,85),(282,10,330,39),(70,85,330,39),(69,98,329,52),(326,39,326,217)]:
        a.line(ax,ay,bx,by,ink,7,'truss-ink');a.line(ax,ay,bx,by,steel,2,'truss-tubes')
    for y in range(75,234,25):
        a.line(27,y,42,y+19,steel,2,'truss-cross-bracing');a.line(42,y,26,y+22,steel,2,'truss-cross-bracing')
    for y in range(35,188,23):
        a.line(282,y,297,y+18,steel,2,'right-cross-bracing');a.line(297,y,282,y+21,steel,2,'right-cross-bracing')
    for x in range(56,279,35):
        y=55-(x-25)*.185;a.line(x,y,x+25,y+9,steel,2,'roof-cross-bracing');a.line(x+25,y-5,x,y+13,steel,2,'roof-cross-bracing')
        a.rect(x,y+20,12,8,'#34222a','lamp-fixtures');a.oval(x+2,y+23,9,9,red,'red-stage-lights',8)
    for x,y in [(48,173),(253,135)]:
        a.rect(x,y,22,53,ink,'speaker-cabinets');a.rect(x+3,y+4,16,45,'#47414a','speaker-faces')
        a.oval(x+5,y+11,12,12,'#202127','speaker-drivers');a.oval(x+5,y+30,12,12,'#202127','speaker-drivers')
    for x,y in [(140,192),(170,186),(203,180)]:
        a.oval(x,y,23,16,'#5b2931','drum-shells');a.oval(x,y-2,23,6,'#c7bb9c','drum-heads')
    for x,y in [(150,175),(218,164)]:a.line(x,y,x,y+40,steel,2,'cymbal-stands');a.oval(x-13,y,27,5,'#b7975f','cymbals')
    return a


def booth(theme, seed=1):
    a=Art(144,164);r=random.Random(seed)
    a.poly([[8,143],[121,135],[142,150],[28,163]],'#71634f','booth-shadow')
    a.rect(17,65,116,79,'#8e6655','counter-stall');a.rect(22,78,105,45,'#2c292b','counter-interior')
    a.poly([[4,58],[71,3],[140,48],[127,74],[20,86]],'#f1e7d6','white-canopy')
    a.poly([[71,3],[74,67],[127,74],[140,48]],'#cdbfa9','canopy-fold-shadow')
    a.line(71,3,20,86,'#fff2dc',2,'canopy-seams');a.line(71,3,127,74,'#fff2dc',2,'canopy-seams')
    for x in range(22,128,13):a.poly([[x,78],[x+10,77],[x+7,88],[x+2,88]],'#d9c8af','canopy-scallops')
    for x in [18,131]:a.line(x,67,x,146,'#42352f',4,'canopy-posts')
    accent='#a03b39' if theme!='DANGER' else '#b09b37'
    a.rect(24,125,101,24,accent,'booth-counter-front');a.line(24,128,125,128,'#e1c48c',2,'counter-trim')
    for n in range(16):
        x=28+n*6;y=111-r.randrange(10);a.rect(x,y,5,10+r.randrange(9),r.choice(['#d1b77c','#c4bfc1','#b2524b','#292a2d']),'merch-stock')
    a.rect(38,90,24,15,'#e1d4b5','hanging-display');a.rect(78,84,19,22,'#393238','hanging-display')
    return a


def tree(color, seed, width=170, height=205):
    a=Art(width,height);r=random.Random(seed);cx=width*.53
    palettes={'red':['#442f2b','#792f2c','#a33a32','#c94d3d','#df6a48'], 'yellow':['#4e5632','#7b813b','#a4a04b','#c0b557','#d2c36c'],
              'green':['#374a2c','#526335','#6d7e3d','#8d9948','#b0ae63']}
    p=palettes[color]
    a.oval(cx-31,height-24,66,22,'#514e33','root-bed-shadow')
    a.poly([[cx-8,height-8],[cx+11,height-10],[cx+8,height-103],[cx-5,height-97]],'#625039','rooted-trunk')
    for dx in [-36,-21,24,43]:a.line(cx,height-68,cx+dx,height-112-abs(dx)//2,'#64533b',7,'branch-structure')
    # Connected shadow mass prevents the floating repeated-cluster appearance.
    a.oval(cx-width*.43,height*.08,width*.86,height*.65,p[0],'connected-canopy-shadow',18)
    a.oval(cx-width*.39,height*.10,width*.80,height*.60,p[1],'connected-canopy-midtones',18)
    for n in range(115):
        angle=r.random()*math.tau;radius=math.sqrt(r.random());x=cx+math.cos(angle)*width*.39*radius;y=height*.40+math.sin(angle)*height*.31*radius
        rw,rh=r.randrange(15,29),r.randrange(13,24)
        a.poly([[x-rw/2,y],[x-rw/3,y-rh/2],[x+rw/4,y-rh/2],[x+rw/2,y],[x+rw/3,y+rh/2],[x-rw/4,y+rh/2]],p[r.randrange(1,5)],'unique-canopy-clusters')
        for _ in range(3):a.rect(x+r.randrange(-7,8),y+r.randrange(-6,7),r.choice([2,3,4]),2,p[r.randrange(1,5)],'leaf-pixel-highlights')
    for n in range(28):
        x=cx+r.randrange(-32,32);y=height-25+r.randrange(12);a.rect(x,y,4,3,p[r.randrange(1,4)],'root-bed-leaves')
    return a


def plaque():
    a=Art(156,96)
    for x in [20,131]:a.rect(x,54,5,38,'#554334','plaque-posts')
    a.rect(4,4,148,65,'#211f27','plaque-outer-ink');a.rect(8,8,140,57,'#d8cbb6','plaque-frame');a.rect(11,11,134,51,'#333039','plaque-face')
    lettering(a,'CULTURE',24,19,2,'#f1ead8');lettering(a,'PARK',44,39,2,'#f1ead8')
    return a


def banner(theme):
    a=Art(50,134);a.rect(23,4,4,126,'#746b58','banner-pole');a.rect(2,0,46,5,'#322d30','crossbar')
    a.poly([[8,6],[42,6],[42,96],[25,110],[8,96]],'#25232a','cloth');a.poly([[8,6],[13,6],[13,96],[25,105],[25,110],[8,96]],'#a83a3a','cloth-edge')
    a.rect(23,24,5,56,'#e5dcc4','cross');a.rect(15,39,21,5,'#e5dcc4','cross')
    return a


def building(theme, width=176, height=154):
    a=Art(width,height)
    a.poly([[8,75],[124,46],[168,75],[168,142],[46,154],[8,130]],'#817764','walls')
    a.poly([[3,71],[119,26],[174,60],[54,103]],'#725354','roof')
    a.poly([[54,103],[174,60],[174,70],[54,113]],'#473d3c','roof-fascia')
    for n in range(9):a.line(15+n*14,67-n*4,57+n*13,94-n*4,'#9c7270',2,'roof-tile-ridges')
    for y in [119,133]:a.line(12,y,164,y-20,'#aa9981',1,'masonry-courses')
    for x in range(25,130,23):a.rect(x,110-(x-25)*.12,11,18,'#352f31','windows');a.rect(x+2,111-(x-25)*.12,7,12,'#ac8360','window-glow')
    return a


def main():
    assert hashlib.sha256((ROOT/TARGET).read_bytes()).hexdigest()==TARGET_HASH
    all_resources=[]
    for theme in ['PARADE','REVENGE','DANGER']:
        terrain=ground(theme);(HERE/f'{theme.lower()}-ground.svg').write_text(terrain.svg(theme+' editable diagonal ground'))
        save(theme.lower()+'-ground-editable.json',terrain.document(theme+' native ground'))
        # Native individually authored scenery. No copied tree sprite is instanced to fill space.
        sprites=[('stage',stage(theme)),('white_merch_booth',booth(theme,1)),('white_food_booth',booth(theme,7)),('park_plaque',plaque()),('cross_banner',banner(theme)),('meeting_pavilion',building(theme)),('giveaway_pavilion',building(theme))]
        for i,color in enumerate(['red','red','yellow','green','yellow','red','green','yellow','green','red','yellow']):sprites.append((f'tree_{i+1:02}',tree(color,310+i*37,166,204)))
        # Shelf packing within a small cacheable atlas. Each tree is a unique authored frame.
        atlas=Art(1024,1024);frames={};x=y=row_h=0
        for name,art in sprites:
            if x+art.width+4>atlas.width:x=0;y+=row_h+4;row_h=0
            assert y+art.height<=atlas.height
            frames[name]={'x':x,'y':y,'width':art.width,'height':art.height}
            for group,color,points in art.items:atlas.poly([[px+x,py+y] for px,py in points],color,name+'_'+group)
            x+=art.width+4;row_h=max(row_h,art.height)
        (HERE/f'{theme.lower()}-scenery.svg').write_text(atlas.svg(theme+' native scenery atlas'))
        save(theme.lower()+'-scenery-editable.json',atlas.document(theme+' native scenery atlas'))
        save(theme.lower()+'-frames.json',frames)
        all_resources.append({'theme_id':theme,'ground_source':theme.lower()+'-ground.svg','atlas_source':theme.lower()+'-scenery.svg','ground_editable':theme.lower()+'-ground-editable.json','atlas_editable':theme.lower()+'-scenery-editable.json'})
    placements=[
      {'id':'main_stage','frame':'stage','x':176,'y':96,'depth_y':366,'footprint':[[195,302],[460,250],[502,310],[242,365]],'interaction_id':'stage_info'},
      {'id':'north_white_food','frame':'white_food_booth','x':650,'y':120,'width':112,'height':128,'depth_y':247,'footprint':[[664,218],[748,214],[754,241],[665,247]],'interaction_id':None},
      {'id':'west_white_merch','frame':'white_merch_booth','x':130,'y':490,'depth_y':650,'footprint':[[149,615],[254,609],[263,640],[150,649]],'interaction_id':'gifts_info'},
      {'id':'culture_park_plaque','frame':'park_plaque','x':585,'y':378,'depth_y':470,'footprint':[[603,447],[723,447],[723,471],[603,471]],'interaction_id':'park_info'},
      {'id':'cross_banner_east','frame':'cross_banner','x':522,'y':200,'depth_y':330,'footprint':[[543,319],[552,319],[552,332],[543,332]],'interaction_id':None},
      {'id':'meeting_pavilion','frame':'meeting_pavilion','x':285,'y':770,'depth_y':924,'footprint':[[303,874],[451,852],[451,913],[330,924]],'interaction_id':None},
      {'id':'giveaway_pavilion','frame':'giveaway_pavilion','x':440,'y':1080,'depth_y':1234,'footprint':[[458,1184],[606,1162],[606,1223],[485,1234]],'interaction_id':None},
    ]
    positions=[(146,0),(328,0),(558,0),(2,245),(596,240),(592,632),(5,680),(135,875),(594,875),(8,1090),(410,600)]
    for i,(x,y) in enumerate(positions):
        # Trunk/root footprint is solid; canopy overhang sorts at its rooted foot, not its top.
        placements.append({'id':f'rooted_tree_{i+1:02}','frame':f'tree_{i+1:02}','x':x,'y':y,'depth_y':y+196,
                           'footprint':[[x+70,y+181],[x+102,y+181],[x+104,y+199],[x+67,y+199]],'interaction_id':None,
                           'placement_reason':'Named rooted perimeter/island tree; unique canopy geometry and seed, no reused raster instances.'})
    old_places=json.loads((ROOT/'frontend/public/world/maps/virtual-places.json').read_text());old_geometry=json.loads((ROOT/'frontend/public/world/maps/geometry.json').read_text())
    point_by_id={p['interaction_id']:p for p in old_geometry['interactions']}
    points=[]
    for key,x,y in [('stage_info',440,408),('park_info',616,488),('gifts_info',216,680),('wall_info',168,1100)]:
        points.append({'interaction_id':key,'place_id':point_by_id[key]['place_id'],'cell_x':x//CELL,'cell_y':y//CELL})
    collision=[]
    for cy in range(H//CELL):
        row=[]
        for cx in range(W//CELL):
            blocked=cx in [0,W//CELL-1] or cy in [0,H//CELL-1] or any(cell_intersects(cx,cy,p['footprint']) for p in placements)
            row.append(int(blocked))
        collision.append(row)
    geometry={'width_cells':W//CELL,'height_cells':H//CELL,'tile_width_px':CELL,'tile_height_px':CELL,'collision_id':'reference_scene_v2_proposal_collision',
              'collision_grid':collision,'terrain_layer':[[2 if any(inside((x+.5)*CELL,(y+.5)*CELL,p) for p in PAVING) else 1 for x in range(W//CELL)] for y in range(H//CELL)],
              'overdraw_layer':[[0]*(W//CELL) for _ in range(H//CELL)],'spawns':[{'spawn_id':'courtyard_entry','cell_x':29,'cell_y':30},{'spawn_id':'community_entry','cell_x':31,'cell_y':58}],
              'default_spawn_id':'courtyard_entry','interactions':points}
    proposal={'format':'MCR_REFERENCE_SCENE_PROPOSAL','proposal_version':2,'status':'PROPOSAL_NOT_RELEASE_BOUND','worker_base':BASE,
              'target':{'path':TARGET,'sha256':'sha256:'+TARGET_HASH,'panels':['WORLD (third)','MAP (fourth)']},'coordinates':{'width_px':W,'height_px':H,'origin':'top-left, x right/y down; virtual pixels only'},
              'geometry':geometry,'geometry_hash':digest(geometry),'placements':placements,'paving_polygons':PAVING,
              'world_camera':{'x':176,'y':72,'width':568,'height':568,'notes':'Stage-led close courtyard; no backend position/admission meaning.'},
              'overview_camera':{'x':0,'y':0,'width':W,'height':H},'places':[p for p in old_places if p['place_id'] in {q['place_id'] for q in points}],
              'map_labels':[{'label':'MAIN STAGE','x':440,'y':190,'interaction_id':'stage_info'},{'label':'PLAZA','x':364,'y':545,'interaction_id':'park_info'},
                            {'label':'MERCH','x':196,'y':640,'interaction_id':'gifts_info'},{'label':'FOOD','x':650,'y':665,'interaction_id':None},
                            {'label':'MEETUP ZONE','x':372,'y':866,'interaction_id':None},{'label':'GIVEAWAY BOOTH','x':540,'y':1180,'interaction_id':None}],
              'resources':all_resources,'identities':'Only four existing C06 virtual place/interaction pairs are referenced; other labels/objects are unbound virtual decoration, not invented backend identities.',
              'limitations':['Not a physical venue map, arrival guide or safe route.','No current MapManifest/release/renderer/schema changes.','No characters drawn or embedded; selected static derivatives belong to AR03 owner.','Server/client adoption of this changed geometry requires coordinated release binding.']}
    save('geometry-proposal.json',proposal)
    save('source-provenance.json',{'base':BASE,'reference':proposal['target'],'authorship':'New original native polygon/rectangle illustration and bitmap lettering; no target/source raster pixel extraction, no raster edits, no generated characters.',
                                  'editable_sources':'Tesseract native geometry plus SVGs and deterministic build.py. Existing generated scenery inspected, not reused in this first increment.',
                                  'license':'Original MCR scenery geometry; no third-party raster/font copied or embedded. System/browser fonts used only in review HTML.',
                                  'source_originals':'Unchanged. Reference/source screenshots are review inputs only, never product textures.','scope':'frontend/public/world/reference-scene-v2/** and docs/mcr/art/reference-scene-v2.md only.'})
    print(json.dumps({'resources':len(all_resources),'objects':len(placements),'geometry_hash':proposal['geometry_hash'],'world_size':[W,H]}))


if __name__=='__main__':main()
