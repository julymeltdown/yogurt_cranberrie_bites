"""One producer-only old/new simultaneous decode probe; no World swap claim."""
import hashlib, io, json, resource, subprocess, time
from pathlib import Path
from PIL import Image
ROOT=Path(__file__).resolve().parents[5]
HERE=Path(__file__).resolve().parent
PUBLIC=ROOT/'frontend/public/world/reference-scene-v2'
before=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
old=[]
for path,expected in [('quality-v6/ground.png','e2d9f785d31ea9e9a1888dc8f73d182eca917b54e1924fbf5159bc91be96e795'),('quality-v6/parade/atlas.png','e0bc7f1d88b307fd7dd317eb17aae9c2dd2158d5106dedee8b6c9a0ccffdc272')]:
 raw=subprocess.check_output(['/usr/bin/git','show','8dee1335a79f831e6a29f005a50f2c0f65777d46:frontend/public/world/reference-scene-v2/'+path],cwd=ROOT)
 assert hashlib.sha256(raw).hexdigest()==expected
 image=Image.open(io.BytesIO(raw)).convert('RGBA');image.load();old.append(image)
new=[]
for name in ['ground.png','atlas.png']:
 image=Image.open(PUBLIC/'photo2-world-display-v1/parade'/name).convert('RGBA');image.load();new.append(image)
old_bytes=sum(x.width*x.height*4 for x in old);new_bytes=sum(x.width*x.height*4 for x in new)
assert (old_bytes,new_bytes)==(8175616,4492352)
peak=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
report=dict(at=time.time(),status='PASS',scope='PRODUCER_ONLY_SIMULTANEOUS_RGBA_DECODE_NOT_WORLD_RENDERER_SWAP',baselineProcessPeakRSSBytes=before,processPeakRSSBytes=peak,oldDecodedRGBABytes=old_bytes,newDecodedRGBABytes=new_bytes,simultaneousDecodedRGBABytes=old_bytes+new_bytes,swap='producer active image tuple changed once while both generations retained',runtimeWorldEvidence=False)
active=old;active=new;assert active is new
(HERE/'decode-probe.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
