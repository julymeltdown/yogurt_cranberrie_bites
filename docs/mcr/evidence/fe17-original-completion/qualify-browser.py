"""Fresh-allocation-only FE17 changed native WS round; reuses whole-carry accounting."""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import runpy
import shutil
import signal
import socket
import subprocess as sp
import tempfile
import time

ROOT = Path('/Users/lhs/Desktop/mcr-worktrees/fe-ref-map')
EVIDENCE = ROOT / 'docs/mcr/evidence/fe17-original-completion'
HELPER = runpy.run_path(str(EVIDENCE / 'qualify-source.py'), run_name='fe17_accounting')
NODE, DEPS = HELPER['NODE'], HELPER['DEPS']

def fingerprint(pid):
    result = sp.run(['ps', '-p', str(pid), '-o', 'lstart=', '-o', 'comm='], capture_output=True, text=True)
    return result.stdout.strip() if result.returncode == 0 else None

def main():
    slot = os.environ['MCR_FE17_SLOT']
    pin = os.environ['MCR_FE17_PIN']
    deadline_text = os.environ['MCR_FE17_DEADLINE']
    deadline = dt.datetime.fromisoformat(deadline_text).timestamp()
    assert slot and Path.cwd() == ROOT and HELPER['git']('rev-parse', 'HEAD') == pin
    assert hashlib.sha256(NODE.read_bytes()).hexdigest() == HELPER['NODE_SHA']
    assert 60 < deadline - time.time() <= 240
    previous_attempt = os.environ.get('MCR_FE17_PREVIOUS_ATTEMPT')
    if previous_attempt:
        previous = json.loads((EVIDENCE / previous_attempt / 'supervisor.json').read_text())
        assert previous['slot'] == slot and previous['deadlineUTC'] == deadline_text and previous['status'] == 'FAIL'
        assert not previous['aliveOwnedPids'] and previous['temporaryAbsent'] and previous['dependencyLinkAbsent']
        assert all(not Path(path).exists() for path in previous.get('externalSingletonTargets', []))
    output = EVIDENCE / os.environ['MCR_FE17_ATTEMPT']
    assert output.parent == EVIDENCE and not output.exists()
    readonly = json.loads((EVIDENCE / 'motion-readonly-inputs.json').read_text())['files']
    readonly_pins = {entry['path']: entry for entry in readonly}
    for path, entry in readonly_pins.items():
        assert hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == entry['sha256']
        assert hashlib.sha256(sp.check_output(['git', 'show', entry['commit'] + ':' + path], cwd=ROOT)).hexdigest() == entry['sha256']
    dirty = HELPER['git']('status', '--porcelain', '--untracked-files=all').splitlines()
    assert all(row[3:] in readonly_pins or row[3:].startswith('docs/mcr/evidence/fe17-original-completion/') for row in dirty)
    assert hashlib.sha256((ROOT / 'frontend/pnpm-lock.yaml').read_bytes()).hexdigest() == HELPER['LOCK']
    link, borrowed = ROOT / 'frontend/node_modules', DEPS / '.vite-temp'
    assert not link.exists() and not link.is_symlink()
    assert not borrowed.exists() or not list(borrowed.iterdir())
    caps = {'whole': 4 * 1024**3, 'new': 128 * 1024**2, 'temporary': 96 * 1024**2, 'durable': 32 * 1024**2,
            'files': 65536, 'rss': int(3.5 * 1024**3), 'processes': 16, 'childSeconds': 90}
    inventory = HELPER['carry_inventory']()
    carry = max(inventory['wholeCarryBytes'], int(os.environ['MCR_FE17_WHOLE_FLOOR']))
    other = int(os.environ['MCR_FE17_OTHER_RESERVE'])
    free = shutil.disk_usage(ROOT).free
    assert carry < caps['whole'] and inventory['chargedFiles'] < caps['files']
    assert other >= 0 and free >= 12 * 1024**3 and free - other - caps['new'] >= 9 * 1024**3
    port = int(os.environ['MCR_FE17_PORT'])
    assert 1024 <= port <= 65535
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', port))
    output.mkdir()
    tmp = Path(tempfile.mkdtemp(prefix='mcr-fe17-original-browser-', dir='/private/tmp'))
    env = {**os.environ, 'PATH': str(NODE.parent) + ':' + os.environ.get('PATH', ''), 'TMPDIR': str(tmp), 'TMP': str(tmp), 'TEMP': str(tmp),
           'XDG_CACHE_HOME': str(tmp / 'cache'), 'MCR_FRONTEND_DEPENDENCIES': str(DEPS.parent), 'MCR_FE17_CACHE': str(tmp / 'vite'),
           'MCR_FE17_EVIDENCE': str(output / 'native')}
    argv = [str(NODE), str(ROOT / 'frontend/src/world/network/__tests__/replication-browser.mjs')]
    report = {'slot': slot, 'pin': pin, 'deadlineUTC': deadline_text, 'startedUTC': dt.datetime.now(dt.timezone.utc).isoformat(),
              'caps': caps, 'inventory': inventory, 'chargedCarryBytes': carry, 'declaredPort': port, 'portPrebind': 'PASS',
              'otherReserveBytes': other, 'readonlyInputs': readonly, 'argv': argv, 'externalSingletonTargets': [],
              'wholePeak': carry, 'temporaryPeak': 0, 'durablePeak': 0, 'rssPeak': 0, 'processPeak': 0, 'newPeak': 0}
    child, owned, status, external = None, {}, 'FAIL', set()
    try:
        with (output / 'native.log').open('wb') as log:
            child = sp.Popen(argv, cwd=ROOT, env=env, stdout=log, stderr=sp.STDOUT, start_new_session=True)
            report['pid'] = child.pid
            started = time.time()
            while child.poll() is None:
                sweep_started = time.monotonic()
                rows = HELPER['processes']()
                ids = HELPER['descendants'](os.getpid(), rows)
                for pid in ids - {os.getpid()}:
                    if pid not in owned:
                        identity = fingerprint(pid)
                        if identity:
                            owned[pid] = identity
                # Chrome may create an external SingletonSocket target. Only a
                # link inside this round's private profile can declare custody;
                # never enumerate or remove an unrelated peer directory.
                for directory, dirs, names in os.walk(tmp, followlinks=False):
                    for name in dirs + names:
                        candidate = Path(directory) / name
                        if name != 'SingletonSocket' or not candidate.is_symlink():
                            continue
                        target = (candidate.parent / os.readlink(candidate)).resolve()
                        if not target.is_relative_to(tmp):
                            external.add(target.parent)
                            report['externalSingletonTargets'] = sorted(str(path) for path in external)
                            if target.name != 'SingletonSocket' or not ('scoped_dir' in target.parent.name or target.parent.name.startswith('com.google.Chrome.')):
                                raise RuntimeError('Unconfirmed external Chrome singleton target; preserve carry, no peer deletion')
                temporary = HELPER['bytes_in'](tmp) + sum(HELPER['bytes_in'](path) for path in external)
                durable = HELPER['bytes_in'](output)
                charged_roots = [HELPER['root_inventory'](path) for path in [tmp, output, *external]]
                entries = inventory['chargedFiles'] + sum(row['files'] + row['symlinks'] for row in charged_roots)
                report['entryPeak'] = max(report.get('entryPeak', inventory['chargedFiles']), entries)
                rss = sum(r * 1024 for p, _, _, r in rows if p in ids)
                free = shutil.disk_usage(ROOT).free
                for key, value in [('wholePeak', carry + temporary + durable), ('temporaryPeak', temporary), ('durablePeak', durable),
                                   ('rssPeak', rss), ('processPeak', len(ids)), ('newPeak', temporary + durable)]:
                    report[key] = max(report[key], value)
                report['minimumFree'] = min(report.get('minimumFree', free), free)
                if (time.time() >= deadline - 60 or time.time() - started >= caps['childSeconds']
                    or carry + temporary + durable > caps['whole'] or temporary + durable > caps['new']
                    or temporary > caps['temporary'] or durable > caps['durable'] or entries > caps['files'] or rss > caps['rss'] or len(ids) > caps['processes']
                    or free < 8 * 1024**3 or free - other - caps['new'] < 9 * 1024**3):
                    raise RuntimeError('Original deadline/child/whole/new/TMP/durable/RSS/process/free bound reached')
                time.sleep(max(0, .25 - (time.monotonic() - sweep_started)))
            report['exit'] = child.returncode
            assert child.returncode == 0
        native = json.loads((output / 'native/report.json').read_text())
        assert native['result'] == 'PASS' and native['cleanup'] == 'PASS'
        assert len(native['checks']) == 5 and native['errors'] == []
        status = 'PASS'
    except BaseException as exc:
        report['failure'] = type(exc).__name__ + ': ' + str(exc)
    finally:
        HELPER['retire'](child)
        # Only exact recorded descendants with the same process-start identity.
        for sig in [signal.SIGTERM, signal.SIGKILL]:
            for pid, identity in owned.items():
                if fingerprint(pid) == identity:
                    try:
                        os.kill(pid, sig)
                    except ProcessLookupError:
                        pass
        report['aliveOwnedPids'] = [pid for pid, identity in owned.items() if fingerprint(pid) == identity]
        report['unconfirmedRetainedExternal'] = [str(path) for path in external if path.exists()]
        shutil.rmtree(tmp)
        report['temporaryAbsent'] = not tmp.exists()
        report['dependencyLinkAbsent'] = not link.exists() and not link.is_symlink()
        report['borrowedViteTempEmpty'] = not borrowed.exists() or not list(borrowed.iterdir())
        report['finalInventory'] = HELPER['carry_inventory']()
        report['finalFree'] = shutil.disk_usage(ROOT).free
        native_path = output / 'native/report.json'
        if native_path.exists():
            native = json.loads(native_path.read_text())
            port = native.get('port')
            if port:
                try:
                    with socket.socket() as listener:
                        listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                        listener.bind(('127.0.0.1', port))
                    report['portReuse'] = 'PASS'
                except OSError as exc:
                    report['portReuse'] = str(exc)
                    status = 'FAIL'
        if (report['aliveOwnedPids'] or report['unconfirmedRetainedExternal'] or not report['dependencyLinkAbsent'] or not report['borrowedViteTempEmpty']
            or report['finalInventory']['wholeCarryBytes'] >= caps['whole'] or report['finalInventory']['chargedFiles'] >= caps['files']):
            status = 'FAIL'
        report.update(status=status, finishedUTC=dt.datetime.now(dt.timezone.utc).isoformat())
        (output / 'supervisor.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2))
    return 0 if status == 'PASS' else 1

if __name__ == '__main__':
    raise SystemExit(main())
