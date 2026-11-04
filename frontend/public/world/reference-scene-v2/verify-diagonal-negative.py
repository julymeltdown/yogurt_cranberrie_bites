#!/usr/bin/env python3
"""Reject reviewed wrong-bitmap and old-geometry mutations, entirely in memory."""
import contextlib,hashlib,io,json,types,zipfile
from pathlib import Path
from unittest.mock import patch
HERE=Path(__file__).resolve().parent;OUT=HERE/'quality-v10';VERIFIER=HERE/'verify-diagonal-composition.py'
original_read=Path.read_bytes;original_text=Path.read_text;original_stat=Path.stat;original_zip=zipfile.ZipFile
read=lambda p:json.loads(original_text(p))
sha=lambda b:'sha256:'+hashlib.sha256(b).hexdigest()
encode=lambda v:(json.dumps(v,indent=2)+'\n').encode()
source=original_text(VERIFIER)
def must_reject(name,files,expected):
 manifest=read(OUT/'manifest.json')
 for pin in [*manifest['ground_sources'],*[f for t in manifest['themes'] for f in t['files']]]:
  file=OUT/pin['path']
  if file in files:pin.update(sha256=sha(files[file]),bytes=len(files[file]))
 files={**files,OUT/'manifest.json':encode(manifest)}
 def raw(p,*a,**k):return files[p] if p in files else original_read(p,*a,**k)
 def text(p,*a,**k):return files[p].decode() if p in files else original_text(p,*a,**k)
 def stat(p,*a,**k):return types.SimpleNamespace(st_size=len(files[p])) if p in files else original_stat(p,*a,**k)
 def archive(file,*a,**k):return original_zip(io.BytesIO(files[file]),*a,**k) if isinstance(file,Path) and file in files else original_zip(file,*a,**k)
 def no_write(p,*a,**k):
  assert p==OUT/'verification.json','Unexpected attempted write in read-only mutation proof'
  return len(a[0])
 try:
  with patch.object(Path,'read_bytes',raw),patch.object(Path,'read_text',text),patch.object(Path,'stat',stat),patch.object(Path,'write_text',no_write),patch.object(zipfile,'ZipFile',archive),contextlib.redirect_stdout(io.StringIO()):
   exec(compile(source,str(VERIFIER),'exec'),{'__file__':str(VERIFIER),'__name__':'__main__'})
 except AssertionError as error:
  assert str(error)==expected,(name,str(error))
  return {'case':name,'result':'REJECTED','reason':str(error)}
 raise AssertionError(name+' was incorrectly accepted')
# A wrong packaged material image must fail even when its outer manifest pin is fresh.
buffer=io.BytesIO()
with original_zip(OUT/'ground.tsrct') as old,original_zip(buffer,'w') as new:
 for item in old.infolist():new.writestr(item,original_read(HERE/'quality-v6/ground.png') if item.filename.endswith('.png') else old.read(item.filename))
results=[must_reject('legacy ground bitmap inside editable project',{OUT/'ground.tsrct':buffer.getvalue()},'Packed ground source differs from selected material ground')]
# Refresh every changed raw-document manifest pin, so rejection tests the semantic
# geometry binding rather than merely detecting a stale outer checksum.
old_hash=read(HERE/'spatial-v4/geometry-proposal.json')['geometry_hash'];files={}
for era in ['revenge','parade','danger']:
 for name in ['layout.json','map-metadata.json','resource-inputs.json','provenance.json']:
  file=OUT/era/name;doc=read(file);doc['geometry_hash']=old_hash;files[file]=encode(doc)
results.append(must_reject('all12 theme geometry hashes reverted with fresh pins',files,'Theme geometry binding differs from canonical spatial-v5'))
print(json.dumps({'status':'PASS_READ_ONLY_MUTATION_PROOFS','cases':results,'writes':0}))
