"""Independent outer SOURCE guard; includes itself, supervisor and all children.

No child starts before current offer/carry/free-space admission. No offer is
created or inferred here. The inner supervisor separately enforces its limits.
"""
import datetime as dt
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess as sp
import sys
import time

SOURCE = Path(__file__).with_name('qualify-source.py')
spec = importlib.util.spec_from_file_location('qa05_source_guard', SOURCE)
source = importlib.util.module_from_spec(spec)
spec.loader.exec_module(source)


def main():
    offer_path, attempt = Path(sys.argv[1]), sys.argv[2]
    assert offer_path.stat().st_mode & 0o077 == 0
    offer = json.loads(offer_path.read_text())
    assert offer['issuer'] == '01a1a21d-39ec-7912-84cb-910058e7a110' and offer['acceptance'] == 'ACCEPT'
    assert offer['mode'] == 'SOURCE' and offer['sourceCommit'] == source.git('rev-parse', 'HEAD')
    assert attempt.startswith('source-') and '/' not in attempt
    caps = offer['caps']
    carried = source.census()
    free = shutil.disk_usage(source.ROOT).free
    assert free >= offer['startFloorBytes'] and free - offer['otherReservedBytes'] - caps['newBytes'] >= 9 * 1024**3
    assert carried['bytes'] <= caps['wholeBytes'] and carried['entries'] <= caps['entries']
    work = dt.datetime.fromisoformat(offer['workByUTC']).timestamp()
    cleanup = dt.datetime.fromisoformat(offer['cleanupByUTC']).timestamp()
    assert 15 < work - time.time() <= 120 and 0 < cleanup - work <= 60
    child = None
    result = {'slot': offer['slot'], 'guardianPid': os.getpid(), 'status': 'FAIL', 'inventory': carried,
              'rssPeak': 0, 'processPeak': 0, 'newPeak': 0, 'minimumFree': free}
    baseline = source.inventory(source.ROOT)['bytes']
    try:
        child = sp.Popen([sys.executable, '-B', str(SOURCE), str(offer_path), attempt], cwd=source.ROOT, start_new_session=True)
        while True:
            rows = source.processes()
            owned = {os.getpid()}
            while True:
                expanded = owned | {pid for pid, parent, _, _ in rows if parent in owned}
                if expanded == owned:
                    break
                owned = expanded
            rss = sum(value * 1024 for pid, _, _, value in rows if pid in owned)
            # Count the entire checkout delta, plus all own external temporary
            # source roots, independently of the supervisor's output labels.
            growth = max(0, source.inventory(source.ROOT)['bytes'] - baseline)
            temp_roots = list(Path('/private/tmp').glob('mcr-qa05-source-*'))
            temporary = sum(source.inventory(path)['bytes'] for path in temp_roots)
            growth += temporary
            free = shutil.disk_usage(source.ROOT).free
            result['rssPeak'] = max(result['rssPeak'], rss)
            result['processPeak'] = max(result['processPeak'], len(owned))
            result['newPeak'] = max(result['newPeak'], growth)
            result['minimumFree'] = min(result['minimumFree'], free)
            assert time.time() < work and rss <= caps['rssBytes'] and len(owned) <= caps['processes']
            assert temporary <= caps['temporaryBytes'] and growth <= caps['newBytes'] and carried['bytes'] + growth <= caps['wholeBytes']
            assert free >= 8 * 1024**3 and free - offer['otherReservedBytes'] - caps['newBytes'] >= 9 * 1024**3
            if child.poll() is not None:
                break
            time.sleep(.25)
        assert child.returncode == 0
        result['status'] = 'PASS'
    except BaseException as error:
        result['failure'] = type(error).__name__ + ': ' + str(error)
    finally:
        source.stop(child)
        result['ownedProcessGroupsAbsent'] = child is None or not any(group == child.pid for _, _, group, _ in source.processes())
        result['finishedUTC'] = dt.datetime.now(dt.timezone.utc).isoformat()
        result['withinCleanupDeadline'] = time.time() < cleanup
        if not result['ownedProcessGroupsAbsent'] or not result['withinCleanupDeadline']:
            result['status'] = 'FAIL'
        output = source.EVIDENCE / attempt
        output.mkdir(exist_ok=True)
        (output / 'guardian.json').write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(result))
    return 0 if result['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
