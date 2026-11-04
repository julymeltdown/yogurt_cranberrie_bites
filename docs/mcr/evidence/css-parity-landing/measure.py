"""Coarse cream-foreground bounds, not a pixel-equivalence or art-approval gate."""
from PIL import Image
from pathlib import Path
import json
p=Path(__file__).parent
baseline=Image.open(p/'baseline-poster-390.png').convert('RGB')
reference=Image.open(p/'reference-landing-native.png').convert('RGB').resize(baseline.size,Image.Resampling.LANCZOS)
images={'reference':reference,'baseline':baseline,'candidate':Image.open(p/'candidate-poster-390.png').convert('RGB')}
regions={'copy':(70,490,310,583),'cta':(20,590,355,645)}
result={'method':'New photo crop x5,y3 to x254,y440; resized LANCZOS to actual 378x670 poster. Cream threshold R>165 G>150 B>120, fixed bounded regions. JPEG/interpolation and frame wear affect results. Bounds alone are not exact-pixel equivalence.','regions':regions,'bounds':{}}
for name,image in images.items():
 result['bounds'][name]={}
 for label,(left,top,right,bottom) in regions.items():
  pixels=[(x,y) for y in range(top,bottom) for x in range(left,right) if all(a>b for a,b in zip(image.getpixel((x,y)),(165,150,120)))]
  result['bounds'][name][label]=[min(x for x,y in pixels),min(y for x,y in pixels),max(x for x,y in pixels),max(y for x,y in pixels)]
result['edge_error_pixels']={mode:{label:sum(abs(a-b) for a,b in zip(result['bounds'][mode][label],result['bounds']['reference'][label])) for label in regions} for mode in ['baseline','candidate']}
(p/'foreground-bounds.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result['edge_error_pixels']))
