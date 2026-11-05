"""Finite PI12 pre-World presentation source/browser round. Requires a fresh named allocator offer.

Adapts the process-group cleanup and disk/RSS guards from the retained
fe41-default-entry supervisor. No install, build or service jobs. Browser execution requires a fresh browser allocation.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import runpy
import sys
import subprocess as sp
import time

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
LINK = ROOT / 'frontend/node_modules'
parser = argparse.ArgumentParser()
for field in ['offer', 'head', 'job', 'attempt', 'acceptance']:
    parser.add_argument('--' + field, required=True)
for field in ['cutoff', 'release', 'whole', 'growth', 'temporary', 'durable', 'files', 'processes', 'rss']:
    parser.add_argument('--' + field, type=int, required=True)
parser.add_argument('--start-floor', type=int, choices=[11, 12], default=12)
parser.add_argument('--freeze', default='source-freeze.json')
parser.add_argument('--bootstrap-work', type=int)
parser.add_argument('--bootstrap-full', type=int)
parser.add_argument('--minimum-carry-bytes', type=int, default=16975478482)
parser.add_argument('--retained-growth-bytes', type=int, default=0)
parser.add_argument('--retain-decoder-pass', action='store_true')
parser.add_argument('--check-completed-finisher', action='store_true')
parser.add_argument('--retain-native09-controls', action='store_true')
parser.add_argument('--family-png-cap', type=int, choices=[40, 48], default=40)
parser.add_argument('--retained-png-count', type=int, default=0)
parser.add_argument('--retained-png-bytes', type=int, default=0)
args = parser.parse_args()
assert args.attempt.isalnum() and args.cutoff < args.release
assert Path(sys.argv[0]).is_absolute() and Path.cwd() == ROOT
assert Path(sys.executable).is_absolute()
scratch = Path('/private/tmp') / ('mcr-pi12-preworld-' + args.attempt)
type_config = ROOT / 'frontend/src/app/pre-world-policy/__tests__/parade-ink/.finite-tsconfig.json'
assert not type_config.exists() and not type_config.is_symlink()
type_config_created = False
features = ['app/pre-world-policy/parade-ink', 'app/pre-world-policy/__tests__/parade-ink']
tests = [str(NODE), str(LINK / 'vitest/vitest.mjs'), 'run', '--root', str(ROOT / 'frontend'), '--configLoader', 'runner', '--no-cache', '--maxWorkers', '1']
owned = ['frontend/src/app/pre-world-policy/parade-ink', 'frontend/src/app/pre-world-policy/__tests__/parade-ink', 'frontend/e2e/visual/parade-ink-pre-world-policy', 'frontend/src/app/pre-world-policy/PolicyNotice.tsx', 'frontend/src/app/WorldShell.tsx']
jobs = {
    'focus': tests + ['--config', 'src/app/pre-world-policy/__tests__/parade-ink/vitest.config.ts'],
    'types': [str(NODE), str(LINK / 'typescript/bin/tsc'), '--noEmit', '--incremental', 'false', '--project', str(type_config)],
    'lint': [str(NODE), str(LINK / 'oxlint/bin/oxlint'), '--deny-warnings'] + owned,
    'browser': [str(NODE), 'frontend/e2e/visual/parade-ink-pre-world-policy/native.mjs'],
    'decoder': [sys.executable, '-B', str(OUT / 'receipt-path-regression.py')],
    'driverLint': [str(NODE), str(LINK / 'oxlint/bin/oxlint'), '--deny-warnings', 'frontend/e2e/visual/parade-ink-pre-world-policy/native.mjs'],
    'finisher': [sys.executable, '-B', str(OUT / 'completed-finisher-regression.py')],
    'guard': ['python3', str(OUT / 'scope-guard.py')],
    'syntax': [str(NODE), '--check', 'frontend/e2e/visual/parade-ink-pre-world-policy/native.mjs'],
}
if args.job == 'source03':
    jobs['source03'] = []
assert args.job in jobs
# Publish the actual current absolute controller BEFORE Git/source/runtime census.
sys.dont_write_bytecode = True
frozen = json.loads((OUT / args.freeze).read_text())
assert hashlib.sha256((OUT / 'guardian.py').read_bytes()).hexdigest() == frozen['controls']['guardian.py']
if args.job == 'browser':
    assert (args.family_png_cap, args.retained_png_count, args.retained_png_bytes) == (frozen['maxPNGs'], frozen['retainedPNGCount'], frozen['retainedPNGBytes'])
shared = runpy.run_path(str(OUT / 'guardian.py'))
accept_path = OUT / args.acceptance
if args.bootstrap_work is not None:
    assert args.job in ('source03', 'types', 'browser')
    assert (args.bootstrap_work, args.bootstrap_full) == ((300, 360) if args.job == 'browser' else (120, 180))
    assert not accept_path.exists()
    accepted = time.time()
    args.cutoff = accepted + args.bootstrap_work
    args.release = accepted + args.bootstrap_full
    bootstrap_path = OUT / (args.attempt + '-bootstrap-acceptance.json')
    assert not bootstrap_path.exists()
    bootstrap_path.write_text(json.dumps(dict(offer=args.offer, accepted=accepted, cutoff=args.cutoff, release=args.release,
        head=args.head, whole=args.whole, new=args.growth, files=args.files, processes=args.processes, rss=args.rss,
        minimumCarryBytes=args.minimum_carry_bytes, fullDOC16UpperBytes=16777216, retainedGrowthBytes=args.retained_growth_bytes,
        pngFamily=dict(cap=args.family_png_cap, retainedCount=args.retained_png_count, retainedBytes=args.retained_png_bytes, byteCap=16777216),
        cwd=str(ROOT), controller=str(Path(__file__))), indent=2) + '\n')
else:
    accepted = json.loads(accept_path.read_text())['accepted']
    bootstrap_path = accept_path
early_config = dict(mode='browser' if args.job == 'browser' else 'source', offerId=args.offer, acceptedAtEpoch=accepted,
    workDeadlineEpoch=args.cutoff, fullDeadlineEpoch=args.release, offer=dict(processes=args.processes, rssBytes=args.rss, newBytes=args.growth, otherUpperBytes=3*1024**3))
shared['publish_accepted'](early_config, os.getpid(), shared['process_rows'](), OUT, bootstrap_path, scratch)
if args.bootstrap_work is not None:
    try:
        raw = sp.check_output([sys.executable, '-B', str(OUT / 'admission.py'), '--browser'], cwd=ROOT, text=True)
        measured = json.loads(raw)
        measured.update(offer=args.offer, accepted=accepted, cutoff=args.cutoff, release=args.release, otherWorstGrowth=3*1024**3, startFloorGiB=args.start_floor,
            minimumCarryBytes=args.minimum_carry_bytes, retainedGrowthBytes=args.retained_growth_bytes, minimumCarryFiles=62398, fullDOC16UpperBytes=16777216, bootstrapReceipt=str(bootstrap_path),
            ownerReceiptBytes=(shared['RECEIPTS'] / (shared['OWN_ID'] + '.json')).stat().st_size)
        accept_path.write_text(json.dumps(measured, indent=2) + '\n')
    except BaseException as error:
        full_path = OUT / (args.attempt + '-FULL.json')
        full_path.write_text(json.dumps(dict(state='FULL', admissionFailure=str(error), finished=time.time(), children=0, link=0, temporary=0, reservations=0)) + '\n')
        owner_path = shared['RECEIPTS'] / (shared['OWN_ID'] + '.json')
        record = json.loads(owner_path.read_text());record.update(state='FULL', actualFullEpoch=time.time(), remainingNewUpperBytes=0, resources=dict(children=0, link=0, temporary=0, reservations=0));record['receiptPaths']['full']=str(full_path)
        staging=owner_path.with_name(owner_path.name + '.' + str(os.getpid()) + '.tmp');staging.write_text(json.dumps(record, indent=2)+'\n');staging.replace(owner_path)
        raise
assert sp.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == args.head
assert not LINK.exists() and not LINK.is_symlink()
scratch = Path('/private/tmp') / ('mcr-pi12-preworld-' + args.attempt)
assert not scratch.exists()
log = OUT / (args.attempt + '.log')
receipt = OUT / (args.attempt + '.json')
assert not log.exists() and not receipt.exists()

def inventory(root, exclude=()):
    assert Path(root).is_dir(), ('required custody root missing', str(root))
    count = total = 0
    for directory, dirs, files in os.walk(root, followlinks=False):
        dirs[:] = [d for d in dirs if d not in exclude and not (Path(directory) / d).is_symlink()]
        for name in files:
            p = Path(directory) / name
            try:
                if not p.is_symlink():
                    count += 1; total += p.stat().st_size
            except FileNotFoundError:
                pass
    return count, total

# Conservative carry: full checkout (including old evidence/backend), all shared
# Git objects read-only plus this worktree's metadata, not only the patch delta.
source_files, source_bytes = inventory(ROOT, ('.git', 'node_modules'))
gitdir = Path(sp.check_output(['git', 'rev-parse', '--git-common-dir'], cwd=ROOT, text=True).strip())
git_files, git_bytes = inventory(gitdir)
metadata_files, metadata_bytes = inventory(Path(sp.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=ROOT, text=True).strip()))
dependency_files, dependency_bytes = inventory(DEPS.resolve())
admission = json.loads((OUT / args.acceptance).read_text())
assert admission['offer'] == args.offer and admission['cutoff'] == args.cutoff and admission['release'] == args.release
# All phases use the one acceptance baseline and original deadline. Logs/Git
# from red/fix/green continue consuming the same total positive-growth caps.
source_files, source_bytes = admission['sourceFiles'], admission['sourceBytes']
git_files, git_bytes = admission['gitFiles'], admission['gitBytes']
metadata_files, metadata_bytes = admission['metadataFiles'], admission['metadataBytes']
assert dependency_files == admission['dependencyFiles'] and dependency_bytes == admission['dependencyBytes']
carry_files = source_files + git_files + metadata_files + dependency_files
carry_bytes = source_bytes + git_bytes + metadata_bytes + dependency_bytes
for runtime, path in [('python', Path('/Library/Frameworks/Python.framework')), ('nodeRuntime', NODE.parent.parent)]:
    runtime_files, runtime_bytes = inventory(path)
    assert runtime_files == admission[runtime + 'Files'] and runtime_bytes == admission[runtime + 'Bytes']
    carry_files += runtime_files
    carry_bytes += runtime_bytes
if admission.get('browserFiles'):
    browser_files, browser_bytes = inventory(Path('/Applications/Google Chrome.app'))
    assert browser_files == admission['browserFiles'] and browser_bytes == admission['browserBytes']
    carry_files += browser_files
    carry_bytes += browser_bytes
carry_bytes = max(carry_bytes, admission['minimumCarryBytes'])
carry_files = max(carry_files, admission['minimumCarryFiles'])
if args.job == 'browser':
    assert admission.get('browserFiles'), 'listed Chrome must be charged completely'
    assert NODE.stat().st_size == admission['nodeBytes']
initial_out_files, initial_out_bytes = admission['outFiles'], admission['outBytes']
info = dict(offer=args.offer, head=args.head, job=args.job, argv=jobs[args.job], started=time.time(), cutoff=args.cutoff, release=args.release,
            carryFiles=carry_files, carryBytes=carry_bytes, dependencyFiles=dependency_files, dependencyBytes=dependency_bytes, peaks={}, minFree=2**63-1, exitCode=None,
            sourcePins={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for name in features for p in (ROOT / 'frontend/src' / name).rglob('*') if p.is_file()},
            locks={p:hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in ['frontend/pnpm-lock.yaml', 'frontend/package.json']})
child = None
guardian = None
guardian_output = OUT / (args.attempt + '-guardian')
known = {os.getpid()}
births = {}
shared_config = dict(mode='browser' if args.job == 'browser' else 'source', offer=dict(processes=args.processes, rssBytes=args.rss, newBytes=args.growth, otherUpperBytes=admission['otherWorstGrowth']))
shared_config.update(censusPath=str(OUT / (args.attempt + '-primary-census.json')), offerId=args.offer, acceptedAtEpoch=admission['accepted'], workDeadlineEpoch=args.cutoff, fullDeadlineEpoch=args.release)

def process_rows():
    probe = sp.Popen(['ps', '-axo', 'pid=,ppid=,pgid=,rss=,lstart=,command='], stdout=sp.PIPE, text=True)
    output, _ = probe.communicate()
    rows = []
    for line in output.splitlines():
        parts = line.split(None, 9)
        if len(parts) == 10:
            rows.append((*map(int, parts[:4]), ' '.join(parts[4:9]), parts[9]))
    rows = [r for r in rows if r[0] != probe.pid]
    known.discard(probe.pid)
    # Do not retain a dead descendant PID and later kill an unrelated process
    # that reused it. Preserve detached Chrome helpers by birth and task TMP.
    known.intersection_update({pid for pid, _, _, _, birth, _ in rows if pid in known and (pid not in births or births[pid] == birth)})
    while True:
        new = known | {pid for pid, parent, group, _, _, command in rows if parent in known or child and group == child.pid or str(scratch) in command}
        if new == known:
            break
        known.update(new)
    births.update({pid: birth for pid, _, _, _, birth, _ in rows if pid in known})
    return [(pid, rss * 1024) for pid, _, _, rss, _, _ in rows if pid in known]

def measure():
    if not scratch.is_dir():
        assert not LINK.is_symlink(), 'owned TMP disappeared while allocation active'
    tf, tb = inventory(scratch) if scratch.is_dir() else (0, 0)
    df, db = inventory(OUT)
    if type_config_created:
        tf += 1; tb += type_config.stat().st_size
    sf, sb = inventory(ROOT, ('.git', 'node_modules'))
    gf, gb = inventory(gitdir)
    mf, mb = inventory(Path(sp.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=ROOT, text=True).strip()))
    # Borrowed dependencies are read-only. Source and Git growth are measured
    # continuously; positive growth never subtracts deleted historical output.
    positive_bytes = max(0, sb-source_bytes) + max(0, gb-git_bytes) + max(0, mb-metadata_bytes)
    owner_record = Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts/01a1a21e-ab80-7012-b9fa-cdb69eed097b.json')
    positive_bytes += max(0, owner_record.stat().st_size - admission.get('ownerReceiptBytes', 0))
    positive_files = max(0, sf-source_files) + max(0, gf-git_files) + max(0, mf-metadata_files)
    processes = process_rows()
    values = dict(whole=carry_bytes + tb + positive_bytes, growth=admission.get('retainedGrowthBytes', 0)+tb+positive_bytes, temporary=tb, durable=max(0, db-initial_out_bytes),
                  files=carry_files + tf + positive_files, processes=len(processes), rss=sum(r for _, r in processes))
    for key, value in values.items():
        info['peaks'][key] = max(info['peaks'].get(key, 0), value)
        assert value <= getattr(args, key), (key, value)
    global_state = shared['global_sample'](shared_config, shared['process_rows'](), {pid for pid, _ in processes})
    free = shutil.disk_usage(ROOT).free
    info['lastGlobal'] = global_state
    shared['global_check'](shared_config, global_state, free)
    info['minFree'] = min(info['minFree'], free)
    assert free >= 8 * 1024**3, ('free floor', free)
    assert time.time() < args.cutoff, 'active cutoff'

try:
    frozen = json.loads((OUT / args.freeze).read_text())
    for name, expected in frozen['controls'].items():
        assert hashlib.sha256((OUT / name).read_bytes()).hexdigest() == expected, ('control pin', name)
    for name, expected in frozen['readset'].items():
        assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == expected, ('source pin', name)
    for name, expected in frozen.get('externalExecutables', {}).items():
        assert hashlib.sha256(Path(name).read_bytes()).hexdigest() == expected, ('executable pin', name)
    sys.dont_write_bytecode = True
    shared = runpy.run_path(str(OUT / 'guardian.py'))
    shared['publish_accepted'](shared_config, os.getpid(), shared['process_rows'](), OUT, bootstrap_path, scratch)
    measure()
    info['primaryInitial'] = dict(sampled=time.time(), peaks=dict(info['peaks']), globalState=info['lastGlobal'])
    start_free = shutil.disk_usage(ROOT).free
    assert admission.get('startFloorGiB', 12) == args.start_floor
    assert start_free >= args.start_floor * 1024**3, ('fresh start floor', start_free, args.start_floor)
    if args.job == 'browser':
        assert 'otherWorstGrowth' in admission
    other_growth = admission.get('otherWorstGrowth', 0)
    assert start_free - other_growth - args.growth >= 9 * 1024**3, ('reserved floor', start_free, other_growth, args.growth)
    info.update(startFree=start_free, otherWorstGrowth=other_growth, startFloorGiB=args.start_floor)
    scratch.mkdir(); LINK.symlink_to(DEPS, target_is_directory=True)
    guardian_file = OUT / 'guardian.py'
    guardian_config = OUT / (args.attempt + '-guardian-config.json')
    guardian_config.write_text(json.dumps(dict(root=str(ROOT), scratch=str(scratch), output=str(guardian_output),
        supervisor=os.getpid(), cutoff=args.cutoff, processes=args.processes, rss=args.rss, temporary=args.temporary,
        whole=args.whole, growth=args.growth, durable=args.durable, files=args.files, admission=admission,
        commonGit=str(gitdir), worktreeGit=sp.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=ROOT, text=True).strip(),
        evidence=str(OUT), typeConfig=str(type_config), carryBytes=carry_bytes, carryFiles=carry_files,
        **{**shared_config, 'censusPath': str(OUT / (args.attempt + '-independent-census.json'))}, guardianSha256=hashlib.sha256(guardian_file.read_bytes()).hexdigest(),
        supervisorFile=str(Path(__file__)), supervisorSha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()), indent=2) + '\n')
    guardian = sp.Popen([sys.executable, '-B', str(guardian_file), str(guardian_config)], cwd=ROOT)
    known.add(guardian.pid)
    while not Path(str(guardian_output) + '.ready').exists():
        assert guardian.poll() is None, 'independent guardian did not become ready'
        assert time.time() < args.cutoff
        time.sleep(.02)
    info['independentInitial'] = json.loads(Path(str(guardian_output) + '.ready').read_text())
    assert info['independentInitial']['values'], 'independent initial sample missing'
    assert guardian.poll() is None
    measure()
    shared['publish_accepted'](shared_config, os.getpid(), shared['process_rows'](), OUT, bootstrap_path, scratch)
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE='1', PATH=str(NODE.parent) + ':' + os.environ['PATH'], TMPDIR=str(scratch), XDG_CACHE_HOME=str(scratch / 'cache'), MCR_PI12_PREWORLD_TEMP=str(scratch), MCR_PI12_PREWORLD_OUTPUT=str(OUT / (args.attempt + '-' + args.job)))
    assert 0 <= args.retained_png_count < args.family_png_cap and 0 <= args.retained_png_bytes < 16 * 1024**2
    env.update(MCR_PI12_PREWORLD_PNG_CAP=str(args.family_png_cap), MCR_PI12_PREWORLD_RETAINED_PNG_COUNT=str(args.retained_png_count), MCR_PI12_PREWORLD_RETAINED_PNG_BYTES=str(args.retained_png_bytes))
    info['pngFamily'] = dict(cap=args.family_png_cap, retainedCount=args.retained_png_count, retainedBytes=args.retained_png_bytes)
    if args.job == 'browser':
        foundation = json.loads((OUT / 'foundation-inputs.json').read_text())
        pins = []
        for row in foundation['readOnlyCompatibility']:
            assert hashlib.sha256((ROOT / row['path']).read_bytes()).hexdigest() == row['sha256'], ('PI01 compatibility', row['path'])
        for row in foundation['implementation']:
            assert guardian.poll() is None
            measure()
            data = sp.check_output(['git', 'show', foundation['source'] + ':' + row['path']], cwd=ROOT)
            assert len(data) == row['bytes'] and hashlib.sha256(data).hexdigest() == row['sha256']
            target = scratch / 'foundation' / row['path']
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            pins.append({**row, 'file': str(target)})
        manifest = scratch / 'foundation-inputs.json'
        manifest.write_text(json.dumps(pins))
        env.update(MCR_PI_FOUNDATION_INPUTS=str(manifest), MCR_PI_CANDIDATE=args.head)
        info.update(foundationSource=foundation['source'], foundationPins=pins, foundationReadsetSha256=foundation['readsetSha256'])
        measure()
    if args.job in ('types', 'source03'):
        config_file = type_config
        type_config_created = True
        config_file.write_text(json.dumps({'extends': str(ROOT / 'frontend/tsconfig.json'),
            'compilerOptions': {'paths': {'zod': [str(LINK / 'zod')]}}}, indent=2) + '\n')
        info['temporaryTypeConfig'] = {'content': json.loads(config_file.read_text()), 'sha256': hashlib.sha256(config_file.read_bytes()).hexdigest()}
        measure()
    info['checks'] = []
    planned_jobs = ['lint', 'syntax', 'types'] if args.job == 'source03' else ['decoder', 'browser'] if args.job == 'browser' else [args.job]
    if args.retain_decoder_pass:
        assert args.job == 'browser'
        prior = json.loads((OUT / 'native05.json').read_text())
        prior_freeze = json.loads((OUT / 'native05-freeze.json').read_text())
        assert prior['offer'] == 'PREWORLD-NATIVE05-0226' and any(check['job'] == 'decoder' and check['exitCode'] == 0 for check in prior['checks'])
        assert hashlib.sha256((OUT / 'receipt-path-regression.py').read_bytes()).hexdigest() == prior_freeze['controls']['receipt-path-regression.py']
        info['retainedDecoderQualification'] = dict(receipt='native05.json', cases=6, exitCode=0, unchangedSourceSha256=prior_freeze['controls']['receipt-path-regression.py'])
        planned_jobs = ['driverLint', 'browser']
    if args.check_completed_finisher:
        assert args.retain_decoder_pass and args.job == 'browser'
        planned_jobs.insert(0, 'finisher')
    if args.retain_native09_controls:
        assert args.retain_decoder_pass and args.job == 'browser' and not args.check_completed_finisher
        qualified = json.loads((OUT / 'native09.json').read_text())
        qualified_freeze = json.loads((OUT / 'native09-freeze.json').read_text())
        assert qualified['offer'] == 'PREWORLD-NATIVE09-0301'
        assert all(any(check['job'] == name and check['exitCode'] == 0 for check in qualified['checks']) for name in ('finisher', 'driverLint'))
        for name in ('guardian.py', 'completed-finisher-regression.py'):
            assert hashlib.sha256((OUT / name).read_bytes()).hexdigest() == qualified_freeze['controls'][name]
        driver_name = 'frontend/e2e/visual/parade-ink-pre-world-policy/native.mjs'
        assert hashlib.sha256((ROOT / driver_name).read_bytes()).hexdigest() == qualified_freeze['readset'][driver_name]
        info['retainedNative09Qualification'] = dict(receipt='native09.json', finisherCases=5, driverLintExitCode=0,
            unchangedGuardianSHA256=qualified_freeze['controls']['guardian.py'], unchangedDriverSHA256=qualified_freeze['readset'][driver_name])
        planned_jobs = ['browser']
    with log.open('w') as output:
        for job_name in planned_jobs:
            assert guardian.poll() is None
            measure()
            output.write('CHECK ' + job_name + '\n'); output.flush()
            child = sp.Popen(jobs[job_name], cwd=ROOT, env=env, stdout=output, stderr=sp.STDOUT, start_new_session=True)
            known.add(child.pid)
            while child.poll() is None:
                sampled = time.monotonic()
                assert guardian.poll() is None, 'independent guardian exited while child active'
                measure(); time.sleep(max(0, .25 - (time.monotonic()-sampled)))
            info['checks'].append({'job': job_name, 'argv': jobs[job_name], 'exitCode': child.returncode, 'finished': time.time()})
            measure()
        info['exitCode'] = max(check['exitCode'] for check in info['checks'])
except BaseException as error:
    import traceback
    info['guardFailure'] = repr(error)
    info['guardTraceback'] = traceback.format_exc()
    raise
finally:
    if guardian and guardian.poll() is None:
        Path(str(guardian_output) + '.stop').write_text(str(time.time()))
        guardian.wait(timeout=2)
    if child:
        for sig in [signal.SIGTERM, signal.SIGKILL]:
            process_rows()
            for pid in known - {os.getpid()}:
                try:
                    os.kill(pid, sig)
                except ProcessLookupError:
                    pass
            try:
                os.killpg(child.pid, sig)
            except ProcessLookupError:
                pass
            try:
                child.wait(timeout=2)
            except sp.TimeoutExpired:
                pass
    if LINK.is_symlink() and LINK.resolve() == DEPS:
        LINK.unlink()
    if type_config_created and type_config.is_file() and not type_config.is_symlink():
        type_config.unlink()
    if scratch.exists():
        shutil.rmtree(scratch)
    info.update(finished=time.time(), childrenRemaining=[p for p, _ in process_rows() if p != os.getpid()], linkAbsent=not LINK.exists() and not LINK.is_symlink(), scratchAbsent=not scratch.exists(), typeConfigAbsent=not type_config.exists(), freeAfter=shutil.disk_usage(ROOT).free)
    info['releasedBeforeDeadline'] = info['finished'] < args.release
    receipt.write_text(json.dumps(info, indent=2) + '\n')
    if args.bootstrap_work is not None and not info['childrenRemaining'] and info['linkAbsent'] and info['scratchAbsent'] and info['typeConfigAbsent']:
        full_path = OUT / (args.attempt + '-FULL.json')
        full_path.write_text(json.dumps({'state': 'FULL', 'offer': args.offer, 'finished': info['finished'], 'resources': dict(children=0, link=0, temporary=0, reservations=0), 'receipt': str(receipt), 'exitCode': info['exitCode'], 'guardFailure': info.get('guardFailure')}, indent=2) + '\n')
        owner_path = shared['RECEIPTS'] / (shared['OWN_ID'] + '.json')
        record = json.loads(owner_path.read_text()); record.update(state='FULL', actualFullEpoch=info['finished'], remainingNewUpperBytes=0, resources=dict(children=0, link=0, temporary=0, reservations=0)); record['receiptPaths']['full'] = str(full_path)
        staging = owner_path.with_name(owner_path.name + '.' + str(os.getpid()) + '.tmp'); staging.write_text(json.dumps(record, indent=2) + '\n'); staging.replace(owner_path)
    print(json.dumps({'receipt': str(receipt), 'checks': info.get('checks'), 'exitCode': info['exitCode'], 'guardFailure': info.get('guardFailure'), 'finished': info['finished'], 'releasedBeforeDeadline': info['releasedBeforeDeadline']}))
    assert not info['childrenRemaining'] and info['linkAbsent'] and info['scratchAbsent'] and info['typeConfigAbsent'] and info['releasedBeforeDeadline']
raise SystemExit(info['exitCode'])
