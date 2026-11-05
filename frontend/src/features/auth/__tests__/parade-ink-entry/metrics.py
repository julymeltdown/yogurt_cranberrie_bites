"""Finite PI10 resource census; no lease is granted by this module."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

GIB = 1024**3
MIB = 1024**2
ROOT = Path(__file__).resolve().parents[6]
FRONT = ROOT / 'frontend'
PRIVATE_GIT = Path(subprocess.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=ROOT, text=True).strip())
COMMON_GIT = Path(subprocess.check_output(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], cwd=ROOT, text=True).strip())
NODE_DIST = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64')
NODE = NODE_DIST / 'bin/node'
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
CARRY = [ROOT, PRIVATE_GIT, COMMON_GIT, DEPS, NODE_DIST, Path('/Library/Frameworks/Python.framework'), Path('/opt/homebrew/opt/git').resolve(), Path('/Applications/Google Chrome.app')]


def inventory(roots):
    result = {}
    for root in roots:
        if root.is_file() or root.is_symlink():
            item = os.lstat(root)
            result[(item.st_dev, item.st_ino)] = max(item.st_size, item.st_blocks * 512)
            continue
        for base, dirs, names in os.walk(root, followlinks=False):
            for name in names:
                try:
                    item = os.lstat(Path(base) / name)
                except FileNotFoundError:
                    continue
                result[(item.st_dev, item.st_ino)] = max(item.st_size, item.st_blocks * 512)
    return result


def free():
    value = os.statvfs(ROOT)
    return value.f_bavail * value.f_frsize


def tree(root_pid, known=None):
    rows = {}
    raw = subprocess.check_output(['/bin/ps', '-axo', 'pid=,ppid=,pgid=,rss=,lstart=,comm='], text=True, timeout=4)
    for line in raw.splitlines():
        fields = line.strip().split(None, 9)
        if len(fields) == 10:
            rows[int(fields[0])] = (int(fields[1]), int(fields[2]), int(fields[3]) * 1024, fields[9], ' '.join(fields[4:9]))
    owned = {root_pid} | {pid for pid, identity in (known or {}).items() if pid in rows and (rows[pid][3], rows[pid][4]) == identity}
    while True:
        grown = owned | {pid for pid, row in rows.items() if row[0] in owned}
        if owned == grown:
            break
        owned = grown
    return {pid: rows[pid] for pid in owned if pid in rows}


def global_sample(config, owned, attempt=0, pending=None):
    """Fresh distinct trees plus accepted reservations; process absence is no release."""
    membership = json.loads(Path(config['membership_path']).read_text())
    released = set()
    for receipt in config.get('literal_releases', []):
        assert hashlib.sha256(Path(receipt['path']).read_bytes()).hexdigest() == receipt['sha256']
        released.add(receipt['cohort'])
    legacy_entries = []
    ordinary_identities = []
    published_identities = []
    for raw_entry in membership['entries']:
        entry = dict(raw_entry)
        entry.setdefault('cohort', entry.get('offerId'))
        entry.setdefault('owner', entry.get('ownerThreadId'))
        if 'knownRootMatchers' not in entry:
            controller = entry.get('controller') or {}
            identities = [controller, *(entry.get('guardControllers') or [])]
            entry['knownRootMatchers'] = [identity['script'] for identity in identities if identity.get('script')]
            if controller.get('outputRoot'):
                entry['knownRootMatchers'].append(controller['outputRoot'])
        if entry.get('kind') == 'ordinary':
            ordinary_identities.extend([entry.get('controller') or {}, *(entry.get('guardControllers') or [])])
        published_identities.extend([entry.get('controller') or {}, *(entry.get('guardControllers') or [])])
        legacy_entries.append(entry)
    cohorts = {entry['cohort']: entry for entry in legacy_entries
               if entry.get('kind') != 'ordinary' and entry['state'] not in ('FULL', 'DECLINED') and not entry['state'].startswith(('LITERAL_FULL', 'DECLINED')) and entry['cohort'] not in released
               and entry['cohort'] not in config.get('own_cohort_names', [])
               and config['control'] not in entry.get('knownRootMatchers', [])}
    for entry in config.get('additional_cohorts', []):
        if entry['cohort'] not in released:
            cohorts[entry['cohort']] = entry
    receipts = sorted(Path(config['receipt_directory']).glob('*.json'))
    assert len(receipts) <= 128, 'Bounded owner receipt inventory'
    receipt_controllers = {}
    owner_records = {}
    for file in receipts:
        assert file.stat().st_size <= 128 * 1024, 'Bounded owner receipt bytes'
        entry = json.loads(file.read_text())
        if 'ownerThreadId' not in entry:
            observation = entry.get('ordinaryObservation') or {}
            assert observation.get('ordinaryCommandsComplete') is True and observation.get('commandExit') == 0, 'Unresolved noncanonical owner metadata'
            # Completed ordinary observation cannot release an accepted legacy cohort.
            # Legacy reservations and actual unknown-root detection remain active.
            continue
        published_identities.extend([entry.get('controller') or {}, *(entry.get('guardControllers') or [])])
        owner, offer = entry['ownerThreadId'], entry['offerId']
        owner_records[owner] = entry
        if entry.get('kind') == 'ordinary':
            ordinary_identities.extend([entry.get('controller') or {}, *(entry.get('guardControllers') or [])])
        for key, legacy in list(cohorts.items()):
            legacy_clock = legacy.get('actualAccept', legacy.get('acceptedAtEpoch'))
            same_clock = isinstance(legacy_clock, (int, float)) and legacy_clock == entry.get('acceptedAtEpoch')
            if legacy.get('owner') == owner and (same_clock or key == offer):
                del cohorts[key]
        if entry.get('kind') == 'ordinary' or not entry['state'].startswith('ACCEPTED') or (owner == config['owner_thread'] and offer == config['lease']):
            continue
        key = owner + ':' + offer
        controller = entry.get('controller') or {}
        identities = [controller, *(entry.get('guardControllers') or [])]
        receipt_controllers[key] = identities
        roots = [identity['script'] for identity in identities if identity.get('script')]
        if controller.get('outputRoot'):
            roots.append(controller['outputRoot'])
        cohorts[key] = dict(cohort=key, owner=owner, kind=entry['kind'], processCap=entry['processCap'],
                            rssCapGiB=entry['rssCapBytes'] / GIB, knownRootMatchers=roots)
    qualified_control_roots = []
    for authority in config.get('factual_control_packet_owners', []):
        record = owner_records.get(authority['owner'], {})
        if not record.get('state', '').startswith('ACCEPTED'):
            continue
        assert record['kind'] == authority['kind']
        if cohorts[authority['owner'] + ':' + record['offerId']]['knownRootMatchers']:
            continue  # Existing producer-declared control roots need no optional packet adapter.
        packet_path = Path(record['receiptPaths']['accept'])
        assert packet_path.is_absolute() and str(packet_path).startswith(authority['receipt_prefix'])
        assert packet_path.stat().st_size <= 128 * 1024
        raw_packet = packet_path.read_bytes(); packet = json.loads(raw_packet)
        assert packet['ownerThreadId'] == authority['owner'] and packet['offerId'] == record['offerId']
        assert packet['accepted'] == record['acceptedAtEpoch'] and packet['kind'] == record['kind']
        assert packet['caps']['processCap'] == record['processCap'] and packet['caps']['rssCap'] == record['rssCapBytes']
        cohort = cohorts[authority['owner'] + ':' + record['offerId']]
        for name in ('guardian', 'run'):
            control = packet['controlCopies'][name]
            path = Path(control['path'])
            assert path.is_absolute() and str(path).startswith(authority['control_prefix']) and path.name == name + '.py'
            assert hashlib.sha256(path.read_bytes()).hexdigest() == control['sha256']
            cohort['knownRootMatchers'].append(str(path))
            qualified_control_roots.append(dict(cohort=cohort['cohort'], accepted=record['acceptedAtEpoch'], receiptSHA256=hashlib.sha256(raw_packet).hexdigest(), **control))
    raw = subprocess.check_output(['/bin/ps', '-axo', 'pid=,ppid=,rss=,lstart=,args='], text=True, timeout=4)
    rows = {}
    for line in raw.splitlines():
        parts = line.strip().split(None, 8)
        if len(parts) == 9:
            rows[int(parts[0])] = (int(parts[1]), int(parts[2]) * 1024, parts[8], ' '.join(parts[3:8]))

    def descendants(seeds):
        result = set(seeds)
        while True:
            grown = result | {pid for pid, row in rows.items() if row[0] in result}
            if grown == result:
                return result & rows.keys()
            result = grown

    # Expand the owned tree in this same current PS table: workers can spawn
    # after the earlier local sample, before this global sample.
    observed = descendants(set(owned))
    covered = set(observed)
    for key, entry in cohorts.items():
        identities = receipt_controllers.get(key, [])
        matches = {identity['pid'] for identity in identities if identity.get('pid') in rows and rows[identity['pid']][3] == identity.get('birth') and rows[identity['pid']][2].startswith(identity.get('executable', '\0'))}
        matches |= {pid for pid, row in rows.items() if any(root in row[2] for root in entry['knownRootMatchers'])}
        ids = descendants(matches)
        observed |= ids
        covered |= ids
    # A literal FULL can precede the controller's own exit by milliseconds.
    # Account its exact current identity and all actual descendants, but cover
    # only that controller; an unregistered workload child remains unknown.
    published_roots = {identity['pid'] for identity in published_identities
                       if identity.get('pid') in rows
                       and rows[identity['pid']][3] == ' '.join(identity.get('birth', '').split())
                       and identity.get('executable') and rows[identity['pid']][2].startswith(identity['executable'])
                       and identity.get('script') and identity['script'] in rows[identity['pid']][2]}
    observed |= descendants(published_roots)
    covered |= published_roots
    # Exact factual ordinary identities count actual PID/RSS without acquiring
    # an execution cohort. Only their identified process and Git/PS/metadata
    # children are covered; an app/compiler/test interpreter still fails closed.
    ordinary_roots = {identity['pid'] for identity in ordinary_identities
                      if identity.get('pid') in rows
                      and rows[identity['pid']][3] == ' '.join(identity.get('birth', '').split())
                      and identity.get('executable')
                      and rows[identity['pid']][2].startswith(identity['executable'])
                      and identity.get('script') and identity['script'] in rows[identity['pid']][2]}
    ordinary_tree = descendants(ordinary_roots)
    observed |= ordinary_tree
    covered |= ordinary_roots | {pid for pid in ordinary_tree
                                  if Path(rows[pid][2].split(' ', 1)[0]).name in ('git', 'ps')
                                  or ' metadata ' in rows[pid][2] and Path(rows[pid][2].split(' ', 1)[0]).name == 'cargo'}
    for ordinary in config.get('ordinary_git_only', []):
        matches = {pid for pid, row in rows.items() if ordinary['script'] in row[2]}
        if not matches:
            continue
        assert hashlib.sha256(Path(ordinary['script']).read_bytes()).hexdigest() == ordinary['sha256']
        for dependency, expected in ordinary.get('dependencies', {}).items():
            assert hashlib.sha256(Path(dependency).read_bytes()).hexdigest() == expected
        ids = descendants(matches)
        observed |= ids
        covered |= ids
    seeds = set()
    for pid, row in rows.items():
        command = row[2]
        executable = command.split(' ', 1)[0].lower()
        if any(tool in executable for tool in ['python', '/node', '/cargo', '/rustc', 'google']) and any(root in command for root in ['/mcr/', '/mcr-', 'mcr-pi']):
            seeds.add(pid)
    unknown = seeds - covered
    roots = {pid for pid in unknown if rows[pid][0] not in unknown}
    pending = dict(pending or {})
    for pid in roots:
        pending[(pid, rows[pid][3])] = dict(pid=pid, parent=rows[pid][0], rss=rows[pid][1], command=rows[pid][2], birth=rows[pid][3])
    unresolved = {key: origin for key, origin in pending.items()
                  if not any(identity.get('pid') == origin['pid']
                             and ' '.join(identity.get('birth', '').split()) == origin['birth']
                             and identity.get('executable') and origin['command'].startswith(identity['executable'])
                             and identity.get('script') and identity['script'] in origin['command']
                             for identity in published_identities)}
    if roots or unresolved:
        write(Path(config['control']) / f'unknown-{os.getpid()}-{time.time_ns()}.json', dict(epoch=time.time(), sampler=os.getpid(), attempt=attempt,
              owned_before=sorted(owned), owned_current=sorted(descendants(set(owned))), cohorts=list(cohorts),
              identities=list(pending.values()), unresolved=list(unresolved.values())))
        actual_pending = observed | descendants(roots)
        assert len(actual_pending) + 2 <= 96 and sum(rows[pid][1] for pid in actual_pending) + 24 * MIB <= 18 * GIB
        if attempt < 2:
            # Two fresh registry/PS rereads bound atomic bootstrap-publication
            # races to400ms. Fixed WORK/FULL clocks continue. Process absence
            # alone never resolves an original unknown identity.
            time.sleep(.2)
            return global_sample(config, owned, attempt + 1, pending)
    assert not roots and not unresolved, f'Unresolved live MCR workflow roots: {sorted({key[0] for key in unresolved} | roots)}'
    counts = dict(workflows=1, source=int(config['mode'] == 'source'), browsers=int(config['mode'] == 'native'), target=0)
    declared_pids, declared_rss = config['process_cap'], config['rss_cap']
    for entry in cohorts.values():
        kind = entry.get('kind') or ('browser' if 'native' in entry['cohort'].lower() else 'target' if 'compile' in entry['cohort'].lower() else 'source')
        counts['workflows'] += 1
        counts[dict(browser='browsers', target='target', source='source', **{'copied-native': 'source'})[kind]] += 1
        declared_pids += entry['processCap']
        declared_rss += entry['rssCapBytes'] if 'rssCapBytes' in entry else entry['rssCapGiB'] * GIB
    rss = sum(rows[pid][1] for pid in observed) + 24 * MIB
    assert counts['workflows'] <= 4 and counts['source'] <= 3 and counts['browsers'] <= 2 and counts['target'] <= 1, counts
    assert declared_pids <= 96 and declared_rss <= 18 * GIB, 'Declared aggregate envelope'
    assert len(observed) + 2 <= 96 and rss <= 18 * GIB, 'Observed distinct PID/RSS aggregate'
    return dict(epoch=time.time(), membership_asof=membership['asOfUTC'], cohorts=list(cohorts), pids=sorted(observed),
                declared_pids=declared_pids, declared_rss=declared_rss, rss=rss, qualified_control_identities=[dict(pid=pid, birth=row[3], command=row[2], cohort=control['cohort'], accepted=control['accepted'], receiptSHA256=control['receiptSHA256'], controlSHA256=control['sha256']) for pid, row in rows.items() for control in qualified_control_roots if control['path'] in row[2]], **counts)


class Meter:
    def __init__(self, config, controller):
        self.config, self.controller = config, controller
        self.initial = inventory([ROOT, PRIVATE_GIT, COMMON_GIT, Path(config['temporary']), Path(config['control']), Path(config['owner_receipt'])])
        self.maximum = dict(self.initial)
        self.carry = max(config['prior_whole'], sum(inventory(CARRY + [Path(config['control']), Path(config['owner_receipt'])] + [Path(p) for p in config.get('retained_roots', [])]).values()))
        self.peak = dict(whole=self.carry, new=0, processes=0, rss=0, native_payload=config.get('prior_native_payload', 0))
        self.minimum_free = free()
        self.seen = {}
        self.last_global = None

    def check(self, final=False):
        cfg = self.config
        current = inventory([ROOT, PRIVATE_GIT, COMMON_GIT, Path(cfg['temporary']), Path(cfg['control']), Path(cfg['owner_receipt'])])
        for key, size in current.items():
            self.maximum[key] = max(self.maximum.get(key, 0), size)
        positive = cfg.get('historical_new_charge', 0) + sum(max(0, size - self.initial.get(key, 0)) for key, size in self.maximum.items()) + 34 * MIB
        live = tree(self.controller, self.seen)
        self.seen.update({pid: (row[3], row[4]) for pid, row in live.items() if Path(row[3]).name != 'ps'})
        sample = dict(whole=max(self.carry, sum(inventory(CARRY + [Path(cfg['temporary']), Path(cfg['control']), Path(cfg['owner_receipt'])] + [Path(p) for p in cfg.get('retained_roots', [])]).values())),
                      new=positive, processes=len(live) + 2, rss=sum(row[2] for row in live.values()) + 24 * MIB)
        for key, value in sample.items():
            self.peak[key] = max(self.peak[key], value)
        if cfg['mode'] == 'native':
            proof = inventory([Path(cfg['control']), Path(cfg['output'])] + [Path(root) for root in cfg.get('retained_native_payload_roots', [])])
            self.peak['native_payload'] = max(self.peak.get('native_payload', 0), sum(proof.values()))
            assert sum(proof.values()) <= cfg.get('native_payload_cap', 80 * MIB), 'Combined native cache/TMP/log/capture cap'
            pngs = list(Path(cfg['output']).rglob('*.png'))
            for root in cfg.get('retained_native_png_roots', []):
                pngs.extend(Path(root).rglob('*.png'))
            assert len(pngs) <= cfg.get('native_png_cap', 40), 'Native PNG count cap'
            assert sum(file.stat().st_size for file in pngs) <= cfg.get('native_png_byte_cap', 16 * MIB), 'Native PNG byte cap'
            self.peak['native_payload'] = max(self.peak.get('native_payload', 0), sum(proof.values()))
        self.minimum_free = min(self.minimum_free, free())
        assert free() >= 8 * GIB, 'live free floor'
        assert time.time() < cfg['full_deadline' if final else 'work_deadline'], 'fixed final/work deadline'
        assert sample['whole'] <= cfg['whole_cap'], 'whole cap'
        assert sample['new'] <= cfg['new_cap'], 'new cap'
        assert sample['processes'] <= cfg['process_cap'], 'process cap'
        assert sample['rss'] <= cfg['rss_cap'], 'RSS cap'
        self.last_global = global_sample(cfg, live)
        for key in ['rss', 'workflows', 'source', 'browsers', 'target']:
            peak_key = 'global_' + key
            self.peak[peak_key] = max(self.peak.get(peak_key, 0), self.last_global[key])
        self.peak['global_processes'] = max(self.peak.get('global_processes', 0), len(self.last_global['pids']) + 2)
        return sample


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + '\n')


def publish_receipt(config, state, guard_pid=None):
    path = Path(config['owner_receipt'])
    previous = json.loads(path.read_text()) if path.exists() else {}
    identities = []
    for pid, script in [(os.getpid() if guard_pid is None else int(os.environ['PI10_CONTROLLER']), 'controller.py')]:
        birth = ' '.join(subprocess.check_output(['/bin/ps', '-p', str(pid), '-o', 'lstart='], text=True).split())
        executable = subprocess.check_output(['/bin/ps', '-p', str(pid), '-o', 'comm='], text=True).strip()
        actual_script = Path(__file__).with_name(script) if guard_pid is not None else Path(sys.argv[0]).resolve()
        controller = dict(pid=pid, birth=birth, executable=executable, script=str(actual_script), cwd=os.getcwd(), outputRoot=config['output'])
    if guard_pid is not None:
        identities = [dict(pid=guard_pid, birth=' '.join(subprocess.check_output(['/bin/ps', '-p', str(guard_pid), '-o', 'lstart='], text=True).split()),
                           executable=subprocess.check_output(['/bin/ps', '-p', str(guard_pid), '-o', 'comm='], text=True).strip(), script=str(Path(__file__).with_name('guardian.py')), cwd=os.getcwd(), outputRoot=config['output'])]
    elif previous.get('offerId') == config['lease']:
        identities = previous.get('guardControllers', [])
    result = dict(ownerThreadId=config['owner_thread'], offerId=config['lease'], state=state, acceptedAtEpoch=config['accept_epoch'],
                  workDeadlineEpoch=config['work_deadline'], fullDeadlineEpoch=config['full_deadline'], kind='browser' if config['mode'] == 'native' else 'source',
                  processCap=config['process_cap'], rssCapBytes=config['rss_cap'], remainingNewUpperBytes=config.get('remaining_new_reservation', config['new_cap']) if state == 'ACCEPTED' else 0,
                  controller=controller, guardControllers=identities, receiptPaths=dict(accept=str(Path(config['control']) / 'offer.json'), full=str(Path(config['control']) / 'outer-full.json')),
                  observedAtEpoch=time.time(), boundary='Receipt metadata only; no new resource authorization')
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(result, indent=2) + '\n')
    os.replace(temporary, path)
