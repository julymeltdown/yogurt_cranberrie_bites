"""Read-only inode accounting and serialized process sampling for both controls."""
from pathlib import Path
import fcntl
import os
import subprocess

MIB=1024**2
def scan(roots):
    result={}
    for root in roots:
        pending=[Path(root)]
        while pending:
            path=pending.pop()
            try:st=path.lstat()
            except FileNotFoundError:continue
            if path.is_symlink():
                key=(st.st_dev,st.st_ino,getattr(st,'st_birthtime_ns',int(getattr(st,'st_birthtime',0)*10**9)))
                result[key]=st.st_size
            elif path.is_dir():
                try:pending.extend(Path(e.path) for e in os.scandir(path))
                except FileNotFoundError:continue
            elif path.is_file():
                key=(st.st_dev,st.st_ino,getattr(st,'st_birthtime_ns',int(getattr(st,'st_birthtime',0)*10**9)))
                result[key]=st.st_size
    return result

class Inventory:
    def __init__(self,roots,prior):
        self.roots=roots;self.baseline=scan(roots);self.seen={}
        self.logical=sum(self.baseline.values());self.carry=max(prior,self.logical+34*MIB)
    def sample(self):
        current=scan(self.roots)
        for key,size in current.items():
            gain=max(0,size-self.baseline.get(key,0))
            if gain:self.seen[key]=max(self.seen.get(key,0),gain)
        positive=sum(self.seen.values())
        return self.carry+positive,34*MIB+positive

# Developer-owned executable cohort census. Metadata discovers accepted owners;
# actual birth/ancestry joins their controllers and separate guard groups.
import json
import shutil
import shlex
RECEIPTS=Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts')
OWNER='01a1a231-0dc4-7b50-92bd-c0f0f56bd970'

def normalize_birth(value):return ' '.join((value or '').split())
def current_records():
    metadata=json.loads((RECEIPTS.parent/'resource-membership-current.json').read_text())
    result={r.get('ownerThreadId',r.get('owner')):r for r in metadata['entries']}
    for p in RECEIPTS.glob('*.json'):
        r=json.loads(p.read_text());owner=r['ownerThreadId'];old=result.get(owner,{})
        if float(r.get('acceptedAtEpoch') or 0)>=float(old.get('acceptedAtEpoch',old.get('actualAccept')) or 0):result[owner]=r
    return result

def descendants(table,roots):
    owned=set(roots)&table.keys()
    while True:
        children={pid for pid,row in table.items() if row['parent'] in owned}
        if children<=owned:return owned
        owned|=children

def controller_matches(row,controller,verified=None):
    assert normalize_birth(controller.get('birth'))==normalize_birth(row['birth']),('Controller PID birth mismatch',controller.get('pid'),row)
    # Zombies lack argv but retain ancestry and the owner's full reservation.
    # Only a literal FULL retires the accepted reservation.
    if row.get('state','').startswith('Z'):return True
    key=(controller.get('pid'),normalize_birth(row['birth']),controller.get('script'),controller.get('executable'))
    if row['command'] in ('(Python)','<defunct>'):
        prior=verified.get(key) if verified is not None else None
        assert prior and prior['parent']==row['parent'],('Unavailable argv lacks previously verified exact identity and ancestry',controller.get('pid'),row)
        return True
    argv=shlex.split(row['command'])
    executable=controller.get('executable')
    assert argv and (not executable or Path(argv[0]).resolve()==Path(executable).resolve()),('Controller executable mismatch',controller.get('pid'),row)
    script=controller.get('script')
    if not script:return True
    def accept():
        if verified is not None:verified[key]=dict(parent=row['parent'],actualComm=str(Path(argv[0]).resolve()),expectedScript=script)
        return True
    expected=Path(script).resolve()
    for arg in argv[1:]:
        if arg.startswith('-'):continue
        candidate=Path(arg)
        if candidate.is_absolute() and candidate.resolve()==expected:return accept()
        if not candidate.is_absolute() and row.get('actualCwd'):
            actual=Path(row['actualCwd']);declared=controller.get('cwd')
            if declared and actual.resolve()!=Path(declared).resolve():continue
            if (actual/candidate).resolve()==expected:return accept()
    raise AssertionError(('Controller script mismatch',controller.get('pid'),dict(script=script,declaredCwd=controller.get('cwd'),**row)))

def qualification_deadlines(accepted,work,hard):
    return min(work,accepted+180),min(hard,accepted+240)

def cohort(table,records,own,verified=None):
    active=[r for r in records.values() if r.get('state')=='ACCEPTED' and r.get('kind')!='ordinary']
    kinds=[r['kind'] for r in active]
    assert len(active)<=4 and kinds.count('source')<=3 and kinds.count('browser')+kinds.count('copied-native')<=2 and kinds.count('target')<=1,'Global workflow class ceiling'
    assert sum(r['processCap'] for r in active)<=96 and sum(r['rssCapBytes'] for r in active)<=18*1024**3,'Global reserved envelope'
    classified=set(own);rows=[]
    # Ordinary controllers identify bounded source/publication helpers, without
    # consuming an app/test workflow slot or erasing accepted executions.
    for r in records.values():
        if r.get('state')!='ACCEPTED':continue
        roots=[]
        for controller in [r.get('controller')]+r.get('guardControllers',[]):
            if not controller or not controller.get('pid'):continue
            pid=controller['pid']
            if pid in table:
                controller_matches(table[pid],controller,verified)
                roots.append(pid)
        actual=descendants(table,roots);classified|=actual
        if r.get('kind')!='ordinary':
            assert len(actual)<=r['processCap'] and sum(table[p]['rss'] for p in actual)<=r['rssCapBytes'],('Peer live cap',r['ownerThreadId'])
        rows.append(dict(owner=r['ownerThreadId'],offer=r['offerId'],kind=r['kind'],pids=sorted(actual),visible=bool(actual)))
    unknown=[]
    for pid,r in table.items():
        if pid in classified:continue
        command=r['command'];exe=Path(command.split()[0]).name.lower() if command else ''
        relevant=any(token in exe for token in ('python','node','cargo','rustc','chrome','chromium','postgres','playwright'))
        if relevant and any(token in command.lower() for token in ('/mcr-','/mcr/','mcr-worktrees','mcr-evidence')):unknown.append(dict(pid=pid,birth=r['birth'],command=command[:400]))
    assert not unknown,('Unknown actual MCR workload root',unknown)
    other=sum(r.get('remainingNewUpperBytes',0) for r in active if r.get('ownerThreadId')!=OWNER)
    return dict(workflows=len(active),source=kinds.count('source'),browser=kinds.count('browser')+kinds.count('copied-native'),target=kinds.count('target'),
                reservedProcesses=sum(r['processCap'] for r in active),reservedRSS=sum(r['rssCapBytes'] for r in active),
                otherRemainingUpper=other+2684354560,owners=rows,unknown=unknown)

# Replace the old comm-only sampler with one serialized full process census.
_verified_controllers={}
def process_snapshot(supervisor,lock):
    with Path(lock).open('a') as handle:
        fcntl.flock(handle,fcntl.LOCK_EX)
        sampler=subprocess.Popen(['ps','-ww','-axo','pid=,ppid=,rss=,lstart=,state=,command='],stdout=subprocess.PIPE,text=True)
        raw=sampler.communicate()[0];table={}
        for line in raw.splitlines():
            p=line.split(None,9)
            if len(p)==10:table[int(p[0])]=dict(parent=int(p[1]),rss=int(p[2])*1024,birth=' '.join(p[3:8]),state=p[8],command=p[9])
        records=current_records()
        for record in records.values():
            if record.get('state')!='ACCEPTED':continue
            for controller in [record.get('controller')]+record.get('guardControllers',[]):
                if not controller or controller.get('pid') not in table:continue
                row=table[controller['pid']];script=controller.get('script')
                if script and script not in row['command'] and not row['state'].startswith('Z'):
                    # Observe the actual cwd; never infer it from the checkout.
                    found=subprocess.run(['/usr/sbin/lsof','-a','-p',str(controller['pid']),'-d','cwd','-Fn'],capture_output=True,text=True)
                    paths=[line[1:] for line in found.stdout.splitlines() if line.startswith('n/')]
                    if len(paths)==1:row['actualCwd']=paths[0]
        owned=descendants(table,[supervisor]);assert supervisor in owned,'Supervisor disappeared'
        parent=table[supervisor]['parent']
        if parent in table and Path(table[parent]['command'].split()[0]).name in ('zsh','bash','sh'):owned.add(parent)
        for key in list(_verified_controllers):
            if key[0] not in table or normalize_birth(table[key[0]]['birth'])!=key[1]:del _verified_controllers[key]
        global_state=cohort(table,records,owned,_verified_controllers)
        return dict(processes=len(owned),rss=sum(table[p]['rss'] for p in owned)+16*MIB,pids=sorted(owned),
                    globalCensus=global_state,birth=table[supervisor]['birth'],executable=table[supervisor]['command'].split()[0],
                    controllerIdentities={str(p):dict(pid=p,birth=table[p]['birth'],executable=table[p]['command'].split()[0]) for p in owned})
