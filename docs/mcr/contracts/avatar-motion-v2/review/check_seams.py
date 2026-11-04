#!/usr/bin/env python3
"""Read-only pixel composition analysis; no source or derived images are written."""
import hashlib
import json
from pathlib import Path
from PIL import Image
ROOT=Path(__file__).resolve().parents[5]

def main():
    path=ROOT/'backend/releases/selected-character-motion-v2/manifest.json'
    raw=path.read_bytes();assert hashlib.sha256(raw).hexdigest()=='9a555f726a9e421f14ed876117be87bb1bb8fa128ac401ab38f8927760776fd6'
    m=json.loads(raw);atlas=Image.open(ROOT/m['atlas']['path']).convert('RGBA')
    parts={x['local_key']:x for x in m['items']}
    def cell(index):
        x,y=index%20*64,index//20*64
        return atlas.crop((x,y,x+64,y+64))
    def render(body,hair,eyes,outfit,accessory,direction,phase):
        def layer(key,field='frames'):
            return cell(parts[key]['directions'][direction][field][phase])
        result=Image.new('RGBA',(64,64));base=layer('body-'+str(body))
        if outfit is not None:
            base.paste((0,0,0,0),(0,42,64,64))
        result.alpha_composite(base)
        if outfit is not None:
            mask=layer('outfit-'+str(outfit),'hand_frames').getchannel('A')
            rgb=parts['body-'+str(body)]['skin_palette']['colors'][2]
            hands=Image.new('RGBA',(64,64),tuple(rgb)+(255,));hands.putalpha(mask)
            result.alpha_composite(hands);result.alpha_composite(layer('outfit-'+str(outfit)))
        result.alpha_composite(layer('eyes-'+str(eyes)))
        result.alpha_composite(layer('hair-'+str(hair)))
        if accessory is not None:result.alpha_composite(layer('accessory-'+str(accessory)))
        return result
    total=0;groups={}
    def check(group,body,hair,eyes,outfit,accessory,d,p):
        nonlocal total
        frame=render(body,hair,eyes,outfit,accessory,d,p)
        b=frame.getchannel('A').getbbox()
        assert b is not None and b[0]>0 and b[1]>0 and b[2]<64 and b[3]==60,(group,body,hair,eyes,outfit,accessory,d,p,b)
        total+=1;groups[group]=groups.get(group,0)+1
    # The first 960 cases repeat the actual skin/outfit/gait coverage independently.
    for body in range(8):
        for outfit in range(6):
            for d in range(4):
                for p in range(5):check('all-body-outfit-direction-phase',body,outfit,body%4,outfit,None,d,p)
    # Close the earlier review's optional-outfit gap over every skin, gait and hair.
    for body in range(8):
        for hair in range(6):
            for d in range(4):
                for p in range(5):check('undressed-all-body-hair-gait',body,hair,body%4,None,None,d,p)
    # Eye/hair/accessory cross-family overlap, both undressed and dressed,
    # with light/dark bodies. Stationary attachments do not need all gait repeats.
    for body in [0,7]:
        for hair in range(6):
            for eyes in range(4):
                for accessory in [None,0,1,2,3]:
                    for outfit in [None,0,5]:
                        for d in range(4):check('optional-layer-eye-hair-accessory',body,hair,eyes,outfit,accessory,d,1)
    for eye in [x for x in m['items'] if x['slot']=='eyes']:
        assert cell(eye['directions'][3]['frames'][0]).getchannel('A').getbbox() is None
    report={'result':'PASS','scope':'Independent composition alpha bounds/foot and optional-layer coverage, not visual judgment',
      'manifest_sha256':hashlib.sha256(raw).hexdigest(),'atlas_sha256':m['atlas']['sha256'],
      'cases':total,'groups':groups,'bounds':'0 < minX/minY; maxX < 64; exclusive maxY == 60',
      'body_clip_y_exclusive':42,'hand_palette_color_index':2,'rear_eyes_transparent':True,
      'source_images_modified':False,'images_generated':False}
    print(json.dumps(report,indent=2))
if __name__=='__main__':main()
