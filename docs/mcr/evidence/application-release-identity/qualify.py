"""One real-source manifest, retained five-case suite and scoped source checks."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
OUT = Path(os.environ['MCR_RELEASE_OUTPUT'])
NODE = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node')
DEPS = Path('/Users/lhs/Desktop/mcr/frontend/node_modules')
SOURCE = os.environ['MCR_RELEASE_SOURCE']
CANDIDATE = os.environ['MCR_RELEASE_CANONICAL']
assert (OUT.parent == HERE or OUT.parent.parent == ROOT/'docs/mcr/evidence/c07-be38-public-wire') and SOURCE == subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
assert not subprocess.check_output(['git', 'diff', '--name-only', 'HEAD'], cwd=ROOT)
assert all(path.startswith(('docs/mcr/evidence/application-release-identity/', 'docs/mcr/evidence/c07-be38-public-wire/')) for path in subprocess.check_output(
    ['git', 'ls-files', '--others', '--exclude-standard'], cwd=ROOT, text=True).splitlines())
readset = json.loads((HERE / 'readset.json').read_text())
assert all(hashlib.sha256((ROOT/path).read_bytes()).hexdigest() == value for path, value in readset.items())
free = shutil.disk_usage(ROOT).free
assert free >= int(os.environ['MCR_RELEASE_START_FLOOR_GIB'])*1024**3
assert free-int(os.environ['MCR_RELEASE_OTHER_NEW'])-128*1024**2 >= 9*1024**3
report = dict(task='APPLICATION-RELEASE-IDENTITY', source=SOURCE, candidate=CANDIDATE, status='IN_PROGRESS', commands=[], started=time.time(), runtime='NOT_RUN', emittedOutputs='NOT_BUILT_BY_THIS_PUBLISHER')
env = {**os.environ, 'MCR_IN04_SOURCE_REPO': str(ROOT), 'MCR_IN04_SOURCE_CANDIDATE': CANDIDATE,
       'MCR_APPLICATION_MANIFEST_PATH': str(OUT/'application-release.json'), 'NO_COLOR': '1'}
env.pop('NODE_COMPILE_CACHE', None)
def run(name, args):
    assert time.time() < float(os.environ['MCR_RELEASE_DEADLINE'])-60
    assert shutil.disk_usage(ROOT).free >= 8*1024**3
    result = subprocess.run([str(arg) for arg in args], cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=180)
    (OUT/(name+'.log')).write_text(result.stdout)
    report['commands'].append(dict(name=name, argv=[str(arg) for arg in args], exit=result.returncode))
    assert result.returncode == 0, name + ' failed; original output retained'
base = ROOT/'frontend/e2e/runtime-adapters/in04-wall-b'
try:
    run('manifest', [NODE, base/'application-release.mjs', ROOT, CANDIDATE, DEPS, 'vite-serve', OUT/'application-release.json'])
    run('five-cases', [NODE, '--test', '--test-isolation=none', base/'application-release.test.mjs'])
    for filename in ('application-release.mjs', 'application-release.test.mjs', 'server.mjs'):
        run('syntax-'+filename, [NODE, '--check', base/filename])
    run('lint', [NODE, DEPS/'oxlint/bin/oxlint', '--deny-warnings', base/'application-release.mjs', base/'application-release.test.mjs', base/'server.mjs', ROOT/'frontend/src/shared/release/application.ts'])
    assert all(hashlib.sha256((ROOT/path).read_bytes()).hexdigest() == value for path, value in readset.items())
    manifest = json.loads((OUT/'application-release.json').read_text())
    report.update(status='PASS', applicationVersion=manifest['applicationVersion'], buildId=manifest['buildId'], inputs=len(manifest['inputs']), inputBytes=sum(x['bytes'] for x in manifest['inputs']), manifestSha256=hashlib.sha256((OUT/'application-release.json').read_bytes()).hexdigest())
except Exception as failure:
    report.update(status='FAIL', error=repr(failure))
finally:
    report['finished'] = time.time()
    (OUT/'qualification.json').write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps(dict(status=report['status'], error=report.get('error'), output=str(OUT))))
raise SystemExit(0 if report['status'] == 'PASS' else 1)
