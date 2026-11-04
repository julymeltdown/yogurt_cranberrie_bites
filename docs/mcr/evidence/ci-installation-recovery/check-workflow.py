"""Finite source check; no Corepack execution, dependency install or app tests."""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time

p = argparse.ArgumentParser()
p.add_argument('--job', required=True)
p.add_argument('--deadline', type=float, required=True)
p.add_argument('--aggregate-reserve', type=int, required=True)
a = p.parse_args()
root = Path(__file__).resolve().parents[4]
out = Path(__file__).with_name('local-check')
assert not out.exists() and 0 < a.deadline - time.time() <= 30
assert shutil.disk_usage(root).free >= 12 * 1024**3
assert shutil.disk_usage(root).free - a.aggregate_reserve >= 9 * 1024**3
def size(path):
    total = 0
    for parent, dirs, files in os.walk(path, followlinks=False):
        dirs[:] = [d for d in dirs if not (Path(parent) / d).is_symlink()]
        for name in files:
            try:
                stat = (Path(parent) / name).lstat()
                total += max(stat.st_size, stat.st_blocks * 512)
            except FileNotFoundError:
                pass
    return total
gitdir = Path(subprocess.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=root, text=True).strip())
carried = size(root) + size(gitdir)
assert carried + 2 * 1024**2 < 2 * 1024**3
out.mkdir()
code = r'''
require 'yaml'
require 'json'
require 'open3'
require 'digest'
base = 'b97608d4235eb7f82cba29c4807e219f7f606e3c'
path = '.github/workflows/mcr-contracts.yml'
original, status = Open3.capture2('git', 'show', base + ':' + path)
raise 'Missing base' unless status.success?
before = YAML.safe_load(original)
after = YAML.safe_load(File.read(path))
steps = after.fetch('jobs').fetch('contracts').fetch('steps')
install = steps.fetch(5)
raise 'Corepack must start inside frontend' unless install.fetch('working-directory') == 'frontend'
expected = ['corepack enable', 'corepack pnpm --version', 'corepack pnpm install --frozen-lockfile']
raise 'Frozen install commands changed' unless install.fetch('run').lines.map(&:strip) == expected
before.fetch('jobs').fetch('contracts').fetch('steps')[5] = install
raise 'Unexpected workflow changes outside install step' unless before == after
shell, error, status = Open3.capture3('/bin/bash', '-n', stdin_data: install.fetch('run'))
raise 'Shell syntax: ' + error unless status.success?
manifest = JSON.parse(File.read('frontend/package.json'))
raise 'Selected package-manager pin changed' unless manifest.fetch('packageManager') == 'pnpm@11.7.0'
changed, status = Open3.capture2('git', 'diff', '--name-only', base)
raise 'Source inventory failed' unless status.success?
paths = changed.lines.map(&:strip)
raise 'Out-of-scope edits' unless paths.all? {|f| f == path || f == 'docs/mcr/ci-installation-recovery.md' || f.start_with?('docs/mcr/evidence/ci-installation-recovery/')}
pins = ['frontend/package.json', 'frontend/pnpm-lock.yaml', 'scripts/mcr/requirements-contracts.txt',
        'scripts/mcr/contracts/c07/run.py', 'scripts/mcr/contracts/qa01-coverage/run.py',
        'scripts/mcr/contracts/qa01-coverage/test_coverage.py'].to_h {|f| [f, Digest::SHA256.file(f).hexdigest]}
puts JSON.pretty_generate({result: 'PASS', syntax: 'Ruby Psych YAML and bash -n',
  unchanged: 'All workflow configuration except the exact install step; all other tracked paths outside owned doc/evidence unchanged',
  packageManager: manifest.fetch('packageManager'), readset: pins,
  hostedQualification: 'NOT_RUN', localInstall: 'NOT_RUN', localContractReplay: 'NOT_RUN'})
'''
info = {'job': a.job, 'started': time.time(), 'workDeadline': a.deadline, 'cleanupDeadline': a.deadline + 20,
        'candidate': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
        'carriedWholeBytes': carried, 'aggregateReserve': a.aggregate_reserve,
        'caps': {'wholeBytes': 2 * 1024**3, 'newBytes': 2 * 1024**2, 'allPids': 8, 'rss': 256 * 1024**2},
        'peaks': {}, 'command': ['/usr/bin/ruby', '-e', 'embedded unchanged-workflow/YAML/shell check']}
seen = set()
def rows():
    return [list(map(int, x.split())) for x in subprocess.check_output(['ps', '-axo', 'pid,ppid,rss'], text=True).splitlines()[1:] if len(x.split()) == 3]
child = None
try:
    with (out / 'check.log').open('w') as log:
        child = subprocess.Popen(['/usr/bin/ruby', '-e', code], cwd=root, stdout=log, stderr=subprocess.STDOUT)
        seen.add(child.pid)
        while child.poll() is None:
            processes = rows()
            owned = {os.getpid()} | seen
            for _ in range(8):
                updated = owned | {pid for pid, parent, rss in processes if parent in owned}
                if updated == owned:
                    break
                owned = updated
            seen.update(owned - {os.getpid()})
            active = [x for x in processes if x[0] in owned]
            rss = sum(x[2] * 1024 for x in active)
            free = shutil.disk_usage(root).free
            info['minFree'] = min(info.get('minFree', free), free)
            info['peaks']['allPids'] = max(info['peaks'].get('allPids', 0), len(active))
            info['peaks']['rss'] = max(info['peaks'].get('rss', 0), rss)
            if len(active) > 8 or rss > 256 * 1024**2 or size(out) > 2 * 1024**2 or free < 8 * 1024**3 or time.time() >= a.deadline - 1:
                info['stopReason'] = 'process/RSS/output/free/deadline guard'
                for pid in seen:
                    try:
                        os.kill(pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass
                break
            time.sleep(.1)
        info['exitCode'] = child.wait(timeout=2)
finally:
    remaining = {x[0] for x in rows()} & seen
    for pid in remaining:
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    info.update(finished=time.time(), remainingOwnedPids=sorted({x[0] for x in rows()} & seen),
                newBytes=size(out), finalFree=shutil.disk_usage(root).free,
                cleanupWithinDeadline=time.time() < a.deadline + 20,
                ports='NONE', temp='NONE', dependencyLink='NOT_CREATED')
    info['FULL'] = not info['remainingOwnedPids'] and info['cleanupWithinDeadline']
    (out / 'guard.json').write_text(json.dumps(info, indent=2) + '\n')
    print(json.dumps(info))
raise SystemExit(info.get('exitCode', 1) if info['FULL'] else 2)
