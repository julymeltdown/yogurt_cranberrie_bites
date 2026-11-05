"""Private Auth232 guard controls. Import performs no target access."""
import gzip
import fcntl
import hashlib
import traceback
import json
import os
from pathlib import Path
import subprocess
import time
import signal
import accounting

R = Path(__file__).parent

def atomic(name, value):
    p = R / name
    t = R / (name + '.tmp')
    t.write_text(json.dumps(value, indent=2) + '\n')
    t.replace(p)

def snapshot(name,value):
    # Diagnostic files have one inode per sampler. Do not create an unbounded
    # series of replaced inodes while preserving positive-growth accounting.
    text=json.dumps(value,indent=2)+'\n'
    assert len(text.encode())<=512*1024, 'bounded diagnostic snapshot'
    path=R/name
    with path.open('r+' if path.exists() else 'w') as f:
        f.seek(0);f.write(text);f.truncate();f.flush()


def process_table(pids=None):
    with (R/'ps-helper.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        argv=['/bin/ps','-axo','pid=,ppid=,pgid=,rss=,lstart=,command='] if pids is None else ['/bin/ps','-p',','.join(str(pid) for pid in pids),'-o','pid=,ppid=,pgid=,rss=,lstart=,command=']
        raw = subprocess.check_output(argv, text=True, timeout=5)
    rows = {}
    for line in raw.splitlines():
        p = line.split(maxsplit=9)
        if len(p) == 10:
            rows[int(p[0])] = (int(p[1]), int(p[2]), int(p[3]) * 1024, p[9], " ".join(p[4:9]))
    return rows

def descendants(rows, roots, groups=()):
    ids = set(roots)
    while True:
        found = {pid for pid, row in rows.items() for parent, group in [row[:2]] if parent in ids or group in groups} - ids
        if not found:
            return ids & rows.keys()
        ids.update(found)

def declared_capacity(record):
    if record.get('kind')=='ordinary':
        return {'executionCapDeclared':False,'processes':0,'rss':0,'remainingNew':0}
    if record.get('state') in ('FULL','DECLINED'):
        return {'executionCapDeclared':True,'processes':0,'rss':0,'remainingNew':0}
    assert type(record.get('processCap')) is int and record['processCap']>=0
    assert isinstance(record.get('rssCapBytes'),(int,float)) and record['rssCapBytes']>=0
    assert isinstance(record.get('remainingNewUpperBytes'),(int,float)) and record['remainingNewUpperBytes']>=0
    return {'executionCapDeclared':True,'processes':record['processCap'],'rss':record['rssCapBytes'],'remainingNew':record['remainingNewUpperBytes']}



def proof_path(plan, record, key):
    original=Path(record['receiptPaths'][key])
    if original.exists():return original
    # Only exact, frozen historical FULL bytes can substitute for a moved
    # receipt. Live accounting roots and ACCEPT receipts never use aliases.
    assert key=='full' and record.get('state')=='FULL', 'missing live receipt owner='+str(record.get('owner',record.get('ownerThreadId')))+' offer='+str(record.get('offerId'))+' key='+key+' path='+str(original)
    aliases=json.loads(Path(plan['historical_receipt_aliases_file']).read_text())['aliases']
    owner=record.get('owner',record.get('ownerThreadId'))
    epoch=record.get('actualAccept',record.get('acceptedAtEpoch'))
    matches=[a for a in aliases if a['owner']==owner and a['offer']==record.get('offerId')
        and a['accepted_epoch']==epoch and a['original_path']==str(original)]
    assert len(matches)==1, 'unresolved missing historical receipt: '+str(original)
    a=matches[0];retained=Path(a['retained_path'])
    assert retained.is_file() and not retained.is_symlink() and retained.stat().st_size<=4*1024**2
    assert hashlib.sha256(retained.read_bytes()).hexdigest()==a['sha256'], 'retained literal receipt changed'
    return retained


class ReceiptDecodeError(ValueError):
    def __init__(self,identity,error):
        self.identity=identity
        super().__init__('receipt decode failed '+json.dumps(identity,sort_keys=True)+': '+str(error))
    def receipt(self):return self.identity


def decode_receipt_bytes(path,raw,record):
    """Validate every nonblank JSONL record; never treat its last line alone
    as release authority. Canonical owner/offer/epoch reconciliation is separate.
    Preserve the exact path and byte identity if any record is malformed.
    """
    path=Path(path)
    identity={'path':str(path),'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw),
        'format':'JSONL' if path.suffix=='.jsonl' else 'JSON',
        'owner':record.get('owner',record.get('ownerThreadId')),'offerId':record.get('offerId'),
        'acceptedAtEpoch':record.get('actualAccept',record.get('acceptedAtEpoch'))}
    try:
        values=[]
        if path.suffix=='.jsonl':
            for number,line in enumerate(raw.splitlines(),1):
                if line.strip():
                    identity['recordLine']=number
                    values.append(json.loads(line))
        else:values=[json.loads(raw)]
        assert values and all(isinstance(value,dict) for value in values),'nonempty receipt object records required'
        return values
    except (ValueError,UnicodeError,AssertionError) as error:
        raise ReceiptDecodeError(identity,error) from error


def validate_full_receipt(plan,record):
    proof=proof_path(plan,record,'full')
    assert proof.is_file() and not proof.is_symlink() and proof.stat().st_size<=4*1024**2,'bounded literal FULL receipt'
    raw=proof.read_bytes()
    expected=record.get('receiptSHA256',{}).get('full')
    if expected:assert hashlib.sha256(raw).hexdigest()==expected,'literal FULL receipt hash'
    return decode_receipt_bytes(proof,raw,record)


def owner_receipts(plan):
    import hashlib,re
    directory=Path(plan['owner_receipt_directory'])
    paths=sorted(directory.glob('*.json'))
    assert len(paths)<=256, 'bounded owner receipt directory'
    result=[]
    for path in paths:
        assert re.fullmatch(r'[0-9a-f-]{36}\.json',path.name), 'owner receipt filename'
        assert path.is_file() and not path.is_symlink() and path.stat().st_size<=256*1024
        record=json.loads(path.read_text())
        raw_state=record.get('state')
        if isinstance(raw_state,str) and raw_state.startswith('ACCEPTED_'):
            assert isinstance(record.get('acceptedAtEpoch'),(int,float)) and record['acceptedAtEpoch']>0, 'actual bootstrap acceptance required'
            record['state']='ACCEPTED'
        assert path.stem==record['ownerThreadId']
        assert record['state'] in ('ACCEPTED','FULL','DECLINED'), 'unknown owner receipt state '+str(record.get('ownerThreadId'))+' '+str(raw_state)
        observation=record.get('ordinaryObservation')
        if isinstance(observation,dict) and observation.get('endedAt') is None and observation.get('actualpid'):
            assert observation.get('birth') and observation.get('role') and observation.get('cwd'), 'concrete ordinary observation'
            result.append({'cohort':'ordinary:'+record['ownerThreadId']+':'+str(observation['actualpid']),
                'owner':record['ownerThreadId'],'state':'ACTUAL_ORDINARY_OBSERVATION','actualAccept':observation.get('observedAt'),
                'offerId':None,'kind':'ordinary','processCap':0,'rssCapGiB':0,'remainingNewConservativeUpperBytes':0,
                'knownRootMatchers':[], 'controller':{'pid':observation['actualpid'],'birth':observation['birth'],'cwd':observation['cwd']},
                'guardControllers':[],'ordinaryObservation':observation})
        if record['state']=='DECLINED':continue
        if record.get('kind')=='ordinary':
            # Ordinary bookings/closures are not execution admissions. Their
            # factual live descriptors count actual PID/RSS; they grant no
            # application slot and can never close legacy execution claims.
            if record['state']=='FULL':continue
            controller=record.get('controller') or {}
            matchers=[v for v in (controller.get('script'),controller.get('outputRoot')) if v]
            if controller:
                assert type(controller.get('pid')) is int and controller['pid']>0
                assert controller.get('birth') and matchers, 'factual ordinary live descriptor'
            result.append({'cohort':'owner:'+record['ownerThreadId'],'owner':record['ownerThreadId'],
                'state':'ACTUAL_ORDINARY_OBSERVATION','actualAccept':record.get('acceptedAtEpoch'),
                'offerId':record.get('offerId'),'kind':'ordinary','processCap':0,'rssCapGiB':0,
                'remainingNewConservativeUpperBytes':0,'knownRootMatchers':matchers,
                'controller':controller,'guardControllers':record.get('guardControllers',[])})
            continue
        if record['state']=='FULL' and record.get('acceptedAtEpoch') is None:
            assert record['remainingNewUpperBytes']==0, 'closed static record reserves growth'
            if record.get('kind')=='ordinary':
                continue # No execution acceptance; never closes a legacy accepted claim.
            validate_full_receipt(plan,record)
            continue # Non-execution publication neither grants nor erases a legacy claim.
        if record['state']=='ACCEPTED':
            assert isinstance(record['acceptedAtEpoch'],(int,float))
            assert record['workDeadlineEpoch']>=record['acceptedAtEpoch']
            assert record['fullDeadlineEpoch']>=record['workDeadlineEpoch']
        caps=declared_capacity(record)
        if record['state']=='FULL':
            record['processCap']=caps['processes'];record['rssCapBytes']=caps['rss'];record['remainingNewUpperBytes']=caps['remainingNew']
        else:assert record['kind'] in ('source','browser','target','copied-native','ordinary')
        if record['state']=='ACCEPTED' and record['kind']=='ordinary':
            record['processCap']=caps['processes'];record['rssCapBytes']=caps['rss'];record['remainingNewUpperBytes']=caps['remainingNew']
        assert type(record['processCap']) is int and record['processCap']>=0
        assert record['rssCapBytes']>=0 and record['remainingNewUpperBytes']>=0
        # The atomically published canonical ACCEPT is the reservation itself.
        # A controller may create its append log only after initial census.
        # Reading that later, mutable file here creates a false admission gate.
        proof_keys=[] if record['state']=='ACCEPTED' else ['full']
        for key in proof_keys:
            assert key=='full'
            validate_full_receipt(plan,record)
        controller=record.get('controller') or {}
        matchers=[v for v in (controller.get('script'),controller.get('outputRoot')) if v]
        # A factual accepted booking can precede controller launch. It still
        # reserves its full execution class/caps until matching literal FULL.
        # No empty descriptor registers or exempts a visible runtime PID.
        if record['state']=='ACCEPTED' and controller:
            assert type(controller.get('pid')) is int and controller['pid']>0
            assert isinstance(controller.get('birth'),str) and controller['birth']
            assert matchers, 'concrete live owner script/output root'
        result.append({'cohort':'owner:'+record['ownerThreadId'],'owner':record['ownerThreadId'],
            'state':('ACTUAL_ORDINARY_OBSERVATION' if record['kind']=='ordinary' else 'ACTUAL_ACCEPT_RECEIPT') if record['state']=='ACCEPTED' else 'LITERAL_FULL_RECEIVED_RESOURCES_RELEASED',
            'actualAccept':record['acceptedAtEpoch'],'offerId':record['offerId'],'kind':record['kind'],
            'processCap':record['processCap'],'rssCapGiB':record['rssCapBytes']/1024**3,
            'rawReceiptState':raw_state,'remainingNewConservativeUpperBytes':record['remainingNewUpperBytes'] if record['state']=='ACCEPTED' else 0,
            'knownRootMatchers':matchers,'controller':controller,'guardControllers':record.get('guardControllers',[])})
    return result


def reconciled_cohorts(plan, previous=None):
    cohorts=dict(previous or {})
    # Allocator metadata routes known receipts; actual process trees and
    # birth/group identities supply live PID/RSS proof independently.
    metadata = json.loads(Path(plan['management_membership_file']).read_text())
    assert metadata['issuer'] == '01a1a21d-39ec-7912-84cb-910058e7a110'
    for raw_entry in metadata['entries']:
        entry=dict(raw_entry)
        if isinstance(entry.get('state'),str) and entry['state'].startswith('ACCEPTED_'):
            assert isinstance(entry.get('acceptedAtEpoch',entry.get('actualAccept')),(int,float)), 'actual embedded bootstrap acceptance'
            entry['state']='ACCEPTED'
        controller=entry.get('controller') or {}
        entry['owner']=entry.get('owner',entry.get('ownerThreadId'))
        entry['actualAccept']=entry.get('actualAccept',entry.get('acceptedAtEpoch'))
        entry['cohort']=entry.get('cohort') or 'legacy:'+str(entry['owner'])+':'+str(entry.get('offerId',entry['actualAccept']))
        entry['knownRootMatchers']=entry.get('knownRootMatchers') or [v for v in (controller.get('script'),controller.get('outputRoot')) if v]
        if entry['state']=='DECLINED':continue
        if entry.get('kind')=='ordinary':
            if entry['state']=='FULL':continue
            assert entry['state']=='ACCEPTED' or entry['state'].startswith(('ACTUAL_ORDINARY','LITERAL_FULL')), 'ordinary state'
            entry['state']='ACTUAL_ORDINARY_OBSERVATION' if entry['state']=='ACCEPTED' else entry['state']
            entry['processCap']=0;entry['rssCapGiB']=0;entry['remainingNewConservativeUpperBytes']=0
            cohorts[entry['cohort']]=entry
            continue
        if entry['state']=='FULL' and entry.get('acceptedAtEpoch') is None:continue
        legacy_full=entry['state']=='FULL' and entry.get('legacyState','').startswith('LITERAL_FULL')
        if legacy_full:
            assert entry.get('sourceMetadataHistory') and Path(entry['sourceMetadataHistory']).is_file(), 'retained legacy literal release metadata'
            entry['state']='LITERAL_FULL_RECEIVED_RESOURCES_RELEASED'
        if entry['state'] in ('FULL','ACCEPTED'):
            key='full' if entry['state']=='FULL' else 'accept'
            if key=='full':
                validate_full_receipt(plan,entry)
            else:
                assert isinstance(entry['actualAccept'],(int,float)) and entry['actualAccept']>0
                assert entry['workDeadlineEpoch']>=entry['actualAccept']
                assert entry['fullDeadlineEpoch']>=entry['workDeadlineEpoch']
                declared_capacity(entry)
            entry['state']='LITERAL_FULL_RECEIVED_RESOURCES_RELEASED' if key=='full' else 'ACTUAL_ACCEPT_RECEIPT'
            ordinary=entry.get('kind')=='ordinary'
            if key=='accept' and ordinary:
                entry['state']='ACTUAL_ORDINARY_OBSERVATION';entry['processCap']=0
            entry['rssCapGiB']=entry['rssCapBytes']/1024**3 if key=='accept' and not ordinary else 0
            if key=='full':entry['processCap']=0
            entry['remainingNewConservativeUpperBytes']=entry['remainingNewUpperBytes'] if key=='accept' and not ordinary else 0
        cohorts[entry['cohort']] = entry
    overrides = json.loads(Path(plan['cohort_overrides_file']).read_text())
    for entry in overrides['cohorts']:
        if 'receipt_path' in entry:
            assert hashlib.sha256(Path(entry['receipt_path']).read_bytes()).hexdigest()==entry['receipt_sha256'], 'cohort literal receipt changed'
        prior=cohorts.get(entry['cohort'])
        if prior and prior['state'].startswith('LITERAL_FULL') and entry['state'].startswith('ACTUAL_ACCEPT'):continue
        cohorts[entry['cohort']] = entry
    # Reconcile owner records with legacy reservations. A missing record
    # never removes a claim. Current owner ACCEPT caps retain any unresolved
    # legacy upper bound; literal FULL closes only its matching prior claim.
    # A current booking need not overwrite the historical terminal record for
    # an earlier exact acceptance. Consume only explicit, exact terminal rows.
    for name,old in list(cohorts.items()):
        if not (old['state'].startswith('ACTUAL_ACCEPT') or old['state']=='ACCEPTED'):continue
        owner=old.get('owner',old.get('ownerThreadId'));epoch=old.get('actualAccept',old.get('acceptedAtEpoch'))
        if not owner or not old.get('offerId') or not isinstance(epoch,(int,float)):continue
        path=Path(plan['owner_receipt_directory'])/(owner+'.json')
        if not path.is_file():continue
        assert not path.is_symlink() and path.stat().st_size<=256*1024
        canonical=json.loads(path.read_text());assert canonical['ownerThreadId']==owner
        history=canonical.get('closedHistory',[])+canonical.get('closedOffers',[])
        for row in history:
            if row.get('offerId')!=old['offerId'] or row.get('acceptedAtEpoch')!=epoch:continue
            if row.get('ownerThreadId',owner)!=owner:continue
            if row.get('state','FULL')!='FULL' or row.get('resourcesReleased') is not True or row.get('kind')=='ordinary':continue
            digest=row.get('fullSHA256',row.get('fullSha256',row.get('fullSHA')))
            assert isinstance(digest,str) and len(digest)==64 and all(c in '0123456789abcdef' for c in digest), 'exact terminal history hash'
            assert isinstance(row.get('actualFullEpoch'),(int,float)) and row['actualFullEpoch']>=epoch
            literal=row.get('fullPath')
            if literal and Path(literal).exists():
                proof=Path(literal);assert proof.is_file() and not proof.is_symlink() and proof.stat().st_size<=4*1024**2
                raw=proof.read_bytes()
                assert hashlib.sha256(raw).hexdigest()==digest,'terminal history literal changed'
                decode_receipt_bytes(proof,raw,{'owner':owner,**row})
            cohorts[name]={**old,'state':'LITERAL_FULL_RECEIVED_RESOURCES_RELEASED','processCap':0,'rssCapGiB':0,
                'remainingNewConservativeUpperBytes':0,'exactCanonicalTerminalHistory':row}
            break
    for current in owner_receipts(plan):
        for name,old in list(cohorts.items()):
            if name==current['cohort'] or old.get('owner',old.get('ownerThreadId'))!=current['owner']:continue
            if current.get('kind')=='ordinary' and old.get('kind')!='ordinary':continue
            same_epoch=current['actualAccept'] is not None and old.get('actualAccept',old.get('acceptedAtEpoch'))==current['actualAccept']
            same_root=any(a in b or b in a for a in old['knownRootMatchers'] for b in current['knownRootMatchers'])
            same_offer=bool(old.get('offerId')) and old['offerId']==current['offerId']
            old_epoch=old.get('actualAccept',old.get('acceptedAtEpoch'))
            if old_epoch is not None and current['actualAccept'] is not None:
                same_attempt=same_epoch and (same_offer if old.get('offerId') and current.get('offerId') else True)
            else:
                same_attempt=same_offer or (old_epoch is None and same_root)
            if current['state'].startswith('ACTUAL_ACCEPT') and (old['state'].startswith('ACTUAL_ACCEPT') or old['state']=='ACCEPTED'):
                assert same_attempt, 'unresolved distinct accepted attempts for owner '+current['owner']
                current['processCap']=max(current['processCap'],old['processCap'])
                current['rssCapGiB']=max(current['rssCapGiB'],old.get('rssCapGiB',old.get('rssCapBytes',0)/1024**3))
                current['remainingNewConservativeUpperBytes']=max(current['remainingNewConservativeUpperBytes'],old.get('remainingNewConservativeUpperBytes',old.get('remainingNewUpperBytes',0)))
                del cohorts[name]
            elif same_attempt:del cohorts[name]
        cohorts[current['cohort']]=current
    return cohorts


def identity_resolved(witness, rows, registered, cohorts):
    """Absence and numeric PID reuse never resolve an observed identity."""
    pid=witness['pid'];birth=witness['birth']
    if pid in rows and rows[pid][4]==birth and pid in registered:return True
    for entry in cohorts.values():
        facts=[entry.get('controller')]+entry.get('guardControllers',[])
        observation=entry.get('ordinaryObservation')
        if observation:facts.append({'pid':observation.get('actualpid'),'birth':observation.get('birth'),
            'script':observation.get('script'),'cwd':observation.get('cwd'),'executable':observation.get('executable')})
        for d in facts:
            if not d or d.get('pid')!=pid or ' '.join(str(d.get('birth')).split())!=birth:continue
            if d.get('script') and d.get('cwd') and d['script'] in witness.get('command','') and d.get('executable')==witness.get('command','').split(maxsplit=1)[0]:return True
    return False


def identity_gate(plan, acceptance, pending, rows):
    """Pause only current, birth-verified direct child groups of this owner.

    Both samplers share pending gates. Neither may resume workload while the
    other has an unresolved identity. No peer process is signalled.
    """
    with (R/'identity-gates.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        path=R/'identity-gates.json'
        state=json.loads(path.read_text()) if path.exists() else {'pending':{},'paused':{}}
        state['pending'][str(os.getpid())]=pending
        blocked=any(state['pending'].values())
        groups=json.loads((R/'groups.json').read_text()) if (R/'groups.json').exists() else []
        witnesses=json.loads((R/'group-identities.json').read_text()) if (R/'group-identities.json').exists() else {}
        for group in groups:
            row=rows.get(group);w=witnesses.get(str(group))
            if row is None:continue
            assert row[0]==acceptance['controller_pid'] and w and row[4]==w['birth'], 'own group identity required before signalling'
            if blocked:
                os.killpg(group,signal.SIGSTOP)
                state['paused'][str(group)]={'birth':row[4],'parent':row[0]}
        if not blocked:
            for key,w in list(state['paused'].items()):
                group=int(key);row=rows.get(group)
                if row is not None:
                    assert row[4]==w['birth'] and row[0]==w['parent']==acceptance['controller_pid'], 'paused own group identity changed'
                    os.killpg(group,signal.SIGCONT)
                del state['paused'][key]
        snapshot('identity-gates.json',state)
        return blocked


class Guard:
    def __init__(self, plan, acceptance, baseline):
        self.p, self.a, self.b = plan, acceptance, baseline
        self.roots = {name: accounting.RootIdentity(**value) for name, value in baseline['roots'].items()}
        self.high = {(device, inode): (scope, size) for scope, device, inode, size in baseline['inodes']}
        self.positive = {name: 0 for name in self.roots}
        self.peak = {}
        self.last_fast = 0
        self.cohorts = {}
        self.births = {}
        self.cleanup = False
        self.frozen = False
        self.pending_identities = {}
        self.owned_births = {}

    def fast(self):
        deadline=time.monotonic()+1.0
        while True:
            value,rows,registered,unknown=self._fast_once()
            for pid in unknown:
                row=rows[pid]
                key=(pid,row[4])
                if key not in self.pending_identities:
                    # Only a genuinely newborn process may use the bounded
                    # publication interval. Existing unknown runtimes fail.
                    birth_epoch=time.mktime(time.strptime(row[4],'%a %b %d %H:%M:%S %Y'))
                    assert 0<=time.time()-birth_epoch<=2.5, 'unregistered non-newborn runtime '+str(pid)
                    self.pending_identities[key]={'pid':pid,'birth':row[4],'parent':row[0],'group':row[1],'command':row[3],'firstObservedAt':time.time()}
            self.pending_identities={key:w for key,w in self.pending_identities.items()
                if not identity_resolved(w,rows,registered,self.cohorts)}
            pending=list(self.pending_identities.values())
            blocked=identity_gate(self.p,self.a,pending,rows)
            if not blocked:
                value['identity_reconciliation']='RESOLVED'
                return value
            assert time.monotonic()<deadline, 'bounded startup identity reconciliation unresolved: '+json.dumps(pending)
            time.sleep(.05)

    def _fast_once(self):
        now = time.time()
        assert now < self.a['full_deadline'] - 5, 'fixed FULL cutoff'
        if not self.cleanup:
            assert now < self.a['work_deadline'], 'fixed WORK cutoff'
        v = os.statvfs(R)
        free = v.f_bavail * v.f_frsize
        assert free >= self.p['live_floor_bytes'], 'continuous free floor'
        rows = process_table()
        groups_file = R / 'groups.json'
        groups = json.loads(groups_file.read_text()) if groups_file.exists() else []
        # Group ids help reap/count descendants, never define workflow count.
        owned = descendants(rows, [self.a['controller_pid']])
        # Retain witnessed orphan identities, never claim a reused numeric PGID.
        for pid in owned:self.owned_births[pid]={'birth':rows[pid][4],'group':rows[pid][1]}
        owned |= {pid for pid,w in self.owned_births.items() if pid in rows and
                  rows[pid][4]==w['birth'] and rows[pid][1]==w['group']}
        snapshot('owned-identities-'+str(os.getpid())+'.json',{str(pid):w for pid,w in self.owned_births.items()})
        candidates={pid:row for pid,row in rows.items() if pid in owned or
            '/Users/lhs/Desktop/mcr' in row[3] or '/private/tmp/mcr-' in row[3] or
            '/var/folders/' in row[3] and 'mcr' in row[3]}
        snapshot('census-'+str(os.getpid())+'.json',{'epoch':now,'controller_pid':self.a['controller_pid'],
            'sampler_pid':os.getpid(),'owned_pids':sorted(owned),'rows':{str(pid):{'parent':row[0],
            'group':row[1],'rss':row[2],'command':row[3],'birth':row[4]} for pid,row in candidates.items()}})
        rss = sum(rows[pid][2] for pid in owned)
        assert len(owned) <= self.p['max_processes'], 'own PID cap'
        assert rss <= self.p['max_rss_bytes'], 'own RSS cap'
        self.cohorts=reconciled_cohorts(self.p,self.cohorts)
        global_ids = set(owned)
        own_kind='source' if (R/'TARGET.json').exists() else self.p.get('own_kind','target')
        workflows=1;source_count=int(own_kind=='source');browser_count=0;rust_count=int(own_kind=='target')
        declared_pids = self.p['max_processes']; declared_rss = self.p['max_rss_bytes']
        remaining_new = 0
        registrations = set(); released_matches=set()
        for name, entry in self.cohorts.items():
            if entry.get('owner') == self.p['thread_id']:continue
            matches = {pid for pid,row in rows.items() if any(m in row[3] for m in entry['knownRootMatchers'])}
            ids = descendants(rows, matches)
            released = entry['state'].startswith('LITERAL_FULL')
            if released:
                released_matches |= ids
                continue
            if entry.get('kind')=='ordinary':
                descriptor=entry.get('controller') or {};pid=descriptor.get('pid')
                if pid in rows:
                    assert ' '.join(str(descriptor['birth']).split())==rows[pid][4], 'ordinary PID birth mismatch '+name
                    observation=entry.get('ordinaryObservation')
                    if observation:
                        assert observation.get('role') and observation.get('cwd'), 'ordinary role/cwd missing'
                    else:assert any(m in rows[pid][3] for m in entry['knownRootMatchers']), 'ordinary script/root mismatch '+name
                    ids |= descendants(rows,[pid])
                registrations |= ids;global_ids |= ids
                continue
            assert entry['state'].startswith('ACTUAL_ACCEPT'), 'unresolved cohort state '+name
            # An accepted cohort remains reserved even with no visible process.
            kind = entry.get('class') or {'source':'SOURCE','browser':'BROWSER','target':'RUST','copied-native':'SOURCE'}.get(entry.get('kind'))
            if kind is None:
                kind = 'SOURCE' if 'source' in name.lower() else ('BROWSER' if 'native' in name.lower() or 'browser' in name.lower() else 'RUST')
            if kind=='COPIED_NATIVE':kind='SOURCE' # Same G1/S1 lease through copied native tests.
            workflows += 1
            if kind == 'SOURCE': source_count += 1
            browser_count += kind == 'BROWSER'; rust_count += kind == 'RUST'
            declared_pids += entry['processCap']; declared_rss += int(entry['rssCapGiB'] * 1024**3)
            remaining_new += entry['remainingNewConservativeUpperBytes']
            for pid in matches:
                identity = (rows[pid][4], rows[pid][1], rows[pid][3])
                if pid in self.births:
                    old=self.births[pid]
                    assert old[:2]==identity[:2], 'cohort PID birth/group changed'
                    if old[2]!=identity[2]:
                        declarations=[entry.get('controller')]+entry.get('guardControllers',[])
                        assert any(d and d.get('pid')==pid and ' '.join(str(d.get('birth')).split())==identity[0] for d in declarations), 'unregistered exec transition'
                        assert any(m in rows[pid][3] for m in entry['knownRootMatchers']), 'exec transition script/root mismatch'
                self.births[pid] = identity
            for descriptor in [entry.get('controller')]+entry.get('guardControllers',[]):
                if not descriptor or not descriptor.get('pid'):continue
                pid=descriptor['pid']
                if pid in rows:
                    assert ' '.join(str(descriptor['birth']).split())==rows[pid][4], 'owner receipt PID birth mismatch '+name
                    assert any(m in rows[pid][3] for m in entry['knownRootMatchers']), 'owner controller script/root mismatch '+name
                    ids |= descendants(rows,[pid])
            registrations |= ids; global_ids |= ids
        namespace = {pid for pid,row in rows.items() if
            ('/Users/lhs/Desktop/mcr' in row[3] or '/private/tmp/mcr-' in row[3] or
             '/var/folders/' in row[3] and 'mcr' in row[3])}
        namespace = descendants(rows, namespace)
        infrastructure = set()
        overrides=json.loads(Path(self.p['cohort_overrides_file']).read_text())
        for pin in overrides.get('infrastructure', []):
            pid = pin['pid']
            if pid in rows:
                assert rows[pid][4] == pin['birth'] and rows[pid][3] == pin['command'], 'infrastructure PID reused'
                infrastructure.add(pid)
        unknown = (namespace | released_matches) - global_ids - infrastructure
        snapshot('classification-'+str(os.getpid())+'.json',{'epoch':now,'owned_pids':sorted(owned),
            'registered_pids':sorted(registrations),'infrastructure_pids':sorted(infrastructure),
            'unknown':{str(pid):{'parent':rows[pid][0],'group':rows[pid][1],'rss':rows[pid][2],
            'command':rows[pid][3],'birth':rows[pid][4]} for pid in unknown},
            'cohorts':self.cohorts})
        pending_runtime=set()
        for pid in unknown:
            command = rows[pid][3]; executable = command.split(maxsplit=1)[0]
            if Path(executable).name in ('cargo','rustc','node','postgres','initdb','pg_ctl','Python','python3','python','python3.14') or any(t in command for t in ('Chromium','Google Chrome.app','playwright')):
                pending_runtime.add(pid)
        global_ids |= namespace
        global_rss = sum(rows[pid][2] for pid in global_ids)
        assert workflows <= 4 and source_count <= 3 and browser_count <= 2 and rust_count <= 1, 'GLOBAL class cap'
        assert declared_pids <= 96 and declared_rss <= 18*1024**3, 'accepted GLOBAL declared PID/RSS cap'
        assert len(global_ids) <= 96 and global_rss <= 18*1024**3, 'actual GLOBAL PID/RSS cap'
        reservation = (max(self.p.get('minimum_other_remaining_bytes',0),self.p['other_remaining_bytes']+remaining_new) if self.p.get('reservation_mode')=='ordinary_plus_actual' else max(self.p['other_remaining_bytes'],remaining_new))
        if not (R/'primary.initial.json').exists():
            assert free - reservation - self.p['new_bytes'] >= self.p['after_reserves_floor_bytes'], 'fresh admission all-reservation floor'
        for pid in owned:
            command = rows[pid][3]
            assert not any(token in command for token in ['postgres -D', '/initdb', '/pg_ctl', 'playwright', 'Chromium', 'node ']), 'unallocated runtime child'
        value = {'epoch': now, 'free': free, 'processes': len(owned), 'rss': rss,
                 'global_workflows': workflows, 'global_processes': len(global_ids), 'global_rss': global_rss,
                 'source_workflows':source_count,'browser_workflows':browser_count,'rust_workflows':rust_count,'declared_global_pids':declared_pids,'declared_global_rss':declared_rss,'other_remaining':reservation}
        for k, n in value.items():
            if k == 'free': self.peak[k] = min(self.peak.get(k, n), n)
            elif k != 'epoch': self.peak[k] = max(self.peak.get(k, 0), n)
        self.last_fast = time.monotonic()
        return value,rows,registrations,pending_runtime

    def checkpoint(self):
        if time.monotonic() - self.last_fast >= .4:
            self.fast()

    def sample(self):
        fast = self.fast()
        roots = {k: v for k, v in self.roots.items() if not (self.frozen and k == 'target')}
        deadline = min(time.monotonic() + 25, time.monotonic() + self.a['full_deadline'] - time.time() - 5)
        report = accounting.collect(roots, deadline=deadline, checkpoint=self.checkpoint,
                                    attempts=5, retry_delay=.03, max_entries=1500000)
        assert report['complete'], 'incomplete allocation refused'
        for scope, device, inode, size in report['inodes']:
            old_scope, old = self.high.get((device, inode), (scope, 0))
            if size > old:
                self.positive[old_scope] += size - old
                self.high[(device, inode)] = old_scope, size
        target = self.positive.get('target', 0)
        outside = self.p['retained_new_floor'] + sum(v for k, v in self.positive.items() if k != 'target')
        if self.p.get('own_kind')=='source':
            # Read-only peer Git/metadata growth is retained in WHOLE, while
            # this book's NEW covers only its own evidence/control writes.
            new=self.p.get('startup_new_reserve',0)+sum(self.positive.get(k,0) for k in self.p['owned_growth_scopes'])
            outside=new
            whole=max(self.p['historical_whole_floor'],self.b['bytes'])+self.p['doc4_upper']+sum(self.positive.values())+self.p.get('startup_new_reserve',0)
        else:
            new=target+outside
            whole=max(self.p['historical_whole_floor'],self.b['bytes'])+new
        assert target <= self.p['target_new_bytes'], 'positive target growth cap'
        assert outside <= self.p['outside_new_bytes'], 'positive outside growth cap'
        assert new <= self.p['new_bytes'], 'aggregate NEW cap'
        assert whole <= self.p['whole_bytes'], 'nonshrinking complete WHOLE cap'
        value = {**fast, 'complete': True, 'new': new, 'whole': whole,
                 'target_growth': target, 'outside': outside, 'positive_by_root': dict(self.positive)}
        for k in ['new', 'whole', 'target_growth', 'outside']:
            self.peak[k] = max(self.peak.get(k, 0), value[k])
        return value

def failure_receipt(error,prefix,synthetic=False):
    """Preserve structured scanner/checkpoint failures before cleanup samples."""
    details={'exception':type(error).__name__,'message':str(error),'synthetic':synthetic,
             'epoch':time.time(),'traceback':traceback.format_exc()}
    if callable(getattr(error,'receipt',None)):
        details['receipt_input' if isinstance(error,ReceiptDecodeError) else 'accounting']=error.receipt()
    cause=error.__cause__
    if cause is not None:details['cause']={'type':type(cause).__name__,'message':str(cause)}
    for name in ['census-'+str(os.getpid())+'.json','classification-'+str(os.getpid())+'.json',
                 'identity-gates.json','owned-identities-'+str(os.getpid())+'.json']:
        path=R/name
        if path.exists():details[name]=json.loads(path.read_text())
    assert len(json.dumps(details).encode())<=1024**2,'bounded failure receipt'
    atomic(prefix+'.json',details)
    return details


def baseline(plan, checkpoint):
    roots = {name: accounting.RootIdentity.capture(path) for name, path in plan['roots'].items()}
    report = accounting.collect(roots, deadline=time.monotonic() + 30, checkpoint=checkpoint,
                                attempts=5, retry_delay=.03, max_entries=1500000)
    assert report['complete']
    report['roots'] = {name: root.__dict__ for name, root in roots.items()}
    assert max(plan['historical_whole_floor'], report['bytes']) + plan.get('doc4_upper',0) + plan['new_bytes'] <= plan['whole_bytes'], 'full NEW does not fit WHOLE'
    return report

def save_baseline(value):
    temporary=R/'baseline.json.gz.tmp'
    with gzip.open(temporary,'wt') as f:
        json.dump(value,f)
    temporary.replace(R/'baseline.json.gz')

def load_baseline():
    with gzip.open(R / 'baseline.json.gz', 'rt') as f:
        return json.load(f)
