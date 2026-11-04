"""Temporary source-only previews, never App captures or durable replacement art."""
import hashlib
import datetime as dt
import json
import sys
from pathlib import Path
from PIL import Image

root = Path.cwd() / 'frontend/public/world/map-facade-derived-v3'
out = Path(sys.argv[1])
assert out.is_dir() and not list(out.iterdir())
rows = []
for era in ('revenge', 'parade', 'danger'):
    with Image.open(root/(era+'-scene.png')) as source:
        assert source.size == (768, 1296) and source.mode == 'RGB'
        for width in (360, 390, 430):
            height = (1296*width+384)//768
            path = out/(era+'-'+str(width)+'.png')
            source.resize((width, height), Image.Resampling.NEAREST).save(path, format='PNG', compress_level=6)
            raw = path.read_bytes()
            rows.append(dict(file=path.name, width=width, height=height, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()))
            assert sum(p.stat().st_size for p in out.iterdir()) <= 4194304
offer=sys.argv[2];attempt=sys.argv[3]
assert Path(offer).name==offer and attempt.isdigit()
retained=Path(__file__).resolve().parent/(offer+'-attempt-'+attempt+'-parade-390-source-preview.png')
assert not retained.exists()
raw=(out/'parade-390.png').read_bytes();retained.write_bytes(raw)
print(json.dumps(dict(qualification='SOURCE_RASTER_ONLY_NOT_APP_CAPTURE', previews=rows,
    retainedSourcePreview=dict(path=str(retained),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),
        renderedBy=dt.datetime.now(dt.timezone.utc).isoformat(),notAppCapture=True))))

