"""Finite adapter source qualification. Requires allocator's exact external offer."""
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[5]
HERE = Path(__file__).resolve().parent

def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')

def identity(pid):
    return dict(pid=pid, birth=' '.join(subprocess.check_output(['/bin/ps', '-p', str(pid), '-o', 'lstart='], text=True).split()),
                executable=subprocess.check_output(['/bin/ps', '-p', str(pid), '-o', 'comm='], text=True).strip(),
                script=str(Path(__file__).resolve()), cwd=str(Path.cwd()), outputRoot=cfg['output'])

def publish(state, guard=None):
    path = Path(cfg['owner_receipt'])
    previous = json.loads(path.read_text())
    previous_full = previous.get('previousLiteralFull') if previous.get('offerId') == cfg['lease'] else previous
    result = dict(ownerThreadId=cfg['owner_thread'], offerId=cfg['lease'], kind='source', state=state,
                  acceptedAtEpoch=cfg['accept_epoch'], workDeadlineEpoch=cfg['work_deadline'], fullDeadlineEpoch=cfg['full_deadline'],
                  processCap=cfg['process_cap'], rssCapBytes=cfg['rss_cap'],
                  remainingNewUpperBytes=cfg['remaining_new_reservation'] if state == 'ACCEPTED' else 0,
                  controller=identity(cfg['controller_pid']), guardControllers=[identity(guard)] if guard else previous.get('guardControllers', []),
                  previousLiteralFull=previous_full, receiptPaths=dict(accept=str(folder / 'offer.json'), full=str(folder / 'outer-full.json')),
                  observedAtEpoch=time.time())
    temporary = path.with_suffix('.tmp'); write(temporary, result); os.replace(temporary, path)

is_guard = sys.argv[1] == '--guard'
proposal = Path(sys.argv[2] if is_guard else sys.argv[1]).resolve()
cfg = json.loads(proposal.read_text()); folder = Path(cfg['control'])
if not is_guard:
    assert Path.cwd() == ROOT and not folder.exists(), 'Closed finite rounds cannot restart'
    assert time.time() < cfg['accept_by_epoch'] and cfg['allocator_offer']
    stat = os.statvfs(ROOT); available = stat.f_bavail * stat.f_frsize
    assert available >= cfg['start_floor'] >= 11 * 1024**3
    assert available - cfg['other_new'] - cfg['remaining_new_reservation'] >= 9 * 1024**3
    accepted = time.time()
    cfg.update(accept_epoch=accepted, work_deadline=accepted + 300, full_deadline=accepted + 360,
               controller_pid=os.getpid(), fresh_free=available)
    folder.mkdir(mode=0o700); write(folder / 'offer.json', cfg)
    publish('ACCEPTED')
    proposal = folder / 'offer.json'
else:
    publish('ACCEPTED', os.getpid())
# Actual identities precede imports, expensive inventories, and any Node child.
sys.path.insert(0, str(ROOT / 'frontend/src/features/auth/__tests__/parade-ink-entry'))
from metrics import Meter, NODE, DEPS, tree

if is_guard:
    meter = Meter(cfg, cfg['controller_pid'])
    try:
        first = meter.check()
        write(folder / 'guard-initial.json', dict(epoch=time.time(), sample=first, global_sample=meter.last_global))
        while not (folder / 'stop.json').exists():
            meter.check(); time.sleep(.2)
        meter.check(final=True)
        write(folder / 'guard-full.json', dict(result='PASS', epoch=time.time(), peak=meter.peak, minimum_free=meter.minimum_free))
    except BaseException as error:
        write(folder / 'guard-full.json', dict(result='FAIL', epoch=time.time(), failure=repr(error), peak=meter.peak))
        sys.exit(1)
    sys.exit(0)

child = None; guard = None; meter = None; code = 1; failure = None; commands = []
try:
    assert hashlib.sha256(Path(__file__).read_bytes()).hexdigest() == cfg['pins']['round.py']
    for name, sha in cfg['pins'].items():
        assert hashlib.sha256((HERE / name).read_bytes()).hexdigest() == sha, name
    assert hashlib.sha256(NODE.read_bytes()).hexdigest() == cfg['node_sha']
    assert not (ROOT / 'frontend/node_modules').exists()
    assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == cfg['source']
    log = (folder / 'guard.log').open('w')
    guard = subprocess.Popen([sys.executable, '-B', str(Path(__file__).resolve()), '--guard', str(proposal)],
                             cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
    meter = Meter(cfg, os.getpid())
    first = meter.check()
    write(folder / 'outer-initial.json', dict(epoch=time.time(), sample=first, global_sample=meter.last_global))
    while not (folder / 'guard-initial.json').exists():
        assert guard.poll() is None, 'Guard failed before complete live initial sample'
        meter.check(); time.sleep(.2)
    assert guard.poll() is None
    out, temporary = Path(cfg['output']), Path(cfg['temporary'])
    out.mkdir(parents=True); temporary.mkdir(mode=0o700)
    overlay = temporary / 'overlay'; overlay.mkdir()
    raw = (ROOT / cfg['readset']).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == cfg['readset_sha']
    readset = json.loads(raw)
    if not cfg.get('lint_only'):
        for row_index, row in enumerate(readset['paths']):
            if row_index % 32 == 0: meter.check()
            data = subprocess.check_output(['git', 'show', row['revision'] + ':' + row['path']], cwd=ROOT) if row['revision'] else (ROOT / row['path']).read_bytes()
            assert hashlib.sha256(data).hexdigest() == row['sha256'], row['path']
            target = overlay / row['path']; target.parent.mkdir(parents=True, exist_ok=True); target.write_bytes(data)
        tsconfig = json.loads((ROOT / 'frontend/tsconfig.json').read_text())
        tsconfig['include'] = ['src/app/venue-notes-adapter/index.ts', 'src/app/venue-notes-adapter/__tests__/adapter.test.ts']
        tsconfig['compilerOptions'].update(incremental=False, baseUrl='.', ignoreDeprecations='6.0',
            typeRoots=[str(DEPS / '@types')], types=[str(DEPS / 'vite/client')], paths={
                '*': [str(DEPS / '*')], 'react': [str(DEPS / '@types/react/index.d.ts')],
                'react/*': [str(DEPS / '@types/react/*')], 'react-dom': [str(DEPS / '@types/react-dom/index.d.ts')],
                'react-dom/*': [str(DEPS / '@types/react-dom/*')]})
        write(overlay / 'frontend/tsconfig.json', tsconfig)
    env = dict(os.environ, PATH=str(NODE.parent) + ':' + os.environ['PATH'], TMPDIR=str(temporary) + '/',
               NOTES_OVERLAY=str(overlay), NOTES_TMP=str(temporary))
    argv_list = [
      [str(NODE), str(DEPS / 'vitest/vitest.mjs'), 'run', '--config', str(HERE / 'vitest.config.mjs'), '--configLoader', 'runner'],
      [str(NODE), str(DEPS / 'typescript/bin/tsc'), '-p', str(overlay / 'frontend/tsconfig.json'), '--pretty', 'false', '--incremental', 'false'],
      [str(NODE), str(DEPS / 'oxlint/bin/oxlint'), '--deny-warnings', str(HERE.parent)],
    ]
    if cfg.get('lint_only'):
        argv_list = [argv_list[-1]]
    if cfg.get('repair_filter'):
        argv_list[0] += ['-t', cfg['repair_filter']]
    code = 0
    for index, argv in enumerate(argv_list):
        meter.check(); assert guard.poll() is None
        with (out / f'command-{index:02}.log').open('w') as command_log:
            child = subprocess.Popen(argv, cwd=ROOT / 'frontend', env=env, start_new_session=True, stdout=command_log, stderr=subprocess.STDOUT)
            while child.poll() is None:
                meter.check(); assert guard.poll() is None; time.sleep(.2)
            commands.append(dict(argv=argv, exit=child.returncode))
            code = code or child.returncode
    assert hashlib.sha256((ROOT / cfg['readset']).read_bytes()).hexdigest() == cfg['readset_sha']
    for row in readset['paths']:
        if row['revision'] is None:
            assert hashlib.sha256((ROOT / row['path']).read_bytes()).hexdigest() == row['sha256']
except BaseException as error:
    failure = repr(error); code = 1
finally:
    if child and child.poll() is None:
        try: os.killpg(child.pid, signal.SIGTERM); time.sleep(.3); os.killpg(child.pid, signal.SIGKILL)
        except OSError: pass
        child.wait(timeout=5)
    write(folder / 'stop.json', dict(epoch=time.time()))
    if guard:
        try: guard.wait(timeout=8)
        except subprocess.TimeoutExpired: guard.kill(); guard.wait(); failure = (failure or '') + '; guard did not release'
        log.close()
        if guard.returncode: failure = (failure or '') + '; guard failed'; code = 1
    if meter:
        try: meter.check(final=True)
        except BaseException as error: failure = (failure or '') + '; final census ' + repr(error); code = 1
    remaining = [pid for pid, row in tree(os.getpid(), meter.seen if meter else {}).items() if pid != os.getpid() and Path(row[3]).name != 'ps']
    result = dict(lease=cfg['lease'], result='PASS' if not code and not failure and not remaining else 'FAIL',
                  exit=code, failure=failure, commands=commands, full_epoch=time.time(),
                  peak=meter.peak if meter else {}, minimum_free=meter.minimum_free if meter else None,
                  remaining=remaining, execution_resources=len(remaining), before_hard=time.time() < cfg['full_deadline'])
    write(folder / 'outer-full.json', result)
    if not remaining: publish('FULL')
    print(json.dumps(result), flush=True)
sys.exit(0 if result['result'] == 'PASS' else 1)
