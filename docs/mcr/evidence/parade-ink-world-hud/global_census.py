"""PI08-owned read-only runtime census; the allocator's receipts grant no resources."""
from pathlib import Path
import json, re, shlex, subprocess

DIRECTORY=Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts')
LEGACY=DIRECTORY.parent/'resource-membership-current.json'
OWNER='01a1a5d9-332b-7822-a8f7-40c576fad94a'
PUBLISHER='01a1a5d9-332b-7822-a8f7-412e295fa211'
_cwd_cache={}

def actual_cwd(pid,birth):
    key=(pid,birth)
    if key not in _cwd_cache:
        if len(_cwd_cache)>=64:raise RuntimeError('bounded cwd identity cache full')
        raw=subprocess.check_output(['/usr/sbin/lsof','-a','-p',str(pid),'-d','cwd','-Fn'],text=True,timeout=2)
        names=[line[1:] for line in raw.splitlines() if line.startswith('n')]
        if len(names)!=1:raise RuntimeError('unresolved actual ordinary cwd')
        _cwd_cache[key]=names[0]
    return _cwd_cache[key]

def table():
    raw=subprocess.check_output(['/bin/ps','-axo','pid=,ppid=,rss=,lstart=,command='],text=True,timeout=2)
    result={}
    for line in raw.splitlines():
        parts=line.split(None,8)
        if len(parts)!=9:raise RuntimeError('ambiguous process census row')
        pid,parent,rss=map(int,parts[:3])
        result[pid]={'pid':pid,'parent':parent,'rss':rss*1024,'birth':' '.join(parts[3:8]),'command':parts[8]}
    return result

def descend(rows,pid):
    ids={pid}
    while True:
        expanded=ids|{p for p,r in rows.items() if r['parent'] in ids}
        if expanded==ids:return ids&rows.keys()
        ids=expanded

def reconciled(records,legacy):
    active={r['ownerThreadId']:r for r in records if r['state']=='ACCEPTED'}
    by_owner={r['ownerThreadId']:r for r in records}
    for item in legacy:
        if item.get('state')!='ACCEPTED' and 'ACTUAL_ACCEPT' not in item.get('state',''):continue
        owner=item.get('ownerThreadId',item.get('owner'));record=by_owner.get(owner)
        accepted=item.get('acceptedAtEpoch',item.get('actualAccept'))
        matches=record and ((bool(item.get('offerId')) and record['offerId']==item['offerId']) or record['offerId'] in item.get('knownRootMatchers',[]) or
            (isinstance(accepted,(int,float)) and isinstance(record.get('acceptedAtEpoch'),(int,float)) and record['acceptedAtEpoch']>=accepted-1))
        if not matches:raise RuntimeError('unresolved accepted legacy identity: '+str(owner))
    return list(active.values())

def evaluate(records,legacy,rows,observations=()):
    active=reconciled(records,legacy)
    ordinary=set();ordinary_future=0;ordinary_roles=[]
    # Observation-only metadata never participates in ACCEPT/FULL reconciliation.
    for observation in observations:
        booking=observation.get('booking',{})
        upper=booking.get('additionalUpperBytes')
        if type(upper) is not int or upper<0:raise RuntimeError('invalid observation byte booking')
        ordinary_future+=upper
    if ordinary_future>int(2.5*1024**3):raise RuntimeError('ordinary future ceiling')
    execution_records=[]
    for record in active:
        if record['kind']!='ordinary':execution_records.append(record);continue
        # Ordinary preparation bookings reserve bytes, never an execution workflow.
        future=record['remainingNewUpperBytes']
        if record.get('executionWorkflow') is True or record.get('processCap') is not None or record.get('rssCapBytes') is not None or type(future) is not int or future<0:
            raise RuntimeError('invalid ordinary preparation booking')
        ordinary_future+=future
        if ordinary_future>int(2.5*1024**3):raise RuntimeError('ordinary future ceiling')
        control=record.get('controller')
        if control is None:continue
        row=rows.get(control['pid'])
        if row is None:continue
        # A stale static observation is not a live execution identity or a release.
        if not control.get('birth') or row['birth']!=' '.join(control['birth'].split()):continue
        ids=descend(rows,control['pid'])
        for pid in ids:
            executable=rows[pid]['command'].split()[0].rsplit('/',1)[-1]
            if executable in ('node','cargo','rustc') or rows[pid]['command'].startswith('/Applications/Google Chrome.app/'):
                raise RuntimeError('execution child under ordinary publisher role')
        # Resolve only the explicitly known publisher execution-looking wrapper.
        # Other ordinary records do not exempt unknown execution-looking roots.
        script=control.get('script')
        if record['ownerThreadId']==PUBLISHER and script:
            arguments=shlex.split(row['command'])
            if script not in arguments:
                raise RuntimeError('ordinary publisher identity changed')
            if control.get('executable') and Path(control['executable']).resolve()!=Path(arguments[0]).resolve():
                raise RuntimeError('ordinary publisher actual executable changed')
            cwd=row.get('cwd') or actual_cwd(control['pid'],row['birth'])
            expected_cwd=control.get('cwd',control.get('workingDirectory'))
            if expected_cwd and Path(expected_cwd).resolve()!=Path(cwd).resolve():raise RuntimeError('ordinary publisher actual cwd changed')
            ordinary_roles.append({'owner':PUBLISHER,'pid':control['pid'],'birth':row['birth'],'script':script,'actualCwd':cwd})
            ordinary|=ids
    active=execution_records
    kinds=[r['kind'] for r in active]
    if len(active)>4 or sum(k in ('source','copied-native') for k in kinds)>3 or kinds.count('browser')>2 or kinds.count('target')>1:
        raise RuntimeError('global workflow class cap')
    reserved_pids=sum(r['processCap'] for r in active);reserved_rss=sum(r['rssCapBytes'] for r in active)
    if reserved_pids>96 or reserved_rss>18*1024**3:raise RuntimeError('global declared reservation cap')
    union=set();actual_rss=0;absent_pids=absent_rss=0;cohorts=[]
    for record in active:
        control=record.get('controller');pid=control.get('pid') if control else None;row=rows.get(pid)
        if row is None:
            absent_pids+=record['processCap'];absent_rss+=record['rssCapBytes']
            cohorts.append({'owner':record['ownerThreadId'],'visible':False});continue
        if row['birth']!=' '.join(control['birth'].split()):raise RuntimeError('controller PID birth changed')
        script=Path(control['script'])
        arguments=shlex.split(row['command'])
        matches=control['script'] in arguments
        if not matches and control.get('workingDirectory'):
            matches=any(not token.startswith('-') and (Path(control['workingDirectory'])/token).resolve()==script for token in arguments[1:])
        if not matches:raise RuntimeError('controller script identity changed')
        ids=descend(rows,pid)
        if union&ids:raise RuntimeError('overlapping workflow roots')
        rss=sum(rows[p]['rss'] for p in ids)
        if len(ids)>record['processCap'] or rss>record['rssCapBytes']:raise RuntimeError('peer actual tree exceeds declared cap')
        union|=ids;actual_rss+=rss
        cohorts.append({'owner':record['ownerThreadId'],'visible':True,'pid':pid,'pids':len(ids),'rss':rss})
    # Only execution roots in the shared MCR workflows; unrelated desktop apps are outside this allocator.
    execution=re.compile(r'(?:\.mjs|vitest|vite|tsc|oxlint|controller\.py|guard\.py|guardian\.py|lease\.py|primary\.py|--user-data-dir=)')
    for pid,row in rows.items():
        command=row['command']
        executable=command.split()[0].rsplit('/',1)[-1]
        actual_tool=bool(re.fullmatch(r'node|python[0-9.]*|cargo|rustc',executable,re.IGNORECASE)) or command.startswith('/Applications/Google Chrome.app/')
        if actual_tool and ('/mcr' in command or 'mcr-' in command) and execution.search(command) and pid not in union and pid not in ordinary:
            raise RuntimeError('unknown live workflow root: '+str(pid))
    if len(union)+absent_pids>96 or actual_rss+absent_rss>18*1024**3:raise RuntimeError('global actual plus invisible reservations cap')
    return {'workflows':len(active),'processes':len(union),'rss':actual_rss,'reservedPIDs':reserved_pids,'reservedRSS':reserved_rss,'absentReservedPIDs':absent_pids,'ordinaryFutureUpperBytes':ordinary_future,'ordinaryRoles':ordinary_roles,'cohorts':cohorts,'processIdentities':[rows[p] for p in sorted(union)]}

def sample():
    paths=sorted(DIRECTORY.glob('*.json'))
    if len(paths)>64:raise RuntimeError('unbounded owner receipt directory')
    records=[];observations=[]
    for path in paths:
        if path.stat().st_size>1024**2:raise RuntimeError('unbounded owner receipt')
        record=json.loads(path.read_text())
        if set(record)=={'ordinaryObservation'}:
            observations.append(record['ordinaryObservation']);continue
        if path.stem!=record['ownerThreadId'] or record['state'] not in ('ACCEPTED','FULL','DECLINED'):raise RuntimeError('invalid owner receipt identity/state')
        records.append(record)
    return evaluate(records,json.loads(LEGACY.read_text())['entries'],table(),observations)

def selftest():
    def record(owner='one',pid=10,kind='source',state='ACCEPTED'):
        return {'ownerThreadId':owner,'offerId':'offer-'+owner,'state':state,'acceptedAtEpoch':100,'kind':kind,'processCap':24,'rssCapBytes':4*1024**3,'controller':{'pid':pid,'birth':'Thu Nov 5 09:00:00 2026','script':'/mcr/guard.py','workingDirectory':'/mcr'}}
    def row(pid,parent=1):return {'pid':pid,'parent':parent,'rss':100,'birth':'Thu Nov 5 09:00:00 2026','command':'python3 /mcr/guard.py','cwd':'/mcr'}
    rows={10:row(10),11:row(11,10)}
    assert evaluate([record()],[],rows)['processes']==2
    assert evaluate([record()],[],{})['absentReservedPIDs']==24
    def rejects(records,legacy,rows):
        try:evaluate(records,legacy,rows)
        except RuntimeError:return
        raise AssertionError('census failed to close')
    rejects([record(),record('two',11)],[],rows)
    wrong={10:{**row(10),'birth':'different'}};rejects([record()],[],wrong)
    rejects([],[],rows)
    rejects([], [{'owner':'one','state':'ACTUAL_ACCEPT','actualAccept':101,'knownRootMatchers':['new-offer']}],{})
    rejects([record(state='FULL')],[{'owner':'one','state':'ACTUAL_ACCEPT','actualAccept':105,'knownRootMatchers':['new-offer']}],{})
    rejects([],[{'owner':'one','state':'ACCEPTED','actualAccept':105,'knownRootMatchers':['new-offer']}],{})
    rejects([record(str(i),i,'source') for i in range(4)],[],{})
    assert evaluate([record(state='FULL')],[],{})['workflows']==0
    assert evaluate([record()],[],{10:{**row(10),'command':'python3 guard.py'}})['processes']==1
    publisher=record(PUBLISHER,30,'ordinary');publisher.update(executionWorkflow=False,processCap=None,rssCapBytes=None,remainingNewUpperBytes=128*1024**2)
    script='/private/tmp/mcr-publisher-412e-capacity-composition-delivery.py';publisher['controller']['script']=script
    ordinary_row={30:{**row(30),'command':'python3 '+script}}
    assert evaluate([publisher],[],ordinary_row)['workflows']==0
    rejects([publisher],[],{**ordinary_row,31:{**row(31,30),'command':'node /mcr/native.mjs'}})
    assert evaluate([record(state='FULL')],[{'ownerThreadId':'one','offerId':'offer-one','state':'ACCEPTED','acceptedAtEpoch':100}],{})['workflows']==0
    intake='/private/tmp/mcr-publisher-412e-fe37-member-content-intake.py'
    publisher['controller']['script']=intake
    assert evaluate([publisher],[],{30:{**row(30),'command':'python3 '+intake}})['workflows']==0
    publisher['controller']['script']='/private/tmp/unresolved.py'
    rejects([publisher],[],{30:{**row(30),'command':'python3 /private/tmp/different-current.py'}})
    prep=record('prep',40,'ordinary');prep.update(processCap=None,rssCapBytes=None,remainingNewUpperBytes=128*1024**2);prep['controller']['script']=None
    assert evaluate([prep],[],{40:{**row(40),'command':'python3 -'}})['ordinaryFutureUpperBytes']==128*1024**2
    rejects([prep],[],{40:{**row(40),'command':'python3 -'},41:{**row(41,40),'command':'node /mcr/native.mjs'}})
    prep['remainingNewUpperBytes']=3*1024**3;rejects([prep],[],{})
    observation={'booking':{'additionalUpperBytes':64*1024**2}}
    assert evaluate([],[],{},[observation])['ordinaryFutureUpperBytes']==64*1024**2
    for legacy,observed_rows,obs in [([{'owner':'one','state':'ACTUAL_ACCEPT','actualAccept':100}],{},[observation]),([],rows,[observation]),([],{},[{'booking':{'additionalUpperBytes':3*1024**3}}])]:
        try:evaluate([],legacy,observed_rows,obs)
        except RuntimeError:continue
        raise AssertionError('observation erased reservation or exempted execution')
    prep['remainingNewUpperBytes']=16*1024**2;prep['controller']=None
    assert evaluate([prep],[],{})['ordinaryFutureUpperBytes']==16*1024**2
    pending=record('pending');pending['controller']=None
    assert evaluate([pending],[],{})['absentReservedPIDs']==24
    rejects([],[],{10:{**row(10),'command':'/MacOS/Python /mcr/guard.py'}})
    return 'PASS twenty-six bounded census cases'
