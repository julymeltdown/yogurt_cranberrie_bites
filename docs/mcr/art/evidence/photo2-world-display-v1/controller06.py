"""Source06 exact new binding; retained raster/type greens are never replayed."""
from pathlib import Path
import hashlib
raw=Path(__file__).with_name('controller.py').read_bytes()
assert hashlib.sha256(raw).hexdigest()=='f1b53f9fa351b1ee9dc571d8f49fe208922ecff05eb8853e7c2ee5eaeb6dfd94'
code=raw.decode().replace('source03','source06').replace("HERE/'source-control.py'","HERE/'source-control05.py'")
code=code.replace('ROOT=control.ROOT;INDEX=control.INDEX;OFFER=control.OFFER',"ROOT=control.ROOT;INDEX=control.INDEX;OFFER=control.OFFER\nassert OFFER=='ART231-PHOTO2-SOURCE06-0715' and Path(spec.origin).name=='source-control05.py' and Path(__file__).name=='controller06.py', 'Exact new controller/guard/offer binding'")
code=code.replace('1793862900','1793863200').replace('Accept by07:15UTC','Accept by07:20UTC')
code=code.replace('remainingNewUpperBytes=64*control.MIB','remainingNewUpperBytes=35228343,priorSource03ActualPositiveBytes=6223996,priorSource04ConservativeUpperBytes=16777216,priorWrongBindingAttemptActualPositiveBytes=8879309,priorSourceFix16BookRetained=16777216,priorFreeze4ActualHigherBytes=4625653')
code=code.replace("['author.py','controller.py','source-control.py','memory-probe.py','scoped-tsconfig.json']", "['author.py','controller06.py','controller.py','source-control05.py','source-control04.py','source-control.py','memory-probe.py','scoped-tsconfig.json','collision-support.json']")
code=code.replace("ROOT/'frontend/tsconfig.json']", "ROOT/'frontend/tsconfig.json']+[ROOT/'frontend/public/world/reference-scene-v2/photo2-world-display-v1/parade'/n for n in ['ground.svg','ground.png','atlas.png','atlas-editable.json','asset-interface.json','source-provenance.json']]")
start=code.index(' jobs=[');end=code.index('\n for i,(name,argv)',start)
code=code[:start]+" jobs=[('changed-receipt-interop',[control.PYTHON,'-B','-u',str(HERE/'source-control05.py'),'--interop-check']),('owned-hostile-frame-support-qualifier',[control.NODE,'--max-old-space-size=256',str(ROOT/'frontend/src/features/parade-ink/world-hud/__tests__/photo2-art-source-v1/qualifier.test.ts')]),('single-producer-decode-probe',[control.PYTHON,'-B','-u',str(HERE/'memory-probe.py')])]"+code[end:]
exec(compile(code,__file__,'exec'))
