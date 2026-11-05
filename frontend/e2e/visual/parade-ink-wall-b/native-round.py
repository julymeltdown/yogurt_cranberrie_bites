"""PI09 finite browser controller and independent guardian; no install/link/build."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time
sys.dont_write_bytecode = True
import accounting
import resource_receipts

GiB = 1024 ** 3
MiB = 1024 ** 2
HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
NODE_ROOT = Path('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64')
NODE = NODE_ROOT / 'bin/node'
TOOLING = Path('/Users/lhs/Desktop/mcr/frontend')
PYTHON = Path('/Library/Frameworks/Python.framework')
if '--guardian' not in sys.argv and '--preflight' not in sys.argv:
    # Publish this real process before Git/pin/history discovery. The later
    # atomic ACCEPT replaces preparation metadata before any heavy payload.
    resource_receipts.ordinary_preflight(HERE/'native-round.py')
GIT = Path(subprocess.check_output(['git', 'rev-parse', '--git-common-dir'], cwd=REPO, text=True).strip()).resolve()

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def rows():
    return resource_receipts.processes()

def descendants(data, root):
    found = {root}
    for _ in range(30):
        expanded = found | {p for p, parent, _, _ in data if parent in found}
        if expanded == found:
            return found
        found = expanded
    raise RuntimeError('Process-tree closure incomplete')

def capture(temp):
    paths = {'checkout': REPO, 'entireGit': GIT, 'entireNode': NODE_ROOT,
             'entirePython': PYTHON, 'readOnlyDependencies': TOOLING / 'node_modules', 'entireChrome': Path('/Applications/Google Chrome.app'), 'phaseTemporary': temp, 'receiptDiscovery': resource_receipts.BASE/'owner-resource-receipts'}
    return {name: accounting.RootIdentity.capture(path) for name, path in paths.items()}

def process_sample(controller):
    data = rows(); owned = descendants(data, controller)
    current = [row for row in data if row[0] in owned]
    global_roots = {p for p, _, _, command in data if '/mcr' in command and 'Codex.app' not in command}
    global_ids = set()
    for pid in global_roots:
        global_ids.update(descendants(data, pid))
    global_rows = [row for row in data if row[0] in global_ids]
    return {'pids': len(current), 'rss': sum(row[2] for row in current),
            'globalPids': len(global_rows), 'globalRss': sum(row[2] for row in global_rows),
            'ownedPids': sorted(owned), 'free': shutil.disk_usage(REPO).free}

def fast(config, require_peer=None):
    sample = process_sample(config['controller'])
    sample['cohorts'] = resource_receipts.census(config, rows())
    assert time.time() < config['workDeadline'], 'Absolute WORK deadline'
    assert sample['free'] >= 8 * GiB, '8GiB live floor'
    assert sample['pids'] <= 24 and sample['rss'] <= 4 * GiB, 'Owned PID/RSS cap'
    assert sample['globalPids'] <= 96 and sample['globalRss'] <= 18 * GiB, 'Global PID/RSS cap'
    if require_peer:
        os.kill(require_peer, 0)
    return sample

def full_sample(config, roots, carry, peer=None):
    sample = fast(config, peer)
    last_live = time.monotonic()
    def live_checkpoint():
        nonlocal last_live
        assert time.time() < config['workDeadline'], 'Absolute WORK deadline'
        if time.monotonic() - last_live >= .25:
            fast(config, peer); last_live = time.monotonic()
    allocation = accounting.collect(roots, deadline=time.monotonic() + 8, checkpoint=live_checkpoint)
    value = carry.observe(allocation['scopes']); sample.update(value)
    assert value['new'] <= config['newCap'], 'Accepted NEW cap'
    assert value['whole'] <= 8 * GiB, '8GiB WHOLE cap'
    remaining = max(0, config['newCap'] - value['new'])
    assert sample['free'] - remaining - config['otherNew'] >= 9 * GiB, '9GiB after all remaining reservations'
    pngs=list((REPO/'docs/mcr/evidence/parade-ink-wall-b').rglob('*.png'))
    assert len(pngs)<=24 and sum(p.stat().st_size for p in pngs)<=12*MiB, 'PNG count/byte cap'
    sample['time'] = time.time(); sample['allocation'] = allocation
    return sample

def retained_motif(path, config, output):
    allowed=REPO/'docs/mcr/evidence/parade-ink-wall-b/pi09-wallb-app-native07-0120/motif/report.partial.json'
    assert path.resolve()==allowed, 'Only the exact completed Native07 motif cases can be retained'
    report=json.loads(path.read_text());closed=json.loads((path.parent.parent/'FULL.json').read_text())
    assert closed['FULL'] is True and closed['temporaryAbsent'] is True and closed['remainingOwned']==[]
    assert closed['status']=='FAIL' and closed['error']=="AssertionError('Independent guardian failed')"
    assert report['errors']==[] and [c['width'] for c in report['cases']]==[360,390,430]
    assert all(c['status']=='PASS' and not c['errors'] and c['cleanup']['active']==0 and c['cleanup']['blobURLs']==0 for c in report['cases'])
    driver=(HERE/'app.mjs').read_text()
    changes=json.loads((HERE/'commands-driver-deltas.json').read_text())
    assert changes['qualification']=='EXACT_COMMANDS_ONLY_DELTAS_FROM_NATIVE07_MOTIF_DRIVER' and len(changes['deltas'])==6
    motif_driver=driver
    for delta in reversed(changes['deltas']):
        assert motif_driver.count(delta['after'])==1,'Exact declared command-only delta'
        motif_driver=motif_driver.replace(delta['after'],delta['before'])
    assert hashlib.sha256(motif_driver.encode()).hexdigest()==closed['config']['pins'][str(HERE/'app.mjs')], 'Only the six declared commands-only checks may differ from the retained motif driver'
    frozen=json.loads(subprocess.check_output(['git','show','056a8c48ff667134dee8e5e99710185294e4b898:docs/mcr/evidence/parade-ink-foundation/readset.json'],cwd=REPO))
    foundation={r['path']:'056a8c48ff667134dee8e5e99710185294e4b898' for r in frozen['implementation']}
    foundation['frontend/src/app/shell.css']='b5b44e47531e3b75febc5ee5cdd6a11b1648779c'
    last_retained_live=time.monotonic()-.25
    for source,sha in report['sourceReadset'].items():
        assert time.time()<config['workDeadline'],'Absolute WORK deadline'
        if time.monotonic()-last_retained_live>=.25:
            fast(config);last_retained_live=time.monotonic()
        data=subprocess.check_output(['git','show',foundation.get(source,config['head'])+':'+source],cwd=REPO)
        assert hashlib.sha256(data).hexdigest()==sha, 'Retained source changed: '+source
    for case in report['cases']:
        assert case['editor128RGBA']==report['approvedPack']['expected128RGBA']
        assert [r['status'] for r in case['admissionNegative']]==[401,401,405]
        for key in ['screenshot','reducedScreenshot','previewScreenshot']:
            if key in case:
                proof=path.parent/case[key]['path'];assert digest(proof)==case[key]['sha256']
    receipt={'qualification':'RETAINED_COMPLETED_NATIVE07_CASES_NOT_NATIVE07_OVERALL_PASS','source':str(path.relative_to(REPO)),'sha256':digest(path),'priorCandidate':report['candidate'],'currentCandidate':config['head'],'runnerUnchanged':False,'declaredCommandsOnlyDelta':{'restoresToolsBeforeClose':True,'rejectsVacuousPresentEditorButtonProof':True,'motifDriverBytesUnchanged':True,'syncReadySha256':digest(HERE/'sync-ready.mjs'),'commandsOnlyDeltasSha256':digest(HERE/'commands-driver-deltas.json')},'sourceReadsetRechecked':True,'widths':[360,390,430],'originalRoundStatus':'FAIL','closedFullSha256':digest(path.parent.parent/'FULL.json')}
    (output/'retained-motif.json').write_text(json.dumps(receipt,indent=2)+'\n')

def retained_resource_cases(config,output):
    prior=REPO/'docs/mcr/evidence/parade-ink-wall-b/wall-native27-0301'
    closed=json.loads((prior/'FULL.json').read_text())
    assert closed['FULL'] is True and closed['temporaryAbsent'] is True and closed['remainingOwned']==[] and closed['status']=='FAIL'
    log=(prior/'changed-metadata-regressions.log').read_text()
    assert log.rstrip().endswith('OK') and 'Ran 2 tests in' in log and log.count(' ... ok')==2 and 'FAILED' not in log
    proofs={}
    deltas=json.loads((HERE/'resource-case-deltas.json').read_text())['qualifiedNative27']
    for name in ['resource_receipts.py','control-cases.py']:
        path=HERE/name;sha=digest(path)
        delta=deltas[name];assert sha==delta['currentSha256']
        source=path.read_text()
        for change in delta['exactDeltas']:
            assert change['after'] and source.count(change['after'])==1
            source=source.replace(change['after'],change['before'],1)
        qualified=hashlib.sha256(source.encode()).hexdigest()
        assert qualified==delta['qualifiedSha256']
        assert qualified==closed['config']['pins'][str(path)],'Qualified resource case input changed'
        proofs[name]=sha
    (output/'retained-resource-cases.json').write_text(json.dumps({'qualification':'RETAINED_TWO_COMPLETED_NATIVE27_RESOURCE_CASES_NOT_OVERALL_ROUND_PASS','source':str(prior.relative_to(REPO)),'sourceRoundStatus':'FAIL','fullSha256':digest(prior/'FULL.json'),'logSha256':digest(prior/'changed-metadata-regressions.log'),'currentInputs':proofs,'exactDeltaManifestSha256':digest(HERE/'resource-case-deltas.json'),'necessaryRelativeIdentityCaseRunsSeparately':True,'currentControllerOrderingChanged':True},indent=2)+'\n')

def retained_native33_checks(output):
    prior=REPO/'docs/mcr/evidence/parade-ink-wall-b/wall-native33-0331'
    closed=json.loads((prior/'FULL.json').read_text())
    assert closed['FULL'] is True and closed['temporaryAbsent'] is True and closed['remainingOwned']==[] and closed['status']=='FAIL'
    log=(prior/'changed-identity-regressions.log').read_text()
    assert log.rstrip().endswith('OK') and 'Ran 2 tests in' in log and log.count(' ... ok')==2 and 'FAILED' not in log
    for name,delta in json.loads((HERE/'resource-case-deltas.json').read_text())['qualifiedNative33'].items():
        source=(HERE/name).read_text();assert digest(HERE/name)==delta['currentSha256']
        for change in delta['exactDeltas']:
            assert change['after'] and source.count(change['after'])==1
            source=source.replace(change['after'],change['before'],1)
        assert hashlib.sha256(source.encode()).hexdigest()==delta['qualifiedSha256']==closed['config']['pins'][str(HERE/name)]
    assert closed['config']['pins'][str(HERE/'native-round.py')]=='175f6a92fc1a89982621fef60b6147db74e67eb260b6f611487cb58ceb98d424'
    assert (prior/'commands.log').exists() and digest(HERE/'sync-ready.mjs')==closed['config']['pins'][str(HERE/'sync-ready.mjs')]
    (output/'retained-native33-checks.json').write_text(json.dumps({'qualification':'RETAINED_TWO_IDENTITY_CASES_AND_COMMAND_HELPER_SYNTAX_LINT_NOT_NATIVE33_ROUND_PASS','fullSha256':digest(prior/'FULL.json'),'identityLogSha256':digest(prior/'changed-identity-regressions.log'),'deltaManifestSha256':digest(HERE/'resource-case-deltas.json'),'helperEvidence':'Pinned sequential controller entered commands after both unchanged helper checks returned zero.'},indent=2)+'\n')

def retained_native36_cases(output):
    prior=REPO/'docs/mcr/evidence/parade-ink-wall-b/wall-native36-0343'
    closed=json.loads((prior/'FULL.json').read_text());log=(prior/'changed-terminal-identity-regressions.log').read_text()
    assert closed['FULL'] is True and closed['temporaryAbsent'] is True and closed['remainingOwned']==[] and closed['status']=='FAIL'
    assert log.rstrip().endswith('OK') and 'Ran 2 tests in' in log and log.count(' ... ok')==2 and 'FAILED' not in log
    for name in ['resource_receipts.py','control-cases.py']:assert digest(HERE/name)==closed['config']['pins'][str(HERE/name)]
    (output/'retained-native36-cases.json').write_text(json.dumps({'qualification':'RETAINED_TWO_COMPLETED_NATIVE36_TERMINAL_IDENTITY_CASES_NOT_ROUND_PASS','fullSha256':digest(prior/'FULL.json'),'logSha256':digest(prior/'changed-terminal-identity-regressions.log'),'unchangedInputs':True},indent=2)+'\n')

def retained_native37_helpers(output):
    prior=REPO/'docs/mcr/evidence/parade-ink-wall-b/wall-native37-0354'
    closed=json.loads((prior/'FULL.json').read_text())
    assert closed['FULL'] is True and closed['temporaryAbsent'] is True and closed['remainingOwned']==[] and closed['status']=='FAIL'
    assert closed['config']['pins'][str(HERE/'native-round.py')]=='9d057f179a1d5ddae4995bcbc183dd6b4e05792b7ac761d21904d2a80ed0a64a'
    assert (prior/'commands/report.json').exists()
    pins={name:digest(HERE/name) for name in ['capture-reuse.mjs','sync-recover.mjs']}
    for name,sha in pins.items():assert sha==closed['config']['pins'][str(HERE/name)]
    (output/'retained-native37-helper-checks.json').write_text(json.dumps({'qualification':'RETAINED_CAPTURE_AND_RECOVERY_HELPER_SYNTAX_LINT_NOT_NATIVE37_ROUND_PASS','fullSha256':digest(prior/'FULL.json'),'unchangedPins':pins,'evidence':'Exact sequential controller reached actual App report after all four helper checks returned zero.'},indent=2)+'\n')

def guardian(path):
    config = json.loads(path.read_text()); root = path.parent
    roots = capture(root)
    carry = accounting.Carry(config['baseline'], config['carriedNew'], config['priorWhole'], config['carriedNew'])
    try:
        initial = full_sample(config, roots, carry, config['controller'])
        (root / 'guardian.ready').write_text(json.dumps(initial))
        with (root / 'guardian.jsonl').open('a') as log:
            while not (root / 'stop').exists():
                sample = full_sample(config, roots, carry, config['controller'])
                log.write(json.dumps(sample) + '\n'); log.flush()
                time.sleep(.5)
    except BaseException as error:
        (root / 'guardian.failure').write_text(json.dumps({'error':repr(error),'accounting':error.receipt() if isinstance(error,accounting.AccountingError) else None}))
        sys.exit(2)

p = argparse.ArgumentParser()
p.add_argument('--guardian', type=Path)
p.add_argument('--preflight', action='store_true')
p.add_argument('--retained-motif', type=Path)
p.add_argument('--retain-resource-cases',action='store_true')
p.add_argument('--job', default='PI09-WALLB-APP-NATIVE01-20261105T0016Z')
p.add_argument('--other-gib', type=int, default=4)
p.add_argument('--new-mib', type=int, choices=[200,256,320,384,416,448,480,512,544,576,592], default=200)
a = p.parse_args()
if a.guardian:
    guardian(a.guardian)
    sys.exit(0)
if a.preflight:
    observation=resource_receipts.ordinary_preflight(HERE/'native-round.py')
    exit_code=1
    try:
        roots = capture(REPO / 'docs/mcr/evidence/parade-ink-wall-b')
        result = accounting.collect(roots, deadline=time.monotonic() + 20, checkpoint=lambda: None)
        pins = {str(path): digest(path) for path in [NODE, HERE/'native-round.py', HERE/'accounting.py', HERE/'app.mjs', HERE/'dependencies.mjs', HERE/'reference.py',HERE/'sparse-source.mjs',HERE/'resource_receipts.py',HERE/'admission-fixture/transform.mjs', HERE/'control-cases.py', HERE/'sync-ready.mjs', HERE/'capture-reuse.mjs', HERE/'commands-driver-deltas.json', Path(sys.executable).resolve(), Path('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'), TOOLING/'package.json', TOOLING/'pnpm-lock.yaml']}
        print(json.dumps({'allocated': result, 'pins': pins, 'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip()}))
        exit_code=0
    finally:
        resource_receipts.ordinary_preflight(HERE/'native-round.py',exit_code,observation)
    sys.exit(exit_code)

# Invocation is permitted only after the actual predecessor FULL/DECLINE and acceptance.
assert Path(sys.argv[0]).is_absolute(), 'Invoke the exact absolute controller path to match its published identity'
assert os.environ.get('PI09_NATIVE_ACCEPTED') == a.job
assert subprocess.check_output(['git', 'status', '--porcelain'], cwd=REPO, text=True) == ''
historical=[json.loads(proof.read_text()).get('peaks',{}).get('new',0) for proof in (REPO/'docs/mcr/evidence/parade-ink-wall-b').glob('*/FULL.json')]
for phase in (REPO/'docs/mcr/evidence/parade-ink-wall-b').iterdir():
    if not phase.is_dir():continue
    for name in ['controller.jsonl','guardian.jsonl','controller.ready','guardian.ready']:
        proof=phase/name
        if not proof.exists():continue
        assert proof.stat().st_size<=16*MiB,'Historical sample file bound'
        for line in proof.read_text().splitlines():
            measured=json.loads(line)
            if 'new' in measured:historical.append(measured['new'])
# Old carried256 already includes the entire DOC16 reservation. One additional
# MiB bounds this small control/proof/Git freeze; minimum360MiB also
# charges the entire FINALDOC4 and measured between-round retained-root growth
# above every initial/live sample MAX, including phases without workload peaks.
# Native17+ must retain Native16 growth and the complete final documentation
# reservation. The prospective448 option requires its actual external offer.
carry_floor=434*MiB if a.new_mib>=480 else 400*MiB if a.new_mib==448 else 360*MiB
carried_new=max(carry_floor,((max(historical,default=0)+MiB-1)//MiB+1)*MiB)
assert carried_new<a.new_mib*MiB, 'No fresh room under the shared nonresetting NEW cap'
assert shutil.disk_usage(REPO).free >= 12 * GiB
assert shutil.disk_usage(REPO).free - a.other_gib * GiB - max(0, a.new_mib * MiB - carried_new) >= 9 * GiB
for name in ['package.json','pnpm-lock.yaml']:
    assert (REPO/'frontend'/name).read_bytes() == (TOOLING/name).read_bytes()
assert digest(NODE) == 'b6fb419f0f43bb2fbca273700294dd2f3b685a64b60e8417c295f47646a70e4a'
started = time.time()
temp = Path('/private/tmp/mcr-pi09-native-' + str(int(started)))
temp.mkdir(mode=0o700)
roots = capture(temp)
config = {'job': a.job, 'controller': os.getpid(), 'accepted': started, 'workDeadline': started+300,
          'fullDeadline': started+360, 'otherNew': a.other_gib*GiB, 'baseline': {}, 'censusRoot':str(temp), 'initialIdentityReconciliation':True,
          'carriedNew': carried_new, 'allHistoricalSampleNewMaximum':max(historical,default=0), 'newCap': a.new_mib*MiB, 'kind': 'browser', 'processCap': 24, 'rssCap': 4*GiB, 'priorWhole': 5*GiB, 'pins': {str(p):digest(p) for p in [NODE,HERE/'native-round.py',HERE/'accounting.py',HERE/'app.mjs',HERE/'dependencies.mjs',HERE/'reference.py',HERE/'sparse-source.mjs',HERE/'resource_receipts.py',HERE/'admission-fixture/transform.mjs',HERE/'control-cases.py',HERE/'sync-ready.mjs',HERE/'capture-reuse.mjs',HERE/'commands-driver-deltas.json',Path(sys.executable).resolve(),Path('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'),TOOLING/'package.json',TOOLING/'pnpm-lock.yaml']}, 'head': subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPO,text=True).strip(),
          'carryPolicy': 'Conservative5GiB WHOLE; NEW rounds up every retained observed maximum plus1MiB control freeze. Floors434MiB for480/400MiB for448/360MiB otherwise retain every old phase, entire ordinary DOC uppers including FINALDOC1 and retained-root growth. Selected upper is cumulative and never resets; no deletion credits.'}
path = temp/'config.json'
config['pins'][str(HERE/'resource-case-deltas.json')]=digest(HERE/'resource-case-deltas.json')
config['pins'][str(HERE/'sync-recover.mjs')]=digest(HERE/'sync-recover.mjs')
durable = REPO/'docs/mcr/evidence/parade-ink-wall-b'/a.job.lower()
durable.mkdir(parents=True,exist_ok=True)
(durable/'ACCEPT.json').write_text(json.dumps(config,indent=2)+'\n')
resource_receipts.publish(config,'ACCEPTED',HERE/'native-round.py',durable)
child = watcher = None
status = 'FAIL'; failure = None; failure_detail = None; peaks = {}
try:
    last_baseline_live = time.monotonic() - .25
    def baseline_checkpoint():
        global last_baseline_live
        assert time.time() < config['workDeadline'], 'Absolute WORK deadline'
        if time.monotonic() - last_baseline_live >= .25:
            fast(config); last_baseline_live = time.monotonic()
    baseline = accounting.collect(roots, deadline=time.monotonic()+15, checkpoint=baseline_checkpoint)
    assert baseline['bytes'] + a.new_mib*MiB < 8*GiB
    config['baseline'] = {k:v['bytes'] for k,v in baseline['scopes'].items()}
    path.write_text(json.dumps(config,indent=2))
    carry = accounting.Carry(config['baseline'],config['carriedNew'],config['priorWhole'],config['carriedNew'])
    watcher = subprocess.Popen([sys.executable,str(HERE/'native-round.py'),'--guardian',str(path)])
    resource_receipts.publish(config,'ACCEPTED',HERE/'native-round.py',durable,watcher.pid)
    ready_deadline = min(started+20,config['workDeadline'])
    while not (temp/'guardian.ready').exists():
        assert watcher.poll() is None and time.time()<ready_deadline, 'Independent guardian not ready'
        fast(config, watcher.pid); time.sleep(.1)
    initial = full_sample(config, roots, carry, watcher.pid)
    (temp/'controller.ready').write_text(json.dumps(initial))
    output=REPO/'docs/mcr/evidence/parade-ink-wall-b'/a.job.lower()
    output.mkdir(parents=True,exist_ok=True)
    if a.retain_resource_cases:
        retained_resource_cases(config,output);retained_native33_checks(output);retained_native36_cases(output);retained_native37_helpers(output)
    if a.retained_motif:retained_motif(a.retained_motif,config,output)
    checks=[('capture-helper-syntax',[str(NODE),'--check',str(HERE/'capture-reuse.mjs')]),('capture-helper-lint',[str(TOOLING/'node_modules/.bin/oxlint'),str(HERE/'capture-reuse.mjs')])]
    if a.retain_resource_cases:
        previous=REPO/'docs/mcr/evidence/parade-ink-wall-b/wall-native28-0301'
        proof=json.loads((previous/'FULL.json').read_text())
        assert proof['FULL'] is True and proof['remainingOwned']==[] and proof['temporaryAbsent'] is True
        assert proof['config']['pins'][str(HERE/'native-round.py')]=='5ad2ac62aeb626b2a1bafa7be687e8daa68a9e9e698cd846ccd80f63adee7bed'
        assert (previous/'commands').is_dir() and (previous/'commands.log').exists()
        if digest(HERE/'capture-reuse.mjs')==proof['config']['pins'][str(HERE/'capture-reuse.mjs')]:
            (output/'retained-capture-checks.json').write_text(json.dumps({'qualification':'RETAINED_NATIVE28_HELPER_SYNTAX_AND_LINT_ONLY_NOT_ROUND_PASS','fullSha256':digest(previous/'FULL.json'),'controllerFlowPin':proof['config']['pins'][str(HERE/'native-round.py')],'evidence':'Pinned sequential controller reached commands after both helper return codes were zero; helper unchanged.'},indent=2)+'\n')
            checks=[]
    if not a.retain_resource_cases:checks.insert(0,('changed-metadata-regressions',[sys.executable,'-B',str(HERE/'control-cases.py'),str(temp),'Cases.test_ended_ordinary_identity_query_restarts_complete_census','Cases.test_exact_old_and_new_infrastructure_pairs_require_birth']))
    if not a.retain_resource_cases:
        checks.insert(0,('changed-identity-regressions',[sys.executable,'-B',str(HERE/'control-cases.py'),str(temp),'Cases.test_relative_primary_requires_actual_cwd_executable_and_exact_program','Cases.test_absent_ordinary_guard_history_does_not_hide_live_guard']))
        checks.extend([('commands-helper-syntax',[str(NODE),'--check',str(HERE/'sync-ready.mjs')]),('commands-helper-lint',[str(TOOLING/'node_modules/.bin/oxlint'),str(HERE/'sync-ready.mjs')])])
    if not a.retain_resource_cases:checks.insert(0,('changed-terminal-identity-regressions',[sys.executable,'-B',str(HERE/'control-cases.py'),str(temp),'Cases.test_ordinary_image_departure_restarts_complete_census_but_live_mismatch_fails','Cases.test_nested_yield_preserves_active_and_legacy_claims']))
    checks.extend([('sync-recover-syntax',[str(NODE),'--check',str(HERE/'sync-recover.mjs')]),('sync-recover-lint',[str(TOOLING/'node_modules/.bin/oxlint'),str(HERE/'sync-recover.mjs')])])
    if a.retain_resource_cases:checks=[]
    (temp/'payload.started').write_text(str(time.time()))
    for label,command in checks:
        with (temp/(label+'.log')).open('w') as log:
            child=subprocess.Popen(command,cwd=REPO,env={**os.environ,'PATH':str(NODE.parent)+os.pathsep+os.environ['PATH']},stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            while child.poll() is None:
                full_sample(config,roots,carry,watcher.pid);time.sleep(.25)
        assert child.returncode==0,label+' failed; preserve evidence'
    for mode in (['commands'] if a.retained_motif else ['motif','commands']):
        phase=temp/mode; phase.mkdir()
        durable=output/mode; durable.mkdir()
        env={**os.environ,'PATH':str(NODE.parent)+os.pathsep+os.environ['PATH'],'TMPDIR':str(phase),
             'PI09_TOOLING_ROOT':str(TOOLING),'PI09_NATIVE_OUTPUT':str(durable),'PI09_NATIVE_TEMP':str(phase),
             'PI09_NATIVE_JOB':a.job,'PI09_NATIVE_CANDIDATE':config['head'],'PI09_NATIVE_PORT':'5366',
             'PI09_NATIVE_MODE':mode,'PI09_REUSE_COMMAND_CAPTURES':str(REPO/'docs/mcr/evidence/parade-ink-wall-b/pi09-wallb-remainder-native14-0145/commands/report.json'),'PI09_NATIVE_CACHE':str(temp/'vite'),'NO_COLOR':'1','CI':'1'}
        with (temp/(mode+'.log')).open('w') as log:
            child=subprocess.Popen([str(NODE),'--import',str(HERE/'dependencies.mjs'),str(HERE/'app.mjs')],cwd=REPO,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            with (temp/'controller.jsonl').open('a') as samples:
                while child.poll() is None:
                    assert not (temp/'guardian.failure').exists(), 'Independent guardian failed'
                    sample=full_sample(config,roots,carry,watcher.pid)
                    for key in ['new','whole','pids','rss','globalPids','globalRss']:peaks[key]=max(peaks.get(key,0),sample[key])
                    samples.write(json.dumps(sample)+'\n');samples.flush();time.sleep(.25)
        assert child.returncode==0, mode+' actual-App journey failed; preserve evidence'
    # Analytical crops compare the exact reference against existing captures.
    with (temp/'comparison.log').open('w') as log:
        child=subprocess.Popen([sys.executable,str(HERE/'reference.py'),'--output',str(output/'comparison'),'--captures',str(a.retained_motif or output/'motif/report.json')],cwd=REPO,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        while child.poll() is None:
            full_sample(config,roots,carry,watcher.pid);time.sleep(.25)
    assert child.returncode==0,'Reference normalization/comparison helper failed'
    status='PASS'
except BaseException as error:
    failure=repr(error)
    failure_detail={'accounting':error.receipt() if isinstance(error,accounting.AccountingError) else None,'cause':repr(error.__cause__) if error.__cause__ else None}
finally:
    if child and child.poll() is None:
        os.killpg(child.pid,signal.SIGTERM)
        try: child.wait(timeout=5)
        except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait(timeout=5)
    (temp/'stop').write_text('owned work stopped')
    if watcher:
        try:watcher.wait(timeout=10)
        except subprocess.TimeoutExpired:watcher.terminate();watcher.wait(timeout=5)
    remaining=process_sample(os.getpid())['ownedPids']
    # ps itself is ephemeral; no owned Node/guardian may remain before FULL.
    data=rows(); live=[row for row in data if row[0] in descendants(data,os.getpid()) and row[0]!=os.getpid() and row[3] not in ['ps -ww -axo pid=,ppid=,rss=,lstart=,command=','/bin/ps -ww -axo pid=,ppid=,rss=,lstart=,command=']]
    full=not live and (not child or child.poll() is not None) and (not watcher or watcher.poll() is not None) and time.time()<config['fullDeadline']
    result={'job':a.job,'status':status,'error':failure,'failureDetail':failure_detail,'FULL':full,'finished':time.time(),'elapsed':time.time()-started,'peaks':peaks,'config':config,'phaseTemporary':str(temp),'remainingOwned':live}
    durable=REPO/'docs/mcr/evidence/parade-ink-wall-b'/a.job.lower()
    durable.mkdir(parents=True,exist_ok=True)
    for proof in temp.iterdir():
        if proof.is_file():shutil.copy2(proof,durable/proof.name)
    shutil.rmtree(temp)
    result['temporaryAbsent']=not temp.exists()
    result['FULL']=full and result['temporaryAbsent']
    (durable/'FULL.json').write_text(json.dumps(result,indent=2)+'\n')
    if result['FULL']:resource_receipts.publish(config,'FULL',HERE/'native-round.py',durable,result=result)
    sparse=Path(subprocess.check_output(['git','rev-parse','--git-dir'],cwd=REPO,text=True).strip())/'info/sparse-checkout'
    with sparse.open('a') as patterns:
        for proof in sorted(durable.rglob('*')):
            if proof.is_file():patterns.write('/'+proof.relative_to(REPO).as_posix()+'\n')
    print(json.dumps(result),flush=True)
    if status!='PASS' or not full:sys.exit(1)
