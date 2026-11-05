"""Changed-art Source04 continuation; Source03 failure and candidate stay preserved."""
from pathlib import Path
import hashlib
raw=Path(__file__).with_name('controller.py').read_bytes()
assert hashlib.sha256(raw).hexdigest()=='f1b53f9fa351b1ee9dc571d8f49fe208922ecff05eb8853e7c2ee5eaeb6dfd94'
code=raw.decode().replace('source03','source04').replace("HERE/'source-control.py'","HERE/'source-control04.py'")
code=code.replace('1793862900','1793863200').replace('Accept by07:15UTC','Accept by07:20UTC')
code=code.replace('remainingNewUpperBytes=64*control.MIB','remainingNewUpperBytes=60884868,priorFiniteSource03PositiveBytes=6223996,priorFinalFreeze4ActualHigherBytes=4625653,entireSourceFix16BookRetained=16777216')
code=code.replace("['author.py','controller.py','source-control.py','memory-probe.py','scoped-tsconfig.json']", "['author.py','controller04.py','controller.py','source-control04.py','source-control.py','memory-probe.py','scoped-tsconfig.json']")
code=code.replace("assert not temp.exists();temp.mkdir()", "assert not temp.exists();temp.mkdir()")
code=code.replace("jobs=[('original-vector-raster'", "jobs=[('changed-receipt-interop',[control.PYTHON,'-B','-u',str(HERE/'source-control04.py'),'--interop-check']),('original-vector-raster'")
code=code.replace("meter=control.Meter(config)", "meter=control.Meter(config)\n # Preserve the actual failed-round candidate before changed-art regeneration.\n saved=HERE/'source03-candidate';assert not saved.exists();saved.mkdir()\n oldfiles=[ROOT/'frontend/public/world/reference-scene-v2/photo2-world-display-v1/parade'/n for n in ['ground.svg','ground.png','atlas.png','atlas-editable.json','asset-interface.json','source-provenance.json','README.md']]+[HERE/n for n in ['full-scene.png','scene-viewport.png','reference-comparison.png','atlas-contact.png','support-diagnostic.png','collision-support.json','build-metadata.json']]\n for p in oldfiles:shutil.copyfile(p,saved/p.name)\n control.atomic(saved/'manifest.json',dict(meaning='Actual Source03 candidate before material visual correction; qualifier/probe were unrun',files=[control.pin(saved/p.name) for p in oldfiles]))")
exec(compile(code,__file__,'exec'))
