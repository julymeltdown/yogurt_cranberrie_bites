"""Current mixed-stage guards; retain immutable prior controls."""
import importlib.util,json,sys,time
from pathlib import Path
HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('photo2_prior_control',HERE/'source-control05.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
assert base.pin(HERE/'source-control05.py') in json.loads((HERE/'frozen-source06-readset.json').read_bytes())['files']
base.OFFER='ART231-PHOTO2-SOURCE07-0734'
def current_projection():
 allocation=json.loads((base.BASE/'resource-allocation-current.json').read_bytes());path=Path(allocation['currentFutureReconciliation']['record']);p=json.loads(path.read_bytes())
 assert p['ordinaryCombinedFutureUpperBytes']==2684354560
 assert p['sourceWindow']['futureTargetBytes']==p['warmTargetMaxFutureUpperBytes']
 assert time.time()<1793867400
 own=p['phaseOwners']['ART231'];assert own['owner']==base.OWNER and base.OFFER in own['offers'] and own['includedPhaseUpperBytes']==64*base.MIB
 return p,base.pin(path)
base.current_projection=current_projection
source=(HERE/'source-control.py').read_text()
assert base.sha(source.encode())=='3e1aed729b2eeed94ff523f9874b1592a7d7e4848d0d4124dccd42098f2f43fe'
c=source[source.index('def census(config):'):source.index('class Meter:')]
c=c.replace("assert 'target' not in kinds,'Compiler/target window withdrawn'","assert kinds.count('target')<=1,'Only one target workflow'")
c=c.replace("actual=kernel(declared['pid'])","declared=decode_declared(declared,row['ownerThreadId'])\n    actual=kernel(declared['pid'])")
c=c.replace("if owned:assert len(owned)<=row['processCap'] and rss<=row['rssCapBytes'],('live actual cap, including FULL roots',row['ownerThreadId'])", """if owned:
    pc=row.get('physicalProcessCap') or row.get('processCap');rc=row.get('physicalRssCapBytes') or row.get('rssCapBytes')
    if row.get('kind')=='ordinary' and pc in (None,0) and rc in (None,0):
     assert not any(any(t in table[p]['command'].split()[0].lower() for t in ('node','cargo','rustc','chrome','postgres')) for p in owned),'Heavy child in ordinary tree'
    else:assert isinstance(pc,int) and isinstance(rc,int) and len(owned)<=pc and rss<=rc,('live actual cap, including FULL roots',row['ownerThreadId'])""")
revision=base.census
exec(compile(c,__file__,'exec'),base.__dict__)
base.census_once=base.census;base.census=revision
m=source[source.index('class Meter:'):source.index('def guardian(config):')]
m=m.replace('max(4357560906,','max(4390566003,').replace('positive<=64*MIB','positive<=34103767')
a=m.index('  assert 221435639');b=m.index('\n',a)
m=m[:a]+'  assert 475840002+positive<=536870912'+m[b:]
exec(compile(m,__file__,'exec'),base.__dict__)
g=source[source.index('def guardian(config):'):source.index('def main():')].replace('source03-independent-full.json','source07-independent-full.json')
exec(compile(g,__file__,'exec'),base.__dict__)
globals().update({name:value for name,value in base.__dict__.items() if not name.startswith('__')})
if __name__=='__main__':sys.exit(main())
