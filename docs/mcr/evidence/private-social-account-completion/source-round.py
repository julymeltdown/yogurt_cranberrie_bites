"""Finite FE31/35/36 source round. Requires a fresh named allocator offer.

Adapts the process-group cleanup and disk/RSS guards from the retained
fe41-default-entry supervisor. No install, build, browser or service jobs.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
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
args = parser.parse_args()
assert args.attempt.isalnum() and args.cutoff < args.release
features = ['social-sharing', 'inbox', 'safety', 'account']
focused = ['src/features/' + name + '/__tests__/completion.test.tsx' for name in features[1:]]
tests = [str(NODE), str(LINK / 'vitest/vitest.mjs'), 'run', '--root', 'frontend', '--configLoader', 'runner', '--no-cache', '--maxWorkers', '1']
jobs = {
    'focus': tests + focused,
    'regression': tests + ['src/features/' + name + '/__tests__' for name in features] + ['src/app/social-composition/__tests__'],
    'types': [str(NODE), str(LINK / 'typescript/bin/tsc'), '--noEmit', '--incremental', 'false', '--project', 'frontend/tsconfig.json'],
    'ownedtypes': [str(NODE), str(OUT / 'owned-types.mjs')],
    'lint': [str(NODE), str(LINK / 'oxlint/bin/oxlint'), '--deny-warnings'] + ['frontend/src/features/' + name for name in features[1:]],
    'plan': ['python3', 'scripts/mcr/check-plan.py'],
}
assert args.job in jobs
assert sp.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == args.head
assert not LINK.exists() and not LINK.is_symlink()
scratch = Path('/private/tmp') / ('mcr-private-completion-' + args.attempt)
assert not scratch.exists()
log = OUT / (args.attempt + '.log')
receipt = OUT / (args.attempt + '.json')
assert not log.exists() and not receipt.exists()

def inventory(root, exclude=()):
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
source_files, source_bytes = inventory(ROOT, ('.git', 'node_modules', 'target'))
gitdir = Path(sp.check_output(['git', 'rev-parse', '--git-common-dir'], cwd=ROOT, text=True).strip())
git_files, git_bytes = inventory(gitdir / 'objects')
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
initial_out_files, initial_out_bytes = admission['outFiles'], admission['outBytes']
info = dict(offer=args.offer, head=args.head, job=args.job, argv=jobs[args.job], started=time.time(), cutoff=args.cutoff, release=args.release,
            carryFiles=carry_files, carryBytes=carry_bytes, dependencyFiles=dependency_files, dependencyBytes=dependency_bytes, peaks={}, minFree=2**63-1, exitCode=None,
            sourcePins={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for name in features for p in (ROOT / 'frontend/src/features' / name).rglob('*') if p.is_file()},
            locks={p:hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in ['frontend/pnpm-lock.yaml', 'frontend/package.json']})
child = None
known = {os.getpid()}

def process_rows():
    probe = sp.Popen(['ps', '-axo', 'pid=,ppid=,pgid=,rss='], stdout=sp.PIPE, text=True)
    output, _ = probe.communicate()
    rows = [tuple(map(int, r.split())) for r in output.splitlines() if r.strip()]
    rows = [r for r in rows if r[0] != probe.pid]
    known.discard(probe.pid)
    while True:
        new = known | {pid for pid, parent, group, _ in rows if parent in known or child and group == child.pid}
        if new == known:
            break
        known.update(new)
    return [(pid, rss * 1024) for pid, _, _, rss in rows if pid in known]

def measure():
    tf, tb = inventory(scratch); df, db = inventory(OUT)
    sf, sb = inventory(ROOT, ('.git', 'node_modules', 'target'))
    gf, gb = inventory(gitdir / 'objects')
    mf, mb = inventory(Path(sp.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=ROOT, text=True).strip()))
    # Borrowed dependencies are read-only. Source and Git growth are measured
    # continuously; positive growth never subtracts deleted historical output.
    positive_bytes = max(0, sb-source_bytes) + max(0, gb-git_bytes) + max(0, mb-metadata_bytes)
    positive_files = max(0, sf-source_files) + max(0, gf-git_files) + max(0, mf-metadata_files)
    processes = process_rows()
    values = dict(whole=carry_bytes + tb + positive_bytes, growth=tb+positive_bytes, temporary=tb, durable=max(0, db-initial_out_bytes),
                  files=carry_files + tf + positive_files, processes=len(processes), rss=sum(r for _, r in processes))
    for key, value in values.items():
        info['peaks'][key] = max(info['peaks'].get(key, 0), value)
        assert value <= getattr(args, key), (key, value)
    free = shutil.disk_usage(ROOT).free
    info['minFree'] = min(info['minFree'], free)
    assert free >= 8 * 1024**3, ('free floor', free)
    assert time.time() < args.cutoff, 'active cutoff'

try:
    measure()
    scratch.mkdir(); LINK.symlink_to(DEPS, target_is_directory=True)
    env = dict(os.environ, PATH=str(NODE.parent) + ':' + os.environ['PATH'], TMPDIR=str(scratch), XDG_CACHE_HOME=str(scratch / 'cache'), MCR_PRIVATE_COMPLETION_FRONTEND=str(ROOT / 'frontend'))
    with log.open('w') as output:
        child = sp.Popen(jobs[args.job], cwd=ROOT, env=env, stdout=output, stderr=sp.STDOUT, start_new_session=True)
        known.add(child.pid)
        while child.poll() is None:
            sampled = time.monotonic()
            measure(); time.sleep(max(0, .25 - (time.monotonic()-sampled)))
        info['exitCode'] = child.returncode
        measure()
except BaseException as error:
    info['guardFailure'] = str(error)
    raise
finally:
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
    if scratch.exists():
        shutil.rmtree(scratch)
    info.update(finished=time.time(), childrenRemaining=[p for p, _ in process_rows() if p != os.getpid()], linkAbsent=not LINK.exists() and not LINK.is_symlink(), scratchAbsent=not scratch.exists(), freeAfter=shutil.disk_usage(ROOT).free)
    info['releasedBeforeDeadline'] = info['finished'] < args.release
    receipt.write_text(json.dumps(info, indent=2) + '\n')
    print(json.dumps(info))
    assert not info['childrenRemaining'] and info['linkAbsent'] and info['scratchAbsent'] and info['releasedBeforeDeadline']
raise SystemExit(info['exitCode'])
