"""One changed v3 atlas round; exact ground and original decode probe retained."""
from pathlib import Path
import hashlib
HERE=Path(__file__).resolve().parent
raw=(HERE/'controller.py').read_bytes()
assert hashlib.sha256(raw).hexdigest()=='f1b53f9fa351b1ee9dc571d8f49fe208922ecff05eb8853e7c2ee5eaeb6dfd94'
c=raw.decode().replace('source03','source07').replace('source-control.py','source-control07.py')
c=c.replace('ROOT=control.ROOT;INDEX=control.INDEX;OFFER=control.OFFER',"ROOT=control.ROOT;INDEX=control.INDEX;OFFER=control.OFFER\nassert OFFER=='ART231-PHOTO2-SOURCE07-0734' and control.__file__.endswith('/source-control07.py')")
c=c.replace('started+240;hard=started+300','min(started+240,1793867340);hard=min(started+300,1793867400)')
c=c.replace('remainingNewUpperBytes=64*control.MIB','remainingNewUpperBytes=34103767').replace('historicalWholeFloorBytes=4357560906','historicalWholeFloorBytes=4390566003,retainedFamilyBeforeRoundBytes=475840002')
c=c.replace("assert started<1793862900,'Accept by07:15UTC'","assert started<1793866500,'Accept by08:15UTC'")
c=c.replace('assert control.sha(control.PROJECTION.read_bytes())==control.PROJECTION_SHA','projection,projection_pin=control.current_projection()')
c=c.replace('free>=3355443200+9*control.GIB',"free>=projection['totalConservativeFutureUpperBytes']+9*control.GIB")
c=c.replace("['author.py','controller.py','source-control07.py','memory-probe.py','scoped-tsconfig.json']","['author.py','controller07.py','controller.py','source-control07.py','source-control05.py','source-control04.py','source-control.py','memory-probe.py','scoped-tsconfig.json']")
c=c.replace("ROOT/'frontend/tsconfig.json']","ROOT/'frontend/tsconfig.json',ROOT/'frontend/public/world/reference-scene-v2/photo2-world-display-v1/parade/ground.png',ROOT/'frontend/public/world/reference-scene-v2/photo2-world-display-v1/parade/ground.svg']")
old=",('single-producer-decode-probe',[control.PYTHON,'-B','-u',str(HERE/'memory-probe.py')])"
assert old in c;c=c.replace(old,'')
c=c.replace('ART Photo2 original raster generation, owned source qualifier and one producer decode probe','ART Photo2 v3 changed atlas, scoped types and affected owned qualifier; retained original probe')
exec(compile(c,str(Path(__file__).resolve()),'exec'))
