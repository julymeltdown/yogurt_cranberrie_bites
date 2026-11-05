"""Owned receipt publication and conservative global cohort discovery."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import time
BASE=Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d')
OWNER='01a1a5d9-6954-7bd3-9b08-7109883187b5'
RECEIPT=BASE/'owner-resource-receipts'/(OWNER+'.json')
GiB=1024**3
# Fresh exact desktop classification: actual OS comm, parent, process group,
# birth and absolute programs observed at02:55Z. Both program byte hashes equal
# the corresponding already-verified old desktop pair. No cwd-based exemption.
EXACT_DESKTOP_OBSERVATIONS=[{'pids':[25092,25093],'ppid':44380,'pgid':44264,
    'birth':'Thu Nov 5 11:48:17 2026','executable':'/Applications/ChatGPT.app/Contents/Resources/cua_node/bin/node',
    'actualPrograms':['/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/.tmpOq0uBb/kernel.js','/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/.tmpOq0uBb/trusted-worker.js'],
    'referencePrograms':['/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/.tmpCCbUfQ/kernel.js','/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/.tmpCCbUfQ/trusted-worker.js'],
    'programSha256':['d8cc1d07dab419b63afab8c694c43c2e73b4f0393bc50f3ec0bb85d049397e3a','79e4b531950976c1924a43be245cdc38066a611876b3a27f2d5ff097fb76a2d8']}]

class OrdinaryImageMismatch(AssertionError):
    def __init__(self,claim,controller,actual):
        self.pid=controller['pid'];self.expected_birth=controller['birth']
        super().__init__('Ordinary actual executable mismatch: '+json.dumps({'owner':claim['ownerThreadId'],'offer':claim['offerId'],'pid':self.pid,'birth':self.expected_birth,'expectedImage':controller['executable'],'actualImage':actual}))

class ProcessRows(list):
    """Ancestry, RSS, command and birth captured together, without truncation."""
    def __init__(self, values, births):
        super().__init__(values);self.births=births

def processes():
    raw=subprocess.check_output(['ps','-ww','-axo','pid=,ppid=,rss=,lstart=,command='],text=True)
    values=[];births={}
    for line in raw.splitlines():
        fields=line.strip().split(None,8)
        if len(fields)!=9:continue
        pid,parent,rss=int(fields[0]),int(fields[1]),int(fields[2])*1024
        values.append((pid,parent,rss,fields[8]));births[pid]=' '.join(fields[3:8])
    return ProcessRows(values,births)

def birth(pid):
    value=subprocess.run(['ps','-p',str(pid),'-o','lstart='],capture_output=True,text=True)
    return value.stdout.strip() if value.returncode==0 else None

def epoch(value):
    if isinstance(value,(int,float)):return float(value)
    if isinstance(value,str):
        try:return datetime.datetime.fromisoformat(value.replace('Z','+00:00')).timestamp()
        except ValueError:pass
    return None

def identity(pid,script,output=None):
    actual=subprocess.check_output(['ps','-p',str(pid),'-o','comm='],text=True).strip()
    return {'pid':pid,'birth':birth(pid),'executable':actual,'script':str(script),'workingDirectory':os.getcwd(),'outputRoot':str(output) if output else None}

def publish(config,state,script,durable,watcher=None,result=None):
    durable=Path(durable)
    data={'ownerThreadId':OWNER,'offerId':config['job'],'state':state,'kind':config['kind'],
          'acceptedAtEpoch':config['accepted'],'workDeadlineEpoch':config['workDeadline'],'fullDeadlineEpoch':config['fullDeadline'],
          'processCap':config['processCap'],'rssCapBytes':config['rssCap'],'remainingNewUpperBytes':max(0,config['newCap']-config.get('carriedNew',240*1024**2)) if state=='ACCEPTED' else 0,
          'controller':identity(config['controller'],script,durable),'guardControllers':[identity(watcher,script)] if watcher else [],
          'receiptPaths':{'accept':str(durable/'ACCEPT.json'),'full':str(durable/'FULL.json')},'observedAtEpoch':time.time()}
    closed=[]
    for proof in sorted((durable.parent).glob('*/FULL*.json')):
        old=json.loads(proof.read_text()); old_config=old.get('config',{})
        if old.get('FULL') is True and old.get('temporaryAbsent') is True and old.get('remainingOwned')==[]:
            closed.append({'ownerThreadId':OWNER,'offerId':old['job'],'acceptedAtEpoch':old_config['accepted'],'actualFullEpoch':old['finished'],'resourcesReleased':True,'fullPath':str(proof),'fullSha256':hashlib.sha256(proof.read_bytes()).hexdigest()})
    data['closedOffers']=closed
    if result:data['fullAtEpoch']=result['finished']
    if state=='FULL':
        command=subprocess.check_output(['ps','-ww','-p',str(os.getpid()),'-o','command='],text=True).strip()
        data['ordinaryObservation']={**data['controller'],'actualpid':os.getpid(),'cwd':os.getcwd(),'role':'ordinary final receipt/proof manifest publication and controller exit after all workload children ended','commandDigest':hashlib.sha256(command.encode()).hexdigest(),'observedAt':time.time()}
    temp=RECEIPT.with_suffix('.json.tmp-'+str(os.getpid()))
    temp.write_text(json.dumps(data,indent=2)+'\n');os.replace(temp,RECEIPT)

def ordinary_preflight(script, ending=None, observation=None):
    """Record factual preparation without replacing an execution booking."""
    record=json.loads(RECEIPT.read_text()) if RECEIPT.exists() else {}
    if observation is None:
        pid=os.getpid();command=subprocess.check_output(['ps','-ww','-p',str(pid),'-o','command='],text=True).strip()
        observation={'actualpid':pid,'birth':birth(pid),'cwd':os.getcwd(),'role':'ordinary read-only complete allocation and pin preflight; no application/test/compiler/browser','observedAt':time.time(),'commandDigest':hashlib.sha256(command.encode()).hexdigest(),'booking':'WALL-FINALDOC1-0233 extended through04:00Z without new bytes','executable':subprocess.check_output(['ps','-p',str(pid),'-o','comm='],text=True).strip(),'script':str(script)}
    if ending is not None:observation={**observation,'endedAt':time.time(),'exit':ending}
    record['ordinaryObservation']=observation
    temporary=RECEIPT.with_suffix('.json.tmp-'+str(os.getpid()))
    temporary.write_text(json.dumps(record,indent=2)+'\n');os.replace(temporary,RECEIPT)
    return observation

def relative_primary_matches(controller,command):
    """Resolve only the actual primary Python program, never an argument marker."""
    argv=shlex.split(command)
    if not argv or not Path(argv[0]).name.lower().startswith('python'):return False
    program=None
    for value in argv[1:]:
        if value in ['-c','-m']:return False
        if value.startswith('-'):continue
        program=value;break
    declared=controller.get('script')
    if not program or Path(program).is_absolute() or not program.endswith('.py') or not declared or not Path(declared).is_absolute():return False
    pid=controller['pid']
    actual_executable=subprocess.check_output(['ps','-p',str(pid),'-o','comm='],text=True).strip()
    if not controller.get('executable') or actual_executable!=controller['executable']:return False
    cwd_output=subprocess.check_output(['lsof','-a','-p',str(pid),'-d','cwd','-Fn'],text=True)
    directories=[line[1:] for line in cwd_output.splitlines() if line.startswith('n')]
    if len(directories)!=1 or not Path(directories[0]).is_absolute():return False
    return (Path(directories[0])/program).resolve()==Path(declared).resolve()

def descendants(rows,pid):
    found={pid}
    for _ in range(40):
        more=found|{p for p,parent,_,_ in rows if parent in found}
        if more==found:return found
        found=more
    raise RuntimeError('Incomplete global ancestry closure')

def same_offer(legacy,receipt):
    offer=legacy.get('offerId')
    if offer is not None and offer!=receipt['offerId']:return False
    at=epoch(legacy.get('acceptedAtEpoch',legacy.get('actualAccept')))
    if at is not None:return abs(at-receipt['acceptedAtEpoch'])<.001
    old=re.findall(r'(?:source|native|compile|round)\d+',legacy.get('cohort','').lower())
    new=re.findall(r'(?:source|native|compile|round)\d+',receipt['offerId'].lower())
    return bool(set(old)&set(new))

def census(config,rows):
    for attempt in range(3):
        try:return _census(config,rows)
        except OrdinaryImageMismatch as error:
            if attempt==2:raise
            assert time.time()+.1<config['workDeadline'],'Identity reconciliation deadline'
            time.sleep(.1);fresh=processes()
            current_birth=fresh.births.get(error.pid)
            if error.pid in {row[0] for row in fresh} and (not current_birth or ' '.join(current_birth.split())==' '.join(error.expected_birth.split())):raise
            # Never return a partial census or exempt a live mismatching image.
            # An observed departure restarts every receipt/root/cap check.
            rows=fresh
        except AssertionError as error:
            initial=config.get('initialIdentityReconciliation') and config.get('censusRoot') and not (Path(config['censusRoot'])/'payload.started').exists()
            if not (initial and attempt<2 and str(error).startswith('Unresolved live workflow roots: ')):raise
            assert time.time()+.1<config['workDeadline'],'Initial identity reconciliation deadline'
            # Before any payload only: re-read the complete current identity
            # census after at most two100ms metadata intervals. A persistent
            # unknown still fails. The payload marker disables this path.
            time.sleep(.1);rows=processes()
        except subprocess.CalledProcessError as error:
            command=error.cmd
            if not (isinstance(command,list) and len(command)==5 and command[:2]==['ps','-p'] and command[2].isdigit() and command[3]=='-o' and command[4] in ['comm=','pgid=']):raise
            pid=int(command[2]);fresh=processes()
            old_birth=getattr(rows,'births',{}).get(pid)
            fresh_birth=fresh.births.get(pid)
            # Retry the complete receipt/claim/root census only after a proved
            # disappearance or changed birth. A present unchanged PID's error
            # remains a failure. No partial sample or booking release survives.
            if pid in {row[0] for row in fresh} and fresh_birth==old_birth:raise
            if attempt==2:raise
            rows=fresh

def _census(config,rows):
    interface=json.loads((BASE/'resource-receipt-interface.json').read_text())
    directory=Path(interface['directory']);files=sorted(directory.glob('*.json'))
    assert len(files)<=128,'Unbounded receipt directory'
    receipts={};observation_claims=[];snapshot_pids={row[0] for row in rows}
    for path in files:
        assert path.stat().st_size<=64*1024,'Oversized owner receipt'
        data=json.loads(path.read_text())
        observation=data.get('ordinaryObservation')
        if observation:
            observation={**observation,'actualpid':observation.get('actualpid',observation.get('pid'))}
            ended=any(observation.get(key) is not None for key in ['endedAt','completedAt','commandExit'])
            if not ended and (not observation.get('actualpid') or observation['actualpid'] in snapshot_pids):
                assert observation.get('actualpid') and observation.get('birth') and observation.get('role') and observation.get('cwd'),'Incomplete live ordinary observation: '+path.name
                booking=observation.get('booking')
                booking_id=booking.get('id','ORDINARY_OBSERVATION') if isinstance(booking,dict) else booking if isinstance(booking,str) else 'ORDINARY_OBSERVATION'
                same_controller=(data.get('controller') or {}).get('pid')==observation['actualpid']
                controller_already_claimed=same_controller and (data.get('kind')=='ordinary' or data.get('executionWorkflow') is False or data.get('state') in ['ACCEPTED','ACTUAL_ACCEPT'])
                if not controller_already_claimed:
                    observation_claims.append({'ownerThreadId':path.stem,'offerId':booking_id,'kind':'ordinary','executionWorkflow':False,'commandDigest':observation.get('commandDigest'),'controller':{'pid':observation['actualpid'],'birth':observation['birth'],'executable':observation.get('executable'),'script':None,'outputRoot':None}})
        if 'state' not in data:
            assert set(data)=={'ordinaryObservation'},'Unknown observation-only schema: '+path.name
            continue
        assert path.stem==data['ownerThreadId'],'Owner receipt mismatch'
        if data['state']=='ACTUAL_ACCEPT':
            data={**data,'rawReceiptState':'ACTUAL_ACCEPT','state':'ACCEPTED'}
        assert data['state'] in ['ACCEPTED','FULL','DECLINED'],'Unknown receipt state: '+path.name+': '+str(data['state'])
        receipts[data['ownerThreadId']]=data
    own=receipts.get(OWNER)
    assert own and own['state']=='ACCEPTED' and own['offerId']==config['job'],'Own actual ACCEPT receipt required'
    membership=json.loads((BASE/'resource-membership-current.json').read_text())
    claims={}
    for entry in membership['entries']:
        state=entry['state'].upper()
        if entry.get('kind')=='ordinary':continue
        if state in ['FULL','DECLINED'] or any(word in state for word in ['RELEASED','DECLINED','FULL_RECEIVED']):continue
        owner=entry.get('ownerThreadId',entry.get('owner'));assert owner,'Missing legacy owner';record=receipts.get(owner)
        if record:
            if not same_offer(entry,record):
                closures=[*record.get('closedOffers',[]),*record.get('closedReceipts',[]),*record.get('closedHistory',[])]
                matches=set()
                for closed in closures:
                    if closed.get('ownerThreadId',owner)!=owner or closed.get('offerId')!=entry.get('offerId',entry.get('cohort')):continue
                    proof=Path(closed['fullPath']);assert proof.stat().st_size<=128*1024,'Oversized closed proof'
                    sha=closed.get('fullSha256',closed.get('fullSHA',closed.get('sha256')))
                    assert sha and hashlib.sha256(proof.read_bytes()).hexdigest()==sha,'Unverified exact closed booking'
                    original=json.loads(proof.read_text())
                    accepted=closed.get('acceptedAtEpoch',original.get('acceptedAtEpoch',original.get('acceptedEpoch',original.get('config',{}).get('accepted'))))
                    finished=closed.get('actualFullEpoch')
                    original_finished=original.get('actualFullEpoch',original.get('finished'))
                    original_offer=original.get('offerId',original.get('job'))
                    resources=original.get('resources')
                    released=closed.get('resourcesReleased') is True or (isinstance(resources,dict) and bool(resources) and all(value==0 for value in resources.values()))
                    full=original.get('FULL') is True or original.get('state')=='FULL'
                    normalized={'offerId':closed['offerId'],'acceptedAtEpoch':accepted}
                    if accepted is not None and finished is not None and original_offer==closed['offerId'] and full and released and finished==original_finished and finished>=accepted and same_offer(entry,normalized):matches.add((closed['offerId'],accepted,finished,str(proof)))
                assert len(matches)==1,'Unresolved legacy booking for '+owner
                continue
            if record['state']!='ACCEPTED':continue
            claims[owner]=record
        else:
            kind=entry.get('kind')
            assert kind in ['source','browser','target','copied-native'],'Unknown legacy kind'
            claims[owner]={'ownerThreadId':owner,'offerId':entry.get('offerId',entry.get('cohort')),'kind':kind,'processCap':entry['processCap'],
                           'rssCapBytes':entry['rssCapBytes'] if 'rssCapBytes' in entry else int(entry['rssCapGiB']*GiB),'legacyMatchers':entry.get('knownRootMatchers',[])}
    for owner,record in receipts.items():
        if record['state']=='ACCEPTED' and record.get('kind')!='ordinary' and record.get('executionWorkflow') is not False:claims[owner]=record
    count={kind:sum(c['kind']==kind for c in claims.values()) for kind in ['source','browser','target','copied-native']}
    category_diagnostic=json.dumps({'categories':count,'claims':[{'owner':c['ownerThreadId'],'offer':c['offerId'],'kind':c['kind']} for c in claims.values()]})
    assert len(claims)<=4 and count['source']+count['copied-native']<=3 and count['browser']<=2 and count['target']<=1,'Global workflow category cap: '+category_diagnostic
    assert sum(c['processCap'] for c in claims.values())<=96,'Global reserved PID caps'
    assert sum(c['rssCapBytes'] for c in claims.values())<=18*GiB,'Global reserved RSS caps'
    by_pid={r[0]:r for r in rows};covered=set();observed=[]
    ordinary=[r for r in receipts.values() if r.get('kind')=='ordinary' or r.get('executionWorkflow') is False]
    for claim in [*claims.values(),*ordinary,*observation_claims]:
        ids=set();controller=claim.get('controller')
        if controller and controller['pid'] in by_pid:
            actual=by_pid[controller['pid']];actual_birth=rows.births.get(controller['pid']) if hasattr(rows,'births') else birth(controller['pid'])
            ordinary_claim=claim.get('kind')=='ordinary' or claim.get('executionWorkflow') is False
            if ordinary_claim and (not actual_birth or not controller.get('birth') or ' '.join(actual_birth.split())!=' '.join(controller['birth'].split())):continue
            assert actual_birth and controller['birth'] and ' '.join(actual_birth.split())==' '.join(controller['birth'].split()),'Controller PID birth mismatch: '+json.dumps({'owner':claim['ownerThreadId'],'pid':controller['pid'],'expectedBirth':controller.get('birth'),'actualBirth':actual_birth,'commandSHA256':hashlib.sha256(actual[3].encode()).hexdigest()})
            markers=[value for value in [controller.get('script'),controller.get('outputRoot')] if isinstance(value,str) and value]
            ordinary_claim=claim.get('kind')=='ordinary' or claim.get('executionWorkflow') is False
            if ordinary_claim:
                actual_executable=subprocess.check_output(['ps','-p',str(controller['pid']),'-o','comm='],text=True).strip()
                if controller.get('executable'):
                    if actual_executable!=controller['executable']:raise OrdinaryImageMismatch(claim,controller,actual_executable)
                else:
                    assert claim.get('commandDigest') and hashlib.sha256(actual[3].encode()).hexdigest()==claim['commandDigest'],'Ordinary observed command digest mismatch'
            else:
                assert any(value in actual[3] for value in markers) or relative_primary_matches(controller,actual[3]),'Controller script/root mismatch: '+json.dumps({'owner':claim['ownerThreadId'],'offer':claim['offerId'],'pid':controller['pid'],'birth':actual_birth,'declaredMarkers':markers,'observedScriptCandidates':[part for part in actual[3].split() if part.endswith(('.py','.mjs','.cjs','.ts'))],'commandSHA256':hashlib.sha256(actual[3].encode()).hexdigest()})
            ids=descendants(rows,controller['pid'])
        elif claim.get('legacyMatchers'):
            roots=[p for p,_,_,command in rows if any(marker in command for marker in claim['legacyMatchers'])]
            for pid in roots:ids.update(descendants(rows,pid))
        # A guardian can be an ancestor or sibling, rather than a child of the
        # workload controller. Bind every declared guard before unioning its
        # tree; one claim still pays the original complete PID/RSS reservation.
        for guard in claim.get('guardControllers',[]):
            pid=guard.get('pid')
            if pid not in by_pid:continue
            assert claim.get('kind')!='ordinary' and claim.get('executionWorkflow') is not False,'Ordinary role cannot declare live execution guards: '+json.dumps({'owner':claim['ownerThreadId'],'offer':claim['offerId'],'pid':pid})
            actual=by_pid[pid]
            actual_birth=rows.births.get(pid) if hasattr(rows,'births') else birth(pid)
            assert actual_birth and guard.get('birth') and ' '.join(actual_birth.split())==' '.join(guard['birth'].split()),'Guard PID birth mismatch'
            assert isinstance(guard.get('script'),str) and guard['script'].startswith('/') and guard['script'] in actual[3].split(),'Guard absolute script mismatch: '+json.dumps({'owner':claim['ownerThreadId'],'offer':claim['offerId'],'pid':pid,'birth':actual_birth,'declaredScript':guard.get('script'),'observedScriptCandidates':[part for part in actual[3].split() if part.endswith(('.py','.mjs','.cjs','.ts'))],'commandSHA256':hashlib.sha256(actual[3].encode()).hexdigest()})
            ids.update(descendants(rows,pid))
        if claim.get('kind')=='ordinary' or claim.get('executionWorkflow') is False:
            for child in ids:
                command=by_pid[child][3];executable=Path(command.split()[0]).name.lower()
                heavy=executable in ['node','cargo','rustc'] or ('Google Chrome' in command and '--headless' in command)
                scripted_child=child!=claim['controller']['pid'] and executable in ['python','python3','python3.14'] and any(part.endswith(('.py','.mjs')) for part in command.split())
                assert not heavy and not scripted_child,'Ordinary role cannot cover runtime payload'
        assert not (covered&ids),'Workflow roots overlap'
        covered.update(ids);observed.append({'owner':claim['ownerThreadId'],'offer':claim['offerId'],'kind':claim['kind'],'pids':len(ids),'rss':sum(by_pid[p][2] for p in ids if p in by_pid),'reservedWithoutVisibleController':not bool(ids)})
        if claim.get('kind')!='ordinary' and claim.get('executionWorkflow') is not False:
            assert len(ids)<=claim['processCap'] and sum(by_pid[p][2] for p in ids if p in by_pid)<=claim['rssCapBytes'],'Peer runtime exceeds declared cap'
    unknown=[]
    for pid,parent,rss,command in rows:
        executable=Path(command.split()[0]).name.lower()
        workflow=executable in ['python','python3','python3.14','node','cargo','rustc'] and '/mcr' in command
        controls=False
        observations=[interface.get('verifiedInfrastructureObservation',{}),*interface.get('verifiedInfrastructureObservations',[]),*EXACT_DESKTOP_OBSERVATIONS]
        assert len(observations)<=16 and all(isinstance(item,dict) for item in observations),'Unbounded infrastructure observations'
        for observation in observations:
            if pid in observation.get('pids',[]) and parent==observation.get('ppid'):
                expected_program=observation['actualPrograms'][observation['pids'].index(pid)]
                actual_birth=rows.births.get(pid) if hasattr(rows,'births') else birth(pid)
                actual_executable=subprocess.check_output(['ps','-p',str(pid),'-o','comm='],text=True).strip()
                actual_group=subprocess.check_output(['ps','-p',str(pid),'-o','pgid='],text=True).strip()
                controls=bool(actual_birth and ' '.join(actual_birth.split())==' '.join(observation['birth'].split()) and actual_executable==observation['executable'] and actual_group==str(observation['pgid']) and expected_program in command.split())
                if controls and observation.get('programSha256'):
                    index=observation['pids'].index(pid);expected=observation['programSha256'][index]
                    assert hashlib.sha256(Path(expected_program).read_bytes()).hexdigest()==expected and hashlib.sha256(Path(observation['referencePrograms'][index]).read_bytes()).hexdigest()==expected,'Exact desktop program bytes changed'
                if controls:break
        if workflow and not controls and pid not in covered:
            unknown.append({'pid':pid,'parent':parent,'executable':executable,
                'scriptCandidates':[part for part in command.split() if part.endswith(('.py','.mjs','.cjs','.ts'))],
                'birth':getattr(rows,'births',{}).get(pid),
                'parentBirth':getattr(rows,'births',{}).get(parent),
                'commandDigest':hashlib.sha256(command.encode()).hexdigest(),
                'parentCommandDigest':hashlib.sha256(by_pid[parent][3].encode()).hexdigest() if parent in by_pid else None,
                'inlineCode':'-c' in command.split(),'stdinCode':'-' in command.split(),
                'preflightFlag':'--preflight' in command.split()})
    assert not unknown,'Unresolved live workflow roots: '+str(unknown)
    result={'observerPid':os.getpid(),'claims':len(claims),'reservedPids':sum(c['processCap'] for c in claims.values()),'reservedRss':sum(c['rssCapBytes'] for c in claims.values()),'categories':count,'observed':observed,'unknown':unknown,'membershipAt':membership['asOfUTC'],'psSnapshotSHA256':hashlib.sha256(json.dumps(rows,sort_keys=True).encode()).hexdigest()}
    if config.get('censusRoot'):
        with (Path(config['censusRoot'])/('census-'+str(os.getpid())+'.jsonl')).open('a') as log:log.write(json.dumps(result)+'\n')
    return result
