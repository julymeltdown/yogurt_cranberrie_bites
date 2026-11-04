"""FE17 original completion bounded source qualification. Adapted from own AR03 supervisor.
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
BASE = 'ff96d53eb817c93dd1bafb5c4f125630f995413b'
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
NODE_SHA = 'b6fb419f0f43bb2fbca273700294dd2f3b685a64b60e8417c295f47646a70e4a'
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
EVIDENCE = ROOT / 'docs/mcr/evidence/fe17-original-completion'
PREFIX = 'frontend/src/app/character-entry/__tests__/reviewed-production/default-entry/'
EXACT = {'frontend/src/world/network/replication.ts', 'frontend/src/world/entities/replicated.ts',
 'frontend/src/world/network/__tests__/replication.test.ts', 'frontend/src/world/network/__tests__/replication-browser.mjs',
 'frontend/src/world/network/__tests__/fixtures/replication-browser.html', 'frontend/src/world/network/__tests__/fixtures/replication-browser.ts',
 'frontend/src/world/network/prediction.ts', 'frontend/src/world/network/__tests__/prediction.test.ts',
 'docs/mcr/frontend/fe17-original-completion.md',
 'frontend/src/world/network/WorldTransport.ts', 'frontend/src/world/network/transport/engine.ts',
 'frontend/src/world/network/transport/contract.ts', 'frontend/src/world/composition/index.ts', 'frontend/src/app/world-entry/production.ts'}
LOCK = '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'

def git(*args):
    return sp.check_output(['git', *args], cwd=ROOT, text=True).rstrip('\n')

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

_history_cache = {}
def root_inventory(root):
    total = files = symlinks = 0
    for directory, dirs, names in os.walk(root, followlinks=False):
        for name in names:
            p = Path(directory) / name
            try:
                total += p.lstat().st_size; files += 1; symlinks += p.is_symlink()
            except FileNotFoundError:
                pass
        for name in dirs:
            p = Path(directory) / name
            if p.is_symlink():
                total += p.lstat().st_size; symlinks += 1
    return {'root': str(root), 'logicalBytes': total, 'files': files, 'symlinks': symlinks}

def carry_inventory():
    # Entire materialized checkout, entire borrowed installation, and all reachable
    # current + preserved AR03 Git history. No selected-prefix accounting discount.
    head = git('rev-parse', 'HEAD')
    if head not in _history_cache:
        objects = {row.split()[0] for row in git('rev-list', '--objects', head,
                   'defdc8fc429e658c7f53523304208056477c6a88').splitlines()}
        sizes = sp.check_output(['git', 'cat-file', '--batch-check=%(objectsize)'], cwd=ROOT,
                                input='\n'.join(sorted(objects)) + '\n', text=True)
        _history_cache[head] = (sum(int(line) for line in sizes.splitlines()), len(objects))
    history, objects = _history_cache[head]
    roots = [root_inventory(ROOT), root_inventory(DEPS), root_inventory(Path(git('rev-parse', '--absolute-git-dir')))]
    return {'roots': roots, 'reachableGitLogicalBytes': history, 'reachableGitObjects': objects,
            'wholeCarryBytes': sum(r['logicalBytes'] for r in roots) + history,
            'chargedFiles': sum(r['files'] + r['symlinks'] for r in roots) + objects}

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
    slot, pin, mode = os.environ['MCR_FE17_SLOT'], os.environ['MCR_FE17_PIN'], os.environ['MCR_FE17_MODE']
    deadline = dt.datetime.fromisoformat(os.environ['MCR_FE17_DEADLINE']).timestamp()
    output = EVIDENCE / os.environ['MCR_FE17_ATTEMPT']
    assert mode == 'source' and slot and Path.cwd() == ROOT
    assert git('rev-parse', 'HEAD') == pin
    assert hashlib.sha256(NODE.read_bytes()).hexdigest() == NODE_SHA
    assert not output.exists() and output.parent == EVIDENCE
    assert 60 < deadline - time.time() <= 240
    assert hashlib.sha256((ROOT / 'frontend/pnpm-lock.yaml').read_bytes()).hexdigest() == LOCK
    # A retry retains the admitted original clock/caps and the continuous8GiB floor.
    # Fresh frontend source offers require12GiB; never silently start a new allocation as a retry.
    previous = os.environ.get('MCR_FE17_PREVIOUS_ATTEMPT')
    floor = 12 * 1024**3
    if previous:
        prior_path = EVIDENCE / previous / 'supervisor.json'
        assert prior_path.parent.parent == EVIDENCE
        prior = json.loads(prior_path.read_text())
        assert prior['slot'] == slot and prior['deadlineUTC'] == os.environ['MCR_FE17_DEADLINE'] and prior['mode'] == mode
        floor = 8 * 1024**3
    assert shutil.disk_usage(ROOT).free >= floor
    readonly_file = EVIDENCE / 'motion-readonly-inputs.json'
    readonly = json.loads(readonly_file.read_text())['files'] if readonly_file.exists() else []
    readonly_pins = {entry['path']: entry for entry in readonly}
    for path, entry in readonly_pins.items():
        assert hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == entry['sha256']
        source = sp.check_output(['git', 'show', entry['commit'] + ':' + path], cwd=ROOT)
        assert hashlib.sha256(source).hexdigest() == entry['sha256']
    changed = git('diff', '--name-only', BASE).splitlines()
    assert all(path in EXACT or path in readonly_pins or path.startswith('frontend/src/world/network/__tests__/motion-transport/') or path.startswith('docs/mcr/evidence/fe17-original-completion/') for path in changed)
    assert all(row[3:] in readonly_pins or row[3:].startswith('docs/mcr/evidence/fe17-original-completion/') for row in git('status', '--porcelain', '--untracked-files=all').splitlines())
    link = ROOT / 'frontend/node_modules'
    assert not link.exists() and not link.is_symlink()
    borrowed_temp = DEPS / '.vite-temp'
    assert not borrowed_temp.exists() or not list(borrowed_temp.iterdir())
    inventory = carry_inventory()
    carry = inventory['wholeCarryBytes']
    caps = {'whole': 4 * 1024**3, 'temporary': 96 * 1024**2,
            'durable': 32 * 1024**2, 'rss': int(2.5 * 1024**3), 'processes': 8, 'files': 65536, 'new': 128 * 1024**2}
    assert carry < caps['whole'] and inventory['chargedFiles'] < caps['files']
    other_reserve = int(os.environ.get('MCR_FE17_OTHER_RESERVE', '0'))
    assert other_reserve >= 0 and shutil.disk_usage(ROOT).free - other_reserve - caps['new'] >= 9 * 1024**3
    output.mkdir(parents=True)
    tmp = Path(tempfile.mkdtemp(prefix='mcr-fe17-original-', dir='/private/tmp'))
    env = {**os.environ, 'PATH': str(NODE.parent) + ':' + os.environ.get('PATH', ''),
           'TMPDIR': str(tmp), 'TMP': str(tmp), 'TEMP': str(tmp), 'XDG_CACHE_HOME': str(tmp / 'cache'),
           'MCR_FRONTEND_DEPENDENCIES': str(DEPS.parent), 'MCR_FE17_CACHE': str(tmp / 'vitest')}
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
    config = EVIDENCE / 'vitest.config.mjs'
    vitest = [str(NODE), str(DEPS / 'vitest/vitest.mjs'), 'run', '--configLoader', 'runner', '--root', str(ROOT / 'frontend'), '--config', str(config)]
    commands = [
        ('runtime', [str(NODE), '--version'], 0),
        ('published-fixture-delayed-red', vitest, 1),
        ('lifecycle-delayed-green', vitest, 0),
        ('types', [str(NODE), '--input-type=module', '--eval', type_invocation], 0),
        ('lint', [str(NODE), str(DEPS / 'oxlint/bin/oxlint'), str(ROOT / 'frontend/src/world/network/__tests__/replication.test.ts'), str(ROOT / 'frontend/src/world/network/prediction.ts'), str(ROOT / 'frontend/src/world/network/__tests__/prediction.test.ts'), str(config), str(EVIDENCE / 'delayed-digest.ts')], 0),
        ('config-syntax', [str(NODE), '--check', str(config)], 0),
        ('whitespace', ['git', 'diff', '--check', BASE, '--', '.', ':(glob,exclude)docs/mcr/evidence/fe17-original-completion/**/*.log'], 0),
        ('plan', ['python3', str(ROOT / 'scripts/mcr/check-plan.py')], 0),
    ]
    motion_round = os.environ.get('MCR_FE17_MOTION') == '1'
    if motion_round:
        assert readonly and not os.environ.get('MCR_FE17_CONTINUE_REMAINING')
        # Changed prediction requires new green checks; the established RED fixture
        # race is preserved, not replayed as an unrelated publication prerequisite.
        commands = [command for command in commands if command[0] != 'published-fixture-delayed-red']
        commands[1] = ('motion-delayed-green', vitest, 0)
        lint_paths = [path for path in changed if path not in readonly_pins and path.startswith('frontend/') and (path.endswith('.ts') or path.endswith('.mjs'))]
        commands = [(label, [str(NODE), str(DEPS / 'oxlint/bin/oxlint'), '--deny-warnings', *[str(ROOT / path) for path in sorted(lint_paths)], str(config), str(EVIDENCE / 'delayed-digest.ts')], expected)
                    if label == 'lint' else (label, argv, expected) for label, argv, expected in commands]
        commands.insert(-2, ('browser-syntax', [str(NODE), '--check', str(ROOT / 'frontend/src/world/network/__tests__/replication-browser.mjs')], 0))
        if os.environ.get('MCR_FE17_RETRY_CANCELLATION') == '1':
            assert previous
            prior = json.loads((EVIDENCE / previous / 'supervisor.json').read_text())
            assert [(c['label'], c['exit']) for c in prior['commands']] == [('runtime', 0), ('motion-delayed-green', 1)]
            raw = (EVIDENCE / previous / 'motion-delayed-green.log').read_text()
            assert '2 failed | 229 passed (231)' in raw and 'expected "digest" to be called 1 times, but got 2 times' in raw
        if os.environ.get('MCR_FE17_RETRY_TYPES') == '1':
            assert previous
            prior = json.loads((EVIDENCE / previous / 'supervisor.json').read_text())
            assert [(c['label'], c['exit']) for c in prior['commands']] == [('runtime', 0), ('motion-delayed-green', 0), ('types', 1)]
            raw = (EVIDENCE / previous / 'types.log').read_text()
            assert 'Promise<ShellBootstrap>' in raw and 'network/transport/contract.ts' in raw and '"diagnostics":2' in raw
    if os.environ.get('MCR_FE17_RETRY_REMAINDER') == '1':
        assert previous and motion_round
        prior = json.loads((EVIDENCE / previous / 'supervisor.json').read_text())
        assert [(c['label'], c['exit']) for c in prior['commands']] == [('runtime', 0), ('motion-delayed-green', 0), ('types', 0), ('lint', 0), ('config-syntax', 0), ('browser-syntax', 0), ('whitespace', 2)]
        raw = (EVIDENCE / previous / 'whitespace.log').read_text()
        assert all(line.startswith('docs/mcr/evidence/fe17-original-completion/') and '.log:' in line and 'new blank line at EOF' in line for line in raw.splitlines())
        # Raw historical logs are immutable evidence, not engineering source.
        # Preserve their bytes and check all source/docs/JSON without this artifact-only false positive.
        commands = [command for command in commands if command[0] in ['whitespace', 'plan']]
    continuation = os.environ.get('MCR_FE17_CONTINUE_REMAINING') == '1'
    if continuation:
        assert previous
        prior = json.loads((EVIDENCE / previous / 'supervisor.json').read_text())
        assert [c['label'] for c in prior['commands']] == ['runtime', 'published-fixture-delayed-red', 'lifecycle-delayed-green', 'types']
        assert [c['exit'] for c in prior['commands']] == [0, 1, 0, 1]
        raw = (EVIDENCE / previous / 'types.log').read_text()
        assert 'gift-detail-media/native/preview.tsx' in raw and '"diagnostics":2' in raw
        commands = commands[4:]
    report = {'slot': slot, 'pin': pin, 'base': BASE, 'mode': mode, 'startedUTC': dt.datetime.now(dt.timezone.utc).isoformat(),
              'deadlineUTC': os.environ['MCR_FE17_DEADLINE'], 'caps': caps, 'inventory': inventory, 'carryBytes': carry, 'newPeak': 0,
              'commands': [], 'wholePeak': carry, 'temporaryPeak': 0, 'durablePeak': 0, 'rssPeak': 0, 'processPeak': 0, 'sourcePins': {}, 'readonlyInputs': readonly, 'otherReserveBytes': other_reserve}
    child, owned, status = None, set(), 'FAIL'
    try:
        for label, argv, expected_exit in commands:
            child_env = {**env, 'MCR_FE17_BASELINE': '1' if label == 'published-fixture-delayed-red' else '0'}
            assert time.time() < deadline - 60 and git('rev-parse', 'HEAD') == pin
            with (output / (label + '.log')).open('wb') as log:
                child = sp.Popen(argv, cwd=ROOT, env=child_env, stdout=log, stderr=sp.STDOUT, start_new_session=True)
                record = {'label': label, 'argv': argv, 'pid': child.pid, 'startedEpoch': time.time(), 'expectedExit': expected_exit}; report['commands'].append(record); owned.add(child.pid)
                while child.poll() is None:
                    rows = processes(); ids = descendants(os.getpid(), rows); owned.update(ids - {os.getpid()})
                    temporary, durable = bytes_in(tmp), bytes_in(output)
                    # Carry already includes pre-run evidence; add only this attempt's new output.
                    rss, free = sum(r * 1024 for p, _, _, r in rows if p in ids), shutil.disk_usage(ROOT).free
                    whole = carry + temporary + durable
                    report['newPeak'] = max(report['newPeak'], temporary + durable)
                    report['wholePeak'] = max(report['wholePeak'], whole); report['temporaryPeak'] = max(report['temporaryPeak'], temporary)
                    report['durablePeak'] = max(report['durablePeak'], durable); report['rssPeak'] = max(report['rssPeak'], rss)
                    report['processPeak'] = max(report['processPeak'], len(ids)); report['minimumFree'] = min(report.get('minimumFree', free), free)
                    if (time.time() >= deadline - 60 or time.time() - record['startedEpoch'] >= 90 or whole > caps['whole']
                        or temporary + durable > caps['new'] or temporary > caps['temporary'] or durable > caps['durable'] or rss > caps['rss'] or len(ids) > caps['processes'] or free < 8 * 1024**3 or free - other_reserve - caps['new'] < 9 * 1024**3):
                        raise RuntimeError('Original deadline/child/whole/TMP/durable/RSS/process/free bound reached')
                    time.sleep(.25)
                record.update(exit=child.returncode, endedEpoch=time.time())
                if child.returncode != expected_exit:
                    raise RuntimeError(label + ' unexpected exit; original raw evidence retained')
                if label == 'published-fixture-delayed-red':
                    red = (output / (label + '.log')).read_text()
                    assert "Cannot read properties of undefined (reading 'open')" in red, 'Red did not establish the expected admission fixture race'
                    assert 'Tests' in red and 'failed' in red and 'Cannot find package' not in red and 'Failed to load' not in red
                if label == 'runtime':
                    assert (output / 'runtime.log').read_text().strip() == 'v24.22.2'
                retire(child)
        final_inventory = carry_inventory()
        report['finalInventory'] = final_inventory
        assert final_inventory['wholeCarryBytes'] < caps['whole'] and final_inventory['chargedFiles'] < caps['files']
        assert max(0, final_inventory['wholeCarryBytes'] - carry) < caps['new']
        report['sourcePins'] = {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in changed if (ROOT / path).is_file()}
        status = 'PARTIAL_FULL_APPLICATION_TYPES_FAILED' if continuation else 'PASS'
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
