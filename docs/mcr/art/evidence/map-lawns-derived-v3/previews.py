"""New lawn SOURCE-ONLY comparisons. No App/browser or old test-matrix replay."""
from pathlib import Path
import hashlib
import json
from PIL import Image, ImageDraw

ROOT=Path(__file__).resolve().parents[5]
PACK=ROOT/'frontend/public/world/map-lawns-derived-v3'
BASE=ROOT/'frontend/public/world/map-roof-street-derived-v2'
OUT=Path(__file__).resolve().parent


def main():
    images=[]
    for era in ('revenge','parade','danger'):
        base=Image.open(BASE/(era+'-scene.png')).convert('RGBA')
        overlay=Image.open(PACK/(era+'-overlay.png')).convert('RGBA')
        candidate=Image.alpha_composite(base,overlay).convert('RGB')
        path=OUT/(era+'-source-full.png');candidate.save(path,compress_level=9);images.append(path)
        for width in (360,390,430):
            h=round(1296*width/768);top=round(350*width/768);bottom=round(1070*width/768)
            a=base.convert('RGB').resize((width,h),Image.Resampling.NEAREST).crop((0,top,width,bottom))
            b=candidate.resize((width,h),Image.Resampling.NEAREST).crop((0,top,width,bottom))
            board=Image.new('RGB',(2*width+8,bottom-top+24),'#F4E8D8')
            board.paste(a,(0,24));board.paste(b,(width+8,24));d=ImageDraw.Draw(board)
            d.text((5,5),f'V2 BASE / MAP WIDTH {width}',fill='#211B18')
            d.text((width+13,5),f'NEW LAWN / {width} / SOURCE ONLY',fill='#211B18')
            path=OUT/(era+f'-gardens-{width}.png');board.save(path,compress_level=9);images.append(path)
    records=[{'path':str(p.relative_to(ROOT)),'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in images]
    (OUT/'preview-pins.json').write_text(json.dumps({'format':'MCR_GROUND_SOURCE_PREVIEWS_V1','authority':'SOURCE_ONLY_NOT_APP_OR_LIVE_MAP','fullSourceSize':[768,1296],'eachComparisonPaneMapWidths':[360,390,430],'sourceCropY':[350,1070],'images':records},indent=2)+'\n')
    print('PASS: 3 actual full source compositions and 9 paired garden crops, each pane uses its stated Map width; no App/native claim')


if __name__=='__main__':main()
