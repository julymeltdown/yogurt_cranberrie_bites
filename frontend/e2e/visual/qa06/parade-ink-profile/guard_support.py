"""Shared accounting inputs; each live guard samples independently."""
import hashlib
import json
import re
import os
from pathlib import Path
import subprocess
import time
from datetime import datetime


def inventory(roots):
    ledger = {}
    seen = set()
    rows = []
    for root in map(Path, roots):
        count = total = 0
        for parent, directories, files in os.walk(root, followlinks=False):
            for name in ['.', *directories, *files]:
                path = Path(parent) if name == '.' else Path(parent) / name
                try:
                    item = path.lstat()
                except FileNotFoundError:
                    continue
                key = (item.st_dev, item.st_ino)
                if key in seen:
                    continue
                seen.add(key)
                count += 1
                total += item.st_blocks * 512
                ledger[f'{item.st_dev}:{item.st_ino}'] = item.st_blocks * 512
        rows.append({'path': str(root), 'inodes': count, 'allocatedBytes': total})
    return {'roots': rows, 'uniqueInodes': len(seen), 'allocatedBytes': sum(row['allocatedBytes'] for row in rows), 'ledger': ledger}


def processes(root_pid):
    output = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,rss=,command='], text=True)
    rows = {}
    for line in output.splitlines():
        parts = line.split(None, 3)
        if len(parts) == 4:
            pid, parent, rss = map(int, parts[:3])
            rows[pid] = {'pid': pid, 'parent': parent, 'rssKiB': rss,
                         'commandHash': hashlib.sha256(parts[3].encode()).hexdigest()}
    if root_pid is None:
        return list(rows.values())
    ids = {root_pid}
    while True:
        expanded = ids | {pid for pid, row in rows.items() if row['parent'] in ids}
        if expanded == ids:
            return [row for pid, row in rows.items() if pid in ids]
        ids = expanded


def directory_bytes(path, suffix=None):
    total = 0
    for parent, _, files in os.walk(path, followlinks=False):
        for name in files:
            file = Path(parent) / name
            if suffix and file.suffix != suffix:
                continue
            try:
                if not file.is_symlink():
                    total += file.stat().st_size
            except FileNotFoundError:
                continue
    return total


_highwater = {}

def sample(config):
    import shutil
    family = processes(config['rootPid'])
    whole = inventory(config['roots'])
    baseline = config['initialLedger']
    for key, value in whole.pop('ledger').items():
        _highwater[key] = max(_highwater.get(key, baseline.get(key, 0)), value)
    positive = sum(max(0, value - baseline.get(key, 0)) for key, value in _highwater.items())
    return {'chargedWholeBytes': max(whole['allocatedBytes'], config['initialWholeBytes'], config['priorHistoricalMaxBytes']), 'positiveGrowthBytes': positive, 'freeBytes': shutil.disk_usage(config['repo']).free, 'processes': family,
            'rssKiB': sum(row['rssKiB'] for row in family), 'whole': whole,
            'tempBytes': directory_bytes(config['temporary']),
            'durableBytes': directory_bytes(config['output']),
            'logBytes': directory_bytes(config['output'], '.log'), 'global': global_sample()}


def violation(value, config):
    limits = config['limits']
    cohort=value['global']
    if cohort['unknown']:
        return 'Unresolved global runtime identity: '+str(cohort['unknown'])
    if len(cohort['claims'])>4 or cohort['browsers']>2 or cohort['sources']>3 or cohort['targets']>1 or max(len(cohort['actualPids']),cohort['reservedProcessCaps'])>96 or max(cohort['actualRSSBytes'],cohort['reservedRSSBytes'])>18*1024**3:
        return 'Global workflow/process/RSS cap'
    if value['freeBytes'] < 8 * 1024**3:
        return '8GiB live floor'
    if len(value['processes']) > limits['processes'] or value['rssKiB'] > limits['rssMiB'] * 1024:
        return 'Whole descendant process/RSS cap'
    if value['chargedWholeBytes'] > limits['wholeBytes']:
        return 'Complete unique-inode WHOLE cap'
    if value['positiveGrowthBytes'] > limits['newGrowthBytes']:
        return 'NEW positive growth cap'
    if value['tempBytes'] > limits['tempBytes'] or value['durableBytes'] > limits['durableBytes'] or value['logBytes'] > limits['logBytes']:
        return 'TEMP/durable/log cap'
    return None


RECEIPT_ROOT = Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d')
OWNER = '01a1a1fe-1ab7-79b0-adbc-d4c025701acb'


def publish_receipt(report, state, secondary=None):
    def identity(pid, script):
        return {'pid': pid, 'birth': subprocess.check_output(['ps','-p',str(pid),'-o','lstart='], text=True).strip(),
                'executable': subprocess.check_output(['ps','-p',str(pid),'-o','comm='], text=True).strip(),
                'launcherExecutable': str(Path(os.sys.executable).resolve()), 'script': str(script)}
    controller = identity(report['controller']['pid'], report['controller']['path'])
    controller['outputRoot'] = report['controller']['outputRoot']
    guards = [controller]
    if secondary is not None:
        guards.append(identity(secondary.pid, Path(report['controller']['path']).with_name('watchdog.py')))
    record = {'observedAtEpoch': time.time(), 'ownerThreadId': OWNER, 'offerId': report['job'], 'state': state,
        'acceptedAtEpoch': report['acceptedEpoch'], 'workDeadlineEpoch': report['workDeadline'],
        'fullDeadlineEpoch': report['cleanupDeadline'], 'kind': 'browser', 'processCap': 20,
        'rssCapBytes': 4*1024**3, 'remainingNewUpperBytes': 256*1024**2 if state == 'ACCEPTED' else 0,
        'controller': controller, 'guardControllers': guards,
        'receiptPaths': {key: str(Path(controller['outputRoot']) / name) for key,name in [('accept','accepted.json'),('full','full.json')]}}
    target = RECEIPT_ROOT / 'owner-resource-receipts' / (OWNER + '.json')
    temporary = target.with_suffix('.tmp')
    temporary.write_text(json.dumps(record, indent=2)+'\n')
    os.replace(temporary, target)


def global_sample():
    metadata = json.loads((RECEIPT_ROOT / 'resource-membership-current.json').read_text())
    records = {}
    for path in sorted((RECEIPT_ROOT / 'owner-resource-receipts').glob('*.json')):
        record = json.loads(path.read_text())
        records[record['ownerThreadId']] = record
    claims = []
    for entry in metadata['entries']:
        if entry['state'] == 'DECLINED' or ('FULL' in entry['state'] and 'NO_LITERAL_FULL' not in entry['state']):
            continue
        record = records.get(entry['owner'])
        # A newer owner's receipt supersedes its older management snapshot only.
        if record is not None:
            if record['state'] == 'ACCEPTED':
                continue
            # A terminal old owner record cannot erase a newer legacy acceptance.
            accepted = entry.get('actualAccept')
            if isinstance(accepted, str):
                try:
                    accepted = datetime.fromisoformat(accepted.replace('Z','+00:00')).timestamp()
                except ValueError:
                    accepted = None
            released = record.get('fullAtEpoch', record.get('actualFullEpoch', record.get('actualFullReleaseEpoch', record.get('observedAtEpoch',record.get('publishedAtEpoch',0)))))
            if accepted is not None and released >= accepted:
                continue
        claims.append({'owner': entry['owner'], 'kind': entry.get('kind','source'),
            'processCap': entry['processCap'], 'rssCapBytes': entry['rssCapBytes'] if 'rssCapBytes' in entry else int(entry['rssCapGiB']*1024**3),
            'remainingNewUpperBytes': entry['remainingNewUpperBytes'] if 'remainingNewUpperBytes' in entry else entry['remainingNewConservativeUpperBytes'],
            'matchers': entry.get('knownRootMatchers',[]), 'controller': None})
    for record in records.values():
        if record['state'] in ['FULL','DECLINED']:
            continue
        assert record['state'] == 'ACCEPTED', 'Unknown receipt state'
        claims.append({'owner': record['ownerThreadId'], 'kind':record['kind'],
            'processCap':record['processCap'], 'rssCapBytes':record['rssCapBytes'],
            'remainingNewUpperBytes':record['remainingNewUpperBytes'],
            'controller':record['controller'], 'matchers':[record['controller']['script'],record['controller']['outputRoot']]})
    raw = subprocess.check_output(['ps','-axo','pid=,ppid=,rss=,command='],text=True)
    rows = {}
    for line in raw.splitlines():
        parts=line.split(None,3)
        if len(parts)==4:
            pid,parent,rss=map(int,parts[:3]); rows[pid]={'pid':pid,'parent':parent,'rssKiB':rss,'command':parts[3]}
    assigned=set()
    for claim in claims:
        controller=claim['controller']; ids=set()
        if controller and controller['pid'] in rows:
            birth=subprocess.check_output(['ps','-p',str(controller['pid']),'-o','lstart='],text=True).strip()
            assert birth.split()==controller['birth'].split(), 'Controller PID birth mismatch'
            assert controller['script'].replace('/private/var/','/var/') in rows[controller['pid']]['command'].replace('/private/var/','/var/'), 'Controller path mismatch'
            ids.add(controller['pid'])
        # Actual runtime commands only: shell prose and preparation are not workflows.
        for pid,row in rows.items():
            command=row['command']; executable=command.split()[0]
            runtime=bool(re.search(r'(Python|python[0-9.]*|node|cargo|rustc|vitest|vite|oxlint)$',executable)) or ('Google Chrome' in command and '--headless' in command)
            if runtime and any(value and value in command for value in claim['matchers']):
                ids.add(pid)
        while True:
            expanded=ids | {pid for pid,row in rows.items() if row['parent'] in ids}
            if expanded==ids: break
            ids=expanded
        assert not assigned.intersection(ids), 'Ambiguous workflow ancestry'
        assigned.update(ids)
    unknown=[]
    for pid,row in rows.items():
        cmd=row['command']; executable=cmd.split()[0]
        runtime=bool(re.search(r'(Python|python[0-9.]*|node|cargo|rustc|vitest|vite|oxlint)$',executable))
        relevant=runtime and ('/mcr-worktrees/' in cmd or '/private/tmp/mcr-' in cmd or '/Desktop/mcr/' in cmd or bool(re.search(r'/(?:private/)?var/folders/.*mcr-',cmd)))
        relevant=relevant or ('Google Chrome' in cmd and '--headless' in cmd)
        if relevant and pid not in assigned:
            unknown.append({'pid':pid,'parent':row['parent'],'commandHash':hashlib.sha256(cmd.encode()).hexdigest()})
    return {'claims':claims,'unknown':unknown,'actualPids':sorted(assigned),
        'actualRSSBytes':sum(rows[pid]['rssKiB']*1024 for pid in assigned),
        'reservedProcessCaps':sum(row['processCap'] for row in claims),
        'reservedRSSBytes':sum(row['rssCapBytes'] for row in claims),
        'browsers':sum(row['kind']=='browser' for row in claims),
        'sources':sum(row['kind']=='source' for row in claims),
        'targets':sum(row['kind']=='target' for row in claims)}
