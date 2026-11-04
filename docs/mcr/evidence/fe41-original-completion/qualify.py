"""FE41 changed-only qualification; fresh Social allocation required before execution.
Reuses FE17 process/inventory primitives, extending history custody to preserved refs/stash.
"""
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
EVIDENCE = ROOT / 'docs/mcr/evidence/fe41-original-completion'
H = runpy.run_path(str(ROOT / 'docs/mcr/evidence/fe17-original-completion/qualify-source.py'), run_name='fe41_primitives')
NODE, DEPS = H['NODE'], H['DEPS']
BASE = 'ba314a2c96639b05b6c15c1ead57353719004ffa'

def inventory():
    refs = [H['git']('rev-parse', 'HEAD'), '8ca330d8966e4551ef0fade957e5ce907646108b',
            'defdc8fc429e658c7f53523304208056477c6a88', 'f543d43a96848d47021eaa4b0736a27e548ebadc']
    objects = {line.split()[0] for line in H['git']('rev-list', '--objects', *refs).splitlines()}
    sizes = sp.check_output(['git', 'cat-file', '--batch-check=%(objectsize)'], cwd=ROOT,
                            input='\n'.join(sorted(objects))+'\n', text=True)
    roots = [H['root_inventory'](p) for p in [ROOT, DEPS, Path(H['git']('rev-parse', '--absolute-git-dir'))]]
    history = sum(int(row) for row in sizes.splitlines())
    return {'roots': roots, 'historyRefs': refs, 'historyBytes': history, 'historyObjects': len(objects),
            'wholeBytes': history+sum(row['logicalBytes'] for row in roots),
            'entries': len(objects)+sum(row['files']+row['symlinks'] for row in roots)}

def identity(pid):
    p = sp.run(['ps', '-p', str(pid), '-o', 'lstart=', '-o', 'comm='], capture_output=True, text=True)
    return p.stdout.strip() if p.returncode == 0 else None

def commands(mode, tmp):
    if mode == 'native':
        return [('native', [str(NODE), str(ROOT / 'frontend/src/app/errors/fe41-lifecycle/native.mjs')])]
    if os.environ.get('MCR_FE41_HARNESS_TAIL') == '1':
        harness = str(ROOT / 'frontend/src/app/errors/fe41-lifecycle/native.mjs')
        return [('final-harness-lint', [str(NODE), str(DEPS/'oxlint/bin/oxlint'), '--deny-warnings', harness]),
                ('final-harness-syntax', [str(NODE), '--check', harness])]
    helper = ROOT / 'frontend/src/world/avatar/__tests__/motion-adoption/typecheck.mjs'
    type_source = """import assert from 'node:assert/strict'; import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto'; import {dirname} from 'node:path';
const helper=HELPER; const original=readFileSync(helper,'utf8');
assert.equal(createHash('sha256').update(original).digest('hex'),'4cbcb57e175101a3fabbcbd67604c223d2604e24732f9d6e3331583a0d624aa1');
const source=original.replace('...parsed.options, incremental:','...parsed.options, allowImportingTsExtensions: true, incremental:')
.replaceAll('import.meta.dirname',JSON.stringify(dirname(helper))); process.argv.push('--application');
await import('data:text/javascript;base64,'+Buffer.from(source).toString('base64'));""".replace('HELPER', json.dumps(str(helper)))
    changed = json.loads((EVIDENCE / 'readset.json').read_text())['authoredApplicationFiles']
    return [
      ('tests', [str(NODE), str(DEPS/'vitest/vitest.mjs'), 'run', '--configLoader', 'runner', '--root', str(ROOT/'frontend'), '--config', str(EVIDENCE/'vitest.config.mjs')]),
      ('types', [str(NODE), '--input-type=module', '--eval', type_source]),
      ('lint', [str(NODE), str(DEPS/'oxlint/bin/oxlint'), '--deny-warnings', *[str(ROOT/p) for p in changed], str(EVIDENCE/'vitest.config.mjs'), str(EVIDENCE/'setup.ts')]),
      ('native-syntax', [str(NODE), '--check', str(ROOT/'frontend/src/app/errors/fe41-lifecycle/native.mjs')]),
      ('config-syntax', [str(NODE), '--check', str(EVIDENCE/'vitest.config.mjs')]),
      ('whitespace', ['git', 'diff', '--check', BASE, '--', '.', ':(glob,exclude)docs/mcr/evidence/fe41-original-completion/**/*.log']),
      ('plan', ['python3', str(ROOT/'scripts/mcr/check-plan.py')]),
    ]

def main():
    slot, pin, mode = [os.environ['MCR_FE41_'+key] for key in ['SLOT','PIN','MODE']]
    deadline_text = os.environ['MCR_FE41_DEADLINE']; deadline = dt.datetime.fromisoformat(deadline_text).timestamp()
    assert mode in ['source','native'] and Path.cwd() == ROOT and H['git']('rev-parse','HEAD') == pin
    assert hashlib.sha256(NODE.read_bytes()).hexdigest() == H['NODE_SHA']
    assert 60 < deadline-time.time() <= 240
    output = EVIDENCE / os.environ['MCR_FE41_ATTEMPT']; assert output.parent == EVIDENCE and not output.exists()
    previous = os.environ.get('MCR_FE41_PREVIOUS')
    if previous:
        prior = json.loads((EVIDENCE/previous/'supervisor.json').read_text())
        assert prior['slot'] == slot and prior['deadlineUTC'] == deadline_text and prior['mode'] == mode
        assert prior['status'] == 'FAIL' and not prior['alivePids'] and prior['temporaryAbsent']
        assert not prior['externalRemaining']
    pins = json.loads((EVIDENCE/'readset.json').read_text())
    allowed = set(pins['authoredApplicationFiles'])
    allowed.add('docs/mcr/frontend/fe41-original-completion.md')
    changed = H['git']('diff', '--name-only', BASE).splitlines()
    assert all(path in allowed or path.startswith('docs/mcr/evidence/fe41-original-completion/') for path in changed)
    dirty = H['git']('status', '--porcelain', '--untracked-files=all').splitlines()
    assert all(row[3:].startswith('docs/mcr/evidence/fe41-original-completion/') for row in dirty)
    for path, digest in pins['files'].items(): assert hashlib.sha256((ROOT/path).read_bytes()).hexdigest() == digest, path
    current = inventory(); carry = max(current['wholeBytes'], int(os.environ.get('MCR_FE41_WHOLE_FLOOR','0')))
    caps = {'whole': 4*1024**3, 'entries': 65536, 'new':128*1024**2, 'temporary':96*1024**2,
            'durable':32*1024**2, 'rss':int((3.5 if mode=='native' else 2.5)*1024**3), 'processes':16 if mode=='native' else 8, 'childSeconds':90}
    other = int(os.environ['MCR_FE41_OTHER_RESERVE']); assert other >= 0
    free = shutil.disk_usage(ROOT).free
    start_floor = 8 if previous else max(11, int(os.environ.get('MCR_FE41_START_GIB','11')))
    assert free >= start_floor*1024**3 and free-other-caps['new'] >= 9*1024**3
    assert carry < caps['whole'] and current['entries'] < caps['entries']
    link, borrowed = ROOT/'frontend/node_modules', DEPS/'.vite-temp'
    assert not link.exists() and not link.is_symlink()
    assert not borrowed.exists() or not list(borrowed.iterdir())
    port = int(os.environ.get('MCR_FE41_PORT','64379'))
    if mode == 'native':
        with socket.socket() as s:
            s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); s.bind(('127.0.0.1',port))
    output.mkdir(); tmp = Path(tempfile.mkdtemp(prefix='mcr-fe41-', dir='/private/tmp'))
    env = {**os.environ,'PATH':str(NODE.parent)+':'+os.environ.get('PATH',''), 'TMPDIR':str(tmp), 'TMP':str(tmp),'TEMP':str(tmp),
           'XDG_CACHE_HOME':str(tmp/'cache'),'MCR_FRONTEND_DEPENDENCIES':str(DEPS.parent),
           'MCR_FE41_CACHE':str(tmp/'vite'),'MCR_FE41_OUTPUT':str(output/'native')}
    cmds = commands(mode,tmp)
    labels = os.environ.get('MCR_FE41_LABELS')
    if labels:
        assert previous
        selected = labels.split(','); assert set(selected) <= {label for label,_ in cmds}
        cmds = [(label,argv) for label,argv in cmds if label in selected]
    report = {'slot':slot,'pin':pin,'base':BASE,'mode':mode,'deadlineUTC':deadline_text,
              'startedUTC':dt.datetime.now(dt.timezone.utc).isoformat(),'caps':caps,'inventory':current,'carry':carry,'otherReserve':other,
              'temporary':str(tmp),'commands':[],'ownedPids':{},'externalTargets':[],'wholePeak':carry,'newPeak':0,'tmpPeak':0,'durablePeak':0,'rssPeak':0,'processPeak':0,'entryPeak':current['entries']}
    child, owned, external, result = None, {}, set(), 'FAIL'
    try:
        for label,argv in cmds:
            assert H['git']('rev-parse','HEAD') == pin and time.time() < deadline-60
            with (output/(label+'.log')).open('wb') as log:
                child = sp.Popen(argv,cwd=ROOT,env=env,stdout=log,stderr=sp.STDOUT,start_new_session=True)
                started = time.time(); row = {'label':label,'argv':argv,'pid':child.pid,'startedEpoch':started}; report['commands'].append(row)
                while child.poll() is None:
                    sweep=time.monotonic(); rows=H['processes'](); ids=H['descendants'](os.getpid(),rows)
                    for pid in ids-{os.getpid()}:
                        if pid not in owned:
                            value=identity(pid)
                            if value: owned[pid]=value
                    for directory,dirs,names in os.walk(tmp,followlinks=False):
                        for name in dirs+names:
                            p=Path(directory)/name
                            if name == 'SingletonSocket' and p.is_symlink():
                                target=(p.parent/os.readlink(p)).resolve()
                                if not target.is_relative_to(tmp):
                                    external.add(target.parent)
                                    report['externalTargets']=sorted(str(x) for x in external)
                                    assert target.name == 'SingletonSocket' and ('scoped_dir' in target.parent.name or target.parent.name.startswith('com.google.Chrome.')), 'Unconfirmed own-profile target; preserve'
                    temporary=H['bytes_in'](tmp)+sum(H['bytes_in'](p) for p in external); durable=H['bytes_in'](output)
                    entries=current['entries']+sum(sum(H['root_inventory'](p)[k] for k in ['files','symlinks']) for p in [tmp,output,*external])
                    rss=sum(r*1024 for p,_,_,r in rows if p in ids); free=shutil.disk_usage(ROOT).free
                    for key,value in [('wholePeak',carry+temporary+durable),('newPeak',temporary+durable),('tmpPeak',temporary),('durablePeak',durable),('rssPeak',rss),('processPeak',len(ids)),('entryPeak',entries)]: report[key]=max(report[key],value)
                    if time.time()>=deadline-60 or time.time()-started>=caps['childSeconds'] or carry+temporary+durable>caps['whole'] or temporary+durable>caps['new'] or temporary>caps['temporary'] or durable>caps['durable'] or rss>caps['rss'] or len(ids)>caps['processes'] or entries>caps['entries'] or free<8*1024**3 or free-other-caps['new']<9*1024**3: raise RuntimeError('Accepted clock/process/RSS/WHOLE/new/entries/free bound')
                    time.sleep(max(0,.25-(time.monotonic()-sweep)))
                row.update(exit=child.returncode,endedEpoch=time.time())
                assert child.returncode==0, label+' failed; immutable output retained'
            H['retire'](child)
        result='PASS'
    except BaseException as exc: report['failure']=type(exc).__name__+': '+str(exc)
    finally:
        H['retire'](child)
        # Chrome helpers can leave the direct child's process group. Retire only
        # previously observed identities, never a reused PID or peer process.
        for sig in [signal.SIGTERM, signal.SIGKILL]:
            for pid,value in owned.items():
                if identity(pid)==value:
                    try: os.kill(pid,sig)
                    except ProcessLookupError: pass
            until=min(deadline,time.time()+2)
            while time.time()<until and any(identity(pid)==value for pid,value in owned.items()): time.sleep(.1)
        report['ownedPids']={str(pid):value for pid,value in owned.items()}
        report['alivePids']=[pid for pid,value in owned.items() if identity(pid)==value]
        shutil.rmtree(tmp)
        report['externalRemaining']=[str(p) for p in external if p.exists()]
        report['temporaryAbsent']=not tmp.exists(); report['linkAbsent']=not link.exists() and not link.is_symlink()
        report['borrowedViteTempEmpty']=not borrowed.exists() or not list(borrowed.iterdir())
        if mode=='native':
            try:
                with socket.socket() as s:
                    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); s.bind(('127.0.0.1',port))
                listening = sp.run(['lsof','-nP','-iTCP:'+str(port),'-sTCP:LISTEN'],capture_output=True,text=True)
                assert listening.returncode == 1 and not listening.stdout
                report['portReuse']='PASS'
            except (OSError, AssertionError) as exc: report['portReuse']=str(exc); result='FAIL'
        if report['alivePids'] or report['externalRemaining'] or not report['linkAbsent'] or not report['borrowedViteTempEmpty']: result='FAIL'
        report.update(status=result,finishedUTC=dt.datetime.now(dt.timezone.utc).isoformat(),finalFree=shutil.disk_usage(ROOT).free)
        (output/'supervisor.json').write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps(report,indent=2))
    return 0 if result=='PASS' else 1

if __name__=='__main__': raise SystemExit(main())
