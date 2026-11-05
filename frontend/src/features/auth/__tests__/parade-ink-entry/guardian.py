"""Primary PI10 guard and exact commands; allocation is external."""
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
from metrics import Meter, ROOT, FRONT, NODE, DEPS, free, GIB, write, publish_receipt

cfg = json.loads(Path(os.environ['PI10_CONFIG']).read_text())
folder = Path(cfg['control'])
publish_receipt(cfg, 'ACCEPTED', os.getpid())
for name, expected in cfg['pins'].items():
    assert hashlib.sha256(Path(__file__).with_name(name).read_bytes()).hexdigest() == expected, name
assert hashlib.sha256((ROOT / cfg['readset']).read_bytes()).hexdigest() == cfg['readset_sha']
readset = json.loads((ROOT / cfg['readset']).read_text())
for row in readset['paths']:
    assert hashlib.sha256((ROOT / row['path']).read_bytes()).hexdigest() == row['sha256'], row['path']
for name in ['package.json', 'pnpm-lock.yaml']:
    assert (FRONT / name).read_bytes() == (Path('/Users/lhs/Desktop/mcr/frontend') / name).read_bytes()
assert hashlib.sha256((FRONT / 'pnpm-lock.yaml').read_bytes()).hexdigest() == '47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6'
assert hashlib.sha256(NODE.read_bytes()).hexdigest() == cfg['node_sha']
assert free() >= cfg['start_floor'] and free() - cfg['other_new'] - cfg.get('remaining_new_reservation', cfg['new_cap']) >= 9 * GIB
assert not (FRONT / 'node_modules').exists()
meter = Meter(cfg, int(os.environ['PI10_CONTROLLER']))
first = meter.check()
assert (folder / 'outer-initial.json').exists(), 'Independent guard must sample before workload'
write(folder / 'primary-initial.json', dict(epoch=time.time(), sample=first, global_sample=meter.last_global))
out, tmp = Path(cfg['output']), Path(cfg['temporary'])
out.mkdir(parents=True); tmp.mkdir(mode=0o700)
env = dict(os.environ, PATH=str(NODE.parent) + ':' + os.environ['PATH'], TMPDIR=str(tmp) + '/',
           PI10_CACHE=str(tmp / 'cache'), PI10_GUARDED='1', PI10_OUTPUT=str(out), PI10_PORT=str(cfg.get('port', 0)),
           PI10_OVERLAY=str(tmp / 'overlay'))
prefix = 'src/features/auth/__tests__/parade-ink-entry/'
commands = [
    [str(NODE), str(DEPS / 'vitest/vitest.mjs'), 'run', '--config', prefix + 'vitest.config.mjs', '--configLoader', 'runner'],
    [str(NODE), str(DEPS / 'typescript/bin/tsc'), '-p', prefix + 'tsconfig.json', '--pretty', 'false', '--incremental', 'false'],
    [str(NODE), str(DEPS / 'oxlint/bin/oxlint'), '--deny-warnings', *cfg['lint_paths']],
    [str(NODE), '--check', prefix + 'native.mjs'],
]
if cfg.get('source_retry'):
    assert cfg['mode'] == 'source'
    commands[0] += [prefix + 'entry.test.tsx', '-t', 'preserves password recovery mode controls and focus after an uncertain request']
if cfg['mode'] == 'native':
    assert hashlib.sha256(Path('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome').read_bytes()).hexdigest() == cfg['chrome_sha']
    commit = '77cfe94af06cac6b292ad7528352e4750e929e71'
    raw = subprocess.check_output(['git', 'show', commit + ':docs/mcr/evidence/parade-ink-foundation/qualification-readset.json'], cwd=ROOT)
    qualification_sha = hashlib.sha256(raw).hexdigest()
    assert qualification_sha == 'eb80d722f513ce794fe35317a87702dd2878321a2d76f313d36265b3d593d5ef'
    shared = json.loads(raw)
    files = []
    overlay = tmp / 'overlay'; overlay.mkdir()
    for row in shared['implementation']:
        files.append(row)
        data = subprocess.check_output(['git', 'show', commit + ':' + row['path']], cwd=ROOT)
        assert hashlib.sha256(data).hexdigest() == row['sha256'], row['path']
        file = overlay / row['path']; file.parent.mkdir(parents=True, exist_ok=True); file.write_bytes(data)
    write(overlay / 'readset.json', dict(commit=commit, files=files, qualificationSHA=qualification_sha))
    commands = [
        [str(NODE), str(DEPS / 'oxlint/bin/oxlint'), '--deny-warnings', prefix + 'native.mjs', prefix + 'compare.mjs', prefix + 'runtime.mjs'],
        [str(NODE), '--check', prefix + 'native.mjs'],
        [str(NODE), '--check', prefix + 'compare.mjs'],
        [str(NODE), '--check', prefix + 'runtime.mjs'],
        [str(NODE), prefix + 'native.mjs'],
    ]
    if cfg.get('qualified_helper_proof'):
        proof = cfg['qualified_helper_proof']
        proof_path = ROOT / proof['path']
        assert hashlib.sha256(proof_path.read_bytes()).hexdigest() == proof['sha256']
        prior = json.loads(proof_path.read_text())
        assert len(prior['commands']) == 4 and all(row['exit'] == 0 for row in prior['commands'])
        for name, expected in proof['helper_pins'].items():
            assert hashlib.sha256(Path(__file__).with_name(name).read_bytes()).hexdigest() == expected
        commands = [commands[-1]]
    if cfg.get('entry_remainder'):
        assert cfg['entry_remainder'] is True
        commands = [
            [str(NODE), str(DEPS / 'oxlint/bin/oxlint'), '--deny-warnings', prefix + 'native.mjs', prefix + 'entry-remainder.mjs'],
            [str(NODE), '--check', prefix + 'native.mjs'],
            [str(NODE), '--check', prefix + 'entry-remainder.mjs'],
            [str(NODE), prefix + 'entry-remainder.mjs'],
        ]
write(folder / 'primary-accept.json', dict(lease=cfg['lease'], epoch=time.time(), free=free(), carry=meter.carry))
code, failure, child = 0, None, None
command_results = []
try:
    for index, argv in enumerate(commands):
        meter.check()
        with (out / f'command-{index:02}.log').open('w') as log:
            child = subprocess.Popen(argv, cwd=FRONT, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            while child.poll() is None:
                meter.check(); time.sleep(.2)
            command_results.append(dict(index=index, argv=argv, exit=child.returncode))
            if child.returncode and not code:
                code = child.returncode
            if child.returncode and cfg['mode'] == 'native':
                break
except BaseException as error:
    failure, code = repr(error), 1
finally:
    if child and child.poll() is None:
        try:
            os.killpg(child.pid, signal.SIGTERM); time.sleep(.3); os.killpg(child.pid, signal.SIGKILL)
        except OSError:
            pass
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            failure = (failure or '') + '; owned child did not exit within cleanup wait'
            code = 1
    try:
        assert hashlib.sha256((ROOT / cfg['readset']).read_bytes()).hexdigest() == cfg['readset_sha']
        for row in readset['paths']:
            assert hashlib.sha256((ROOT / row['path']).read_bytes()).hexdigest() == row['sha256'], row['path']
    except BaseException as error:
        failure, code = 'post-run pin drift: ' + repr(error), 1
    try:
        meter.check(final=True)
    except BaseException as error:
        failure, code = (failure or '') + '; final resource census: ' + repr(error), 1
    write(folder / 'primary-full.json', dict(lease=cfg['lease'], result='PASS' if code == 0 and failure is None else 'FAIL', exit=code,
          failure=failure, commands=command_results, full_epoch=time.time(), before_hard=time.time() < cfg['full_deadline'], peak=meter.peak, minimum_free=meter.minimum_free))
sys.exit(code)
