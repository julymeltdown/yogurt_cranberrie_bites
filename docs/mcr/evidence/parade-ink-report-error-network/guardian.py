"""Independent PI12 outer guard: factual bootstrap receipt before census."""
import datetime as dt
import importlib.util
import json
import os
from pathlib import Path
import subprocess as sp
import sys
import time

spec = importlib.util.spec_from_file_location('pi12_source_controls', Path(__file__).with_name('controller.py'))
control = importlib.util.module_from_spec(spec)
spec.loader.exec_module(control)


def main():
    offer_path = Path(sys.argv[1]); assert offer_path.stat().st_mode & 0o077 == 0
    offer = json.loads(offer_path.read_text()); output = Path(offer['output'])
    assert offer['acceptance'] == 'ACCEPT' and offer['issuer'] == '01a1a21d-39ec-7912-84cb-910058e7a110'
    assert output.parent == control.EVIDENCE and not output.exists()
    current = control.identity(os.getpid(), __file__, output)
    control.publish_receipt(offer, 'ACCEPTED_BOOTSTRAP', current, [])
    report = {'status': 'FAIL', 'samples': [], 'phase': 'bootstrap'}; child = None
    try:
        output.mkdir()
        assert offer['sourceCommit'] == control.git('rev-parse', 'HEAD')
        control.verify_tools(offer)
        assert 30 < offer['workDeadline'] - time.time() <= (480 if offer['mode'] == 'BROWSER' else 240)
        assert offer['workDeadline'] < offer['fullDeadline'] <= offer['workDeadline'] + 60
        child = sp.Popen([str(Path(sys.executable).resolve()), '-B', str(control.EVIDENCE / 'controller.py'), str(offer_path)], cwd=control.ROOT, start_new_session=True)
        while not (output / 'inner-bootstrap.json').exists():
            assert child.poll() is None and time.time() < offer['workDeadline'] - 20, 'Inner bootstrap deadline'
            time.sleep(.05)
        bootstrap = json.loads((output / 'inner-bootstrap.json').read_text())
        primary = bootstrap['controller']
        assert primary['pid'] == child.pid and control.identity(child.pid)['birth'].split() == primary['birth'].split()
        control.publish_receipt(offer, 'ACCEPTED_BOTH_BOOTSTRAPPED', primary, [current])
        whole = control.census(offer['mode'] == 'BROWSER')
        assert whole['wholeBytes'] <= offer['baselineWholeBytes'] <= offer['wholeBytes'] and whole['entries'] <= offer['fileCap']
        census_bytes = json.dumps(whole,separators=(',',':')).encode()
        (output / ('outer-census.json.gz' if offer.get('compressedCensus') else 'outer-census.json')).write_bytes(control.gzip.compress(census_bytes) if offer.get('compressedCensus') else census_bytes)
        first = control.sample(offer, os.getpid(), whole['rows']); assert first['freeBytes'] >= offer['startFloorBytes']
        initial_temp = output / 'outer-initial.tmp'
        initial_temp.write_text(json.dumps(first))
        os.replace(initial_temp, output / 'outer-initial.json')
        report.update({'samples': [first], 'phase': 'guards', 'wholeBefore': {k: v for k, v in whole.items() if k != 'rows'}})
        control.publish_receipt(offer, 'ACCEPTED_GUARDS_STARTING', primary, [current])
        while child.poll() is None:
            report['samples'].append(control.sample(offer, os.getpid(), whole['rows'])); time.sleep(.5)
        assert child.returncode == 0
        report['status'] = 'PASS'
    except BaseException as error:
        report['failure'] = str(error)
    finally:
        control.stop(child)
        link = control.ROOT / 'frontend/node_modules'
        if link.is_symlink() and link.resolve() == control.DEPS.resolve():
            link.unlink()
        temporary = Path(offer['temporary']); assert str(temporary).startswith('/private/tmp/mcr-pi12-')
        if temporary.exists():
            control.shutil.rmtree(temporary)
        report['finishedUTC'] = dt.datetime.now(dt.timezone.utc).isoformat()
        report['withinFULLDeadline'] = time.time() < offer['fullDeadline']
        report['resourcesReleased'] = not any(int(v[2]) == (child.pid if child else -1) for v in control.processes())
        if not report['withinFULLDeadline'] or not report['resourcesReleased']:
            report['status'] = 'FAIL'
        output.mkdir(exist_ok=True)
        (output / 'guardian.json').write_text(json.dumps(report, indent=2) + '\n')
        control.publish_receipt(offer, 'FULL', current, [])
    print(json.dumps({k: v for k, v in report.items() if k not in ['samples', 'wholeBefore']}))
    return 0 if report['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
