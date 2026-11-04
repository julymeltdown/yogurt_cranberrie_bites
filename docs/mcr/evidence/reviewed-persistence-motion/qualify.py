"""Original AR03 narrow continuation. Adapted from Root7eac13ce combined supervisor.
No executable admission without a fresh explicit allocator OFFER and ACCEPT.
"""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess as sp
import tempfile
import time

ROOT = Path('/Users/lhs/Desktop/mcr-worktrees/fe-ref-map')
BASE = '7eac13ce30d6dd6be4df30727f75d5df90d3489d'
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
EVIDENCE = ROOT / 'docs/mcr/evidence/reviewed-persistence-motion'
PREFIX = 'frontend/src/app/character-entry/__tests__/reviewed-production/default-entry/'
EXACT = {PREFIX + name for name in ['native.mjs', 'preview.ts', 'preview.html']} | {'docs/mcr/frontend/reviewed-persistence-motion.md'}
LOCK = '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'

def git(*args):
    return sp.check_output(['git', *args], cwd=ROOT, text=True).strip()

def bytes_in(root):
    total = 0
    if not root.exists():
        return total
    for directory, _, files in os.walk(root, followlinks=False):
        for name in files:
            try:
                total += (Path(directory) / name).lstat().st_size
            except FileNotFoundError:
                pass
    return total

def source_git_bytes():
    objects = {line.split()[0] for line in git('rev-list', '--objects', BASE + '..HEAD').splitlines()}
    sizes = sp.check_output(['git', 'cat-file', '--batch-check=%(objectsize)'], cwd=ROOT,
                            input='\n'.join(sorted(objects)) + '\n', text=True) if objects else ''
    return sum(int(line) for line in sizes.splitlines()) + bytes_in(Path(git('rev-parse', '--absolute-git-dir')))

def carry_inventory():
    # Whole retained own source/art/test/history/failure evidence, not just new delta.
    prefixes = [
        PREFIX,
        'frontend/src/entities/avatar/runtime/motion-adoption/',
        'frontend/src/entities/avatar/runtime/__tests__/reviewed-production/',
        'frontend/src/features/avatar/motion-adoption/',
        'frontend/src/features/avatar/part-controls/',
        'frontend/src/features/avatar/wardrobe-options/',
        'frontend/src/features/avatar/__tests__/reviewed-production/',
        'frontend/src/world/avatar/motion-adoption/',
        'frontend/src/world/avatar/__tests__/motion-adoption/',
        'frontend/src/app/character-entry/__tests__/motion-adoption/',
        'docs/mcr/art/evidence/motion-candidates-v1/',
        'docs/mcr/evidence/avatar-motion-adoption/',
        'docs/mcr/evidence/customize-reviewed-production/',
        'docs/mcr/evidence/avatar-part-controls/',
        'docs/mcr/evidence/reviewed-persistence-motion/',
    ]
    result = {path: bytes_in(ROOT / path) for path in prefixes}
    for path in ['docs/mcr/frontend/ar03-motion-adoption.md', 'docs/mcr/frontend/customize-reviewed-production.md',
                 'docs/mcr/frontend/avatar-part-controls.md', 'docs/mcr/frontend/reviewed-persistence-motion.md']:
        p = ROOT / path
        result[path] = p.stat().st_size if p.exists() else 0
    result['sourceGit'] = source_git_bytes()
    return result

def processes():
    return [tuple(map(int, row.split())) for row in sp.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True).splitlines() if row.strip()]

def descendants(pid, rows):
    found = {pid}
    while True:
        expanded = found | {p for p, parent, _, _ in rows if parent in found}
        if expanded == found:
            return found
        found = expanded

def retire(child):
    if child is None:
        return
    for sig in [signal.SIGTERM, signal.SIGKILL]:
        if any(group == child.pid for _, _, group, _ in processes()):
            try:
                os.killpg(child.pid, sig)
            except ProcessLookupError:
                pass
        try:
            child.wait(timeout=3)
        except sp.TimeoutExpired:
            continue
        if not any(group == child.pid for _, _, group, _ in processes()):
            return
        time.sleep(.2)

def main():
    slot, pin, mode = os.environ['MCR_REVIEWED_PM_SLOT'], os.environ['MCR_REVIEWED_PM_PIN'], os.environ['MCR_REVIEWED_PM_MODE']
    deadline = dt.datetime.fromisoformat(os.environ['MCR_REVIEWED_PM_DEADLINE']).timestamp()
    output = EVIDENCE / os.environ['MCR_REVIEWED_PM_ATTEMPT']
    assert mode in ['source', 'browser'] and slot and Path.cwd() == ROOT
    assert git('rev-parse', 'HEAD') == pin
    assert not output.exists() and output.parent == EVIDENCE
    assert 90 < deadline - time.time() <= 600
    assert hashlib.sha256((ROOT / 'frontend/pnpm-lock.yaml').read_bytes()).hexdigest() == LOCK
    # A retry retains the admitted original clock/caps and the continuous8GiB floor.
    # Fresh offers still require16GiB; never silently start a new allocation as a retry.
    previous = os.environ.get('MCR_REVIEWED_PM_PREVIOUS_ATTEMPT')
    floor = (12 if mode == 'browser' else 16) * 1024**3
    if previous:
        prior_path = EVIDENCE / previous / 'supervisor.json'
        assert prior_path.parent.parent == EVIDENCE
        prior = json.loads(prior_path.read_text())
        assert prior['slot'] == slot and prior['deadlineUTC'] == os.environ['MCR_REVIEWED_PM_DEADLINE'] and prior['mode'] == mode
        floor = 8 * 1024**3
    assert shutil.disk_usage(ROOT).free >= floor
    changed = git('diff', '--name-only', BASE).splitlines()
    assert all(path in EXACT or path.startswith('docs/mcr/evidence/reviewed-persistence-motion/') for path in changed)
    assert all(row[3:].startswith('docs/mcr/evidence/reviewed-persistence-motion/') for row in git('status', '--porcelain', '--untracked-files=all').splitlines())
    link = ROOT / 'frontend/node_modules'
    assert not link.exists() and not link.is_symlink()
    borrowed_temp = DEPS / '.vite-temp'
    assert not borrowed_temp.exists() or not list(borrowed_temp.iterdir())
    inventory = carry_inventory()
    carry = sum(inventory.values())
    caps = {'whole': (288 if mode == 'browser' else 128) * 1024**2, 'temporary': 96 * 1024**2,
            'durable': 32 * 1024**2, 'rss': int((4 if mode == 'browser' else 2.5) * 1024**3),
            'processes': 48 if mode == 'browser' else 8}
    assert carry < caps['whole']
    output.mkdir(parents=True)
    tmp = Path(tempfile.mkdtemp(prefix='mcr-ar03-persistence-motion-', dir='/private/tmp'))
    env = {**os.environ, 'PATH': str(NODE.parent) + ':' + os.environ.get('PATH', ''),
           'TMPDIR': str(tmp), 'TMP': str(tmp), 'TEMP': str(tmp), 'XDG_CACHE_HOME': str(tmp / 'cache'),
           'MCR_FRONTEND_DEPENDENCIES': str(DEPS.parent), 'MCR_REVIEWED_NATIVE_SCRATCH': str(tmp / 'reviewed'),
           'MCR_REVIEWED_NATIVE_OUTPUT': str(output / 'native'), 'MCR_REVIEWED_PERSISTENCE_MOTION': '1'}
    driver = ROOT / PREFIX / 'native.mjs'
    # The offered invocation-only TS5097 accommodation keeps the tracked helper,
    # tsconfig, file set and all type checks intact. Only this invocation's option changes.
    helper = ROOT / 'frontend/src/world/avatar/__tests__/motion-adoption/typecheck.mjs'
    type_invocation = f"""import assert from 'node:assert/strict';
import {{readFileSync}} from 'node:fs';
import {{createHash}} from 'node:crypto';
import {{dirname}} from 'node:path';
const helper={json.dumps(str(helper))};
const original=readFileSync(helper,'utf8');
assert.equal(createHash('sha256').update(original).digest('hex'),'4cbcb57e175101a3fabbcbd67604c223d2604e24732f9d6e3331583a0d624aa1');
assert(original.includes('...parsed.options, incremental:'));
const source=original.replace('...parsed.options, incremental:','...parsed.options, allowImportingTsExtensions: true, incremental:')
  .replaceAll('import.meta.dirname',JSON.stringify(dirname(helper)));
process.argv.push('--application');
await import('data:text/javascript;base64,'+Buffer.from(source).toString('base64'));
"""
    commands = [('runtime', [str(NODE), '--version'])]
    if mode == 'source':
        commands += [
            ('syntax', [str(NODE), '--check', str(driver)]),
            ('types', [str(NODE), '--input-type=module', '--eval', type_invocation]),
            ('lint', [str(NODE), str(DEPS / 'oxlint/bin/oxlint'), str(driver), str(ROOT / PREFIX / 'preview.ts')]),
            ('whitespace', ['git', 'diff', '--check', BASE]),
            ('plan', ['python3', str(ROOT / 'scripts/mcr/check-plan.py')]),
        ]
    else:
        commands += [('native', [str(NODE), str(driver)])]
    report = {'slot': slot, 'pin': pin, 'base': BASE, 'mode': mode, 'startedUTC': dt.datetime.now(dt.timezone.utc).isoformat(),
              'deadlineUTC': os.environ['MCR_REVIEWED_PM_DEADLINE'], 'caps': caps, 'inventory': inventory, 'carryBytes': carry,
              'commands': [], 'wholePeak': carry, 'temporaryPeak': 0, 'durablePeak': 0, 'rssPeak': 0, 'processPeak': 0, 'sourcePins': {}}
    child, owned, status = None, set(), 'FAIL'
    try:
        if mode == 'browser':
            # Only a fresh browser allocation may authorize this transient read-only target link.
            assert os.environ['MCR_REVIEWED_PM_READONLY_LINK_ACCEPTED'] == '1'
            link.symlink_to(DEPS, target_is_directory=True)
        for label, argv in commands:
            assert time.time() < deadline - 60 and git('rev-parse', 'HEAD') == pin
            with (output / (label + '.log')).open('wb') as log:
                child = sp.Popen(argv, cwd=ROOT, env=env, stdout=log, stderr=sp.STDOUT, start_new_session=True)
                record = {'label': label, 'argv': argv, 'pid': child.pid, 'startedEpoch': time.time()}; report['commands'].append(record); owned.add(child.pid)
                while child.poll() is None:
                    rows = processes(); ids = descendants(os.getpid(), rows); owned.update(ids - {os.getpid()})
                    temporary, durable = bytes_in(tmp), bytes_in(output)
                    # Carry already includes pre-run evidence; add only this attempt's new output.
                    rss, free = sum(r * 1024 for p, _, _, r in rows if p in ids), shutil.disk_usage(ROOT).free
                    whole = carry + temporary + durable
                    report['wholePeak'] = max(report['wholePeak'], whole); report['temporaryPeak'] = max(report['temporaryPeak'], temporary)
                    report['durablePeak'] = max(report['durablePeak'], durable); report['rssPeak'] = max(report['rssPeak'], rss)
                    report['processPeak'] = max(report['processPeak'], len(ids)); report['minimumFree'] = min(report.get('minimumFree', free), free)
                    if (time.time() >= deadline - 60 or time.time() - record['startedEpoch'] >= 300 or whole > caps['whole']
                        or temporary > caps['temporary'] or durable > caps['durable'] or rss > caps['rss'] or len(ids) > caps['processes'] or free < 8 * 1024**3):
                        raise RuntimeError('Original deadline/child/whole/TMP/durable/RSS/process/free bound reached')
                    time.sleep(.2)
                record.update(exit=child.returncode, endedEpoch=time.time())
                if child.returncode:
                    raise RuntimeError(label + ' failed; original raw evidence retained')
                if label == 'runtime':
                    assert (output / 'runtime.log').read_text().strip() == 'v24.22.2'
                retire(child)
        report['sourcePins'] = {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in changed if (ROOT / path).is_file()}
        status = 'PASS'
    except BaseException as exc:
        report['failure'] = type(exc).__name__ + ': ' + str(exc)
    finally:
        retire(child)
        if link.is_symlink() and link.resolve() == DEPS:
            link.unlink()
        shutil.rmtree(tmp)
        alive = {p for p, _, _, _ in processes()}
        report['aliveOwnedPids'] = sorted(owned & alive)
        if report['aliveOwnedPids']:
            status = 'FAIL'
        report.update(status=status, finishedUTC=dt.datetime.now(dt.timezone.utc).isoformat(),
                      linkAbsent=not link.exists() and not link.is_symlink(), temporaryAbsent=not tmp.exists(),
                      borrowedViteTempEmpty=not borrowed_temp.exists() or not list(borrowed_temp.iterdir()), finalFree=shutil.disk_usage(ROOT).free)
        (output / 'supervisor.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2))
    return 0 if status == 'PASS' else 1

if __name__ == '__main__':
    raise SystemExit(main())
