"""Pinned primary controller; independent guardian precedes every work child."""
from pathlib import Path
import argparse
import datetime as dt
import hashlib
import json
import os
import resource
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from guard import (ROOT,OUT,PACK,TESTS,DOC,ROOTS,PYTHON,MIB,GIB,WHOLE_CAP,NEW_CAP,
                   DURABLE_CAP,TEMP_CAP,RSS_CAP,PROCESS_CAP,PRIOR_WHOLE_FLOOR,
                   inventory,pin,children,resident,Accounting)


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--offer',required=True);parser.add_argument('--head',required=True)
    parser.add_argument('--expires',required=True);parser.add_argument('--other-growth-bytes',required=True,type=int)
    parser.add_argument('--other-workflows',required=True,type=int);parser.add_argument('--other-source',required=True,type=int)
    parser.add_argument('--other-browser',required=True,type=int)
    parser.add_argument('--other-process-caps',required=True,type=int)
    parser.add_argument('--other-rss-cap-bytes',required=True,type=int)
    args=parser.parse_args()
    assert args.offer==os.path.basename(args.offer) and args.offer.isascii()
    assert sys.executable==PYTHON and sys.version_info[:3]==(3,14,3)
    from PIL import __version__ as pillow
    assert pillow=='12.2.0'
    def git(*argv):return subprocess.check_output(['git',*argv],cwd=ROOT).decode().strip()
    assert git('rev-parse','HEAD')==args.head and not git('status','--porcelain'),'freeze clean source first'
    assert not git('diff','--name-only','4dfa7f76fc3d8943dea2f2b93655477392e56bd0','HEAD','--',
                   'frontend/public/world/map-roof-street-derived-v2','frontend/public/world/map-reference-composition-v1',
                   'frontend/public/world/reference-scene-v2'),'immutable readset mutation'
    assert 0<=args.other_workflows<4 and 0<=args.other_source<3 and 0<=args.other_browser<=2 and args.other_growth_bytes>=0
    assert 0<=args.other_process_caps<=96-PROCESS_CAP and 0<=args.other_rss_cap_bytes<=18*GIB-RSS_CAP
    receipt=OUT/(args.offer+'.jsonl');assert not receipt.exists(),'closed offer cannot be reused'
    contract=json.loads((PACK/'paint-contract.json').read_text())
    proof=json.loads((ROOT/'frontend/public/world/map-roof-street-derived-v2/source-provenance.json').read_text())
    immutable=contract['baselinePins']+[contract['helperPin']]+proof['sourceInputs']+proof['license']['inputPins']
    for expected in immutable:
        actual=pin(ROOT/expected['path']);assert actual['sha256']==expected['sha256'].removeprefix('sha256:')
        assert 'bytes' not in expected or actual['bytes']==expected['bytes']
    # Generated outputs are independently requalified; retain authored code/docs
    # and old raw receipts, without treating replaceable previews as source.
    source=[pin(p) for folder in (PACK,TESTS,OUT) for p in sorted(folder.rglob('*'))
            if p.is_file() and (p.suffix in ('.py','.md','.jsonl') or p.name in ('paint-contract.json','preflight-01.json'))]+[pin(DOC)]
    baseline=inventory(ROOTS);carry=max(sum(baseline.values()),PRIOR_WHOLE_FLOOR)
    assert carry+NEW_CAP<=WHOLE_CAP,'carry plus growth exceeds whole cap'
    now=dt.datetime.now(dt.timezone.utc);assert now<dt.datetime.fromisoformat(args.expires),'unaccepted offer expired'
    free=shutil.disk_usage(ROOT).free
    assert free>=11*GIB and free-args.other_growth_bytes-NEW_CAP>=9*GIB,'fresh aggregate admission floor'
    start=time.monotonic();temp=Path(tempfile.mkdtemp(prefix='mcr-map-lawns-'))
    request={'offer':args.offer,'head':args.head,'supervisor':os.getpid(),'temp':str(temp),'baselineInodes':baseline,
             'carry':carry,'sourcePins':source,'otherGrowthBytes':args.other_growth_bytes,'workEnd':start+120,'hardEnd':start+300,
             'otherWorkflows':args.other_workflows,'otherSource':args.other_source,'otherBrowser':args.other_browser,
             'otherProcessCaps':args.other_process_caps,'otherRssCaps':args.other_rss_cap_bytes}
    request_path=temp/'request.json';request_path.write_text(json.dumps(request))
    request_sha=hashlib.sha256(request_path.read_bytes()).hexdigest()
    account=Accounting(request);worker=None;guardian=None;workers=[];status='FAIL';guardian_result=None
    def emit(event,**fields):
        raw=json.dumps({'event':event,'at':dt.datetime.now(dt.timezone.utc).isoformat(),**fields})+'\n'
        with receipt.open('a') as file:file.write(raw)
        print(raw,end='',flush=True)
    def sample(work=False):
        assert guardian is not None and guardian.poll() is None,'independent guardian exited'
        if work:assert time.monotonic()<request['workEnd'],'original work deadline'
        account.sample()
    def stop(child):
        if child is not None and child.poll() is None:os.killpg(child.pid,signal.SIGKILL);child.wait(timeout=2)
    try:
        emit('ACCEPT',offer=args.offer,head=args.head,supervisor=os.getpid(),executable=sys.executable,python=sys.version,pillow=pillow,
             sourcePins=source,immutablePins=immutable,roots=[str(p) for p in ROOTS],baselineBytes=sum(baseline.values()),carry=carry,
             priorWholeFloor=PRIOR_WHOLE_FLOOR,privateGitIncludedInSharedGit=True,perInodePositiveGrowth=True,
             wholeCap=WHOLE_CAP,newCap=NEW_CAP,durableCap=DURABLE_CAP,tempCap=TEMP_CAP,rssCap=RSS_CAP,processCap=PROCESS_CAP,
             otherGrowthBytes=args.other_growth_bytes,otherWorkflows=args.other_workflows,otherSource=args.other_source,
             otherBrowser=args.other_browser,otherProcessCaps=args.other_process_caps,otherRssCaps=args.other_rss_cap_bytes,
             summedProcessCaps=args.other_process_caps+PROCESS_CAP,summedRssCaps=args.other_rss_cap_bytes+RSS_CAP,
             initialFree=free,afterAllGrowth=free-args.other_growth_bytes-NEW_CAP,temp=str(temp),requestSha256=request_sha,
             workDeadline=(now+dt.timedelta(seconds=120)).isoformat(),hardDeadline=(now+dt.timedelta(seconds=300)).isoformat(),
             cleanupSeconds=60,resourcePolicy='CORRECTED_SOURCE2120_SOURCE3_GLOBAL4_PID96_RSS18GIB',ports=[],cacheRoots=[])
        guardian=subprocess.Popen([PYTHON,'-B',str(OUT/'guard.py'),str(request_path),request_sha],cwd=ROOT,start_new_session=True,
                                  env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1'},stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,text=True)
        while not (temp/'guardian-ready').exists():sample(True);time.sleep(.06)
        assert guardian.pid in children(os.getpid()),'live guardian missing from primary PID inventory'
        emit('GUARDIAN_READY',pid=guardian.pid,controlPin=pin(OUT/'guard.py'))
        go=False
        while time.monotonic()<request['workEnd']:
            sample(True)
            if select.select([sys.stdin],[],[],.06)[0]:go=sys.stdin.readline().strip()=='GO';break
        assert go,'explicit dispatch missing'
        commands=[('new-build',PACK/'build.py'),('new-boundary-probes',TESTS/'qualifier_test.py'),
                  ('independent-qualifier',TESTS/'qualify.py'),('new-source-previews',OUT/'previews.py')]
        for name,path in commands:
            sample(True);began=time.monotonic()
            worker=subprocess.Popen([PYTHON,'-B',str(path)],cwd=ROOT,start_new_session=True,
                 env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1','MCR_ART_SOURCE_COMMIT':args.head,'TMPDIR':str(temp)},
                 stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
            workers.append(worker.pid)
            account.worker=worker
            assert worker.pid in children(os.getpid()),'new live worker missing from PID inventory'
            while worker.poll() is None:
                sample(True);assert time.monotonic()-began<=60,'per-child 60s cap';time.sleep(.06)
            output=worker.communicate()[0]
            retained=resident(os.getpid())+resident(guardian.pid)+resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
            account.peak['rss']=max(account.peak['rss'],retained);assert retained<=RSS_CAP,'retained worker peak RSS cap'
            emit('CHILD',name=name,pid=worker.pid,exit=worker.returncode,seconds=round(time.monotonic()-began,3),output=output)
            assert worker.returncode==0,name+' failed; finite round ends without silent retry'
            sample(True)
        for expected in source:assert pin(ROOT/expected['path'])==expected,'frozen implementation changed'
        for expected in immutable:assert pin(ROOT/expected['path'])['sha256']==expected['sha256'].removeprefix('sha256:')
        outputs=[pin(p) for p in sorted(PACK.iterdir()) if p.is_file() and p.name!='interface.json']
        outputs+=[pin(OUT/'source-qualification.json'),pin(OUT/'preview-pins.json')]
        emit('READY_FOR_READONLY_INSPECTION',outputs=outputs,images=json.loads((OUT/'preview-pins.json').read_text())['images'])
        released=False
        while time.monotonic()<request['hardEnd']-60:
            sample()
            if select.select([sys.stdin],[],[],.06)[0]:
                value=sys.stdin.readline();assert value,'EOF is not owner inspection release'
                if value.strip()=='RELEASE':released=True;break
        assert released,'owner did not release image inspection'
        assert account.peak['processes']==4,'independent supervisor/guardian/worker/exec accounting was not observed'
        (temp/'stop').write_text('STOP');guardian.wait(timeout=2)
        guardian_result=json.loads((temp/'guardian-result.json').read_text())
        assert guardian.returncode==0 and guardian_result['status']=='PASS','independent guardian failure'
        status='PASS'
    except BaseException as error:emit('ERROR',reason=repr(error))
    finally:
        stop(worker)
        if guardian is not None and guardian.poll() is None:
            (temp/'stop').write_text('STOP')
            try:guardian.wait(timeout=2)
            except subprocess.TimeoutExpired:stop(guardian)
        if (temp/'guardian-result.json').exists():guardian_result=json.loads((temp/'guardian-result.json').read_text())
        if guardian is not None and guardian.returncode!=0:status='FAIL'
        if guardian is not None:
            stderr=guardian.communicate()[1]
            if stderr:emit('GUARDIAN_STDERR',text=stderr)
        account.close()
        shutil.rmtree(temp)
        emit('FULL',status=status,tempAbsent=not temp.exists(),allChildrenExited=not children(os.getpid()),workerPids=workers,
             supervisor=os.getpid(),guardianPid=guardian.pid if guardian else None,guardianResult=guardian_result,peak=account.peak,
             minimumFree=account.minimum_free,finalFree=shutil.disk_usage(ROOT).free,samples=account.samples,
             maxGuardIntervalSeconds=account.max_interval,maxByteScanSeconds=account.max_scan_seconds,ports=[],cacheRoots=[],remainingNewBytes=0,workflows=0,
             elapsedSeconds=round(time.monotonic()-start,3))
    if status=='PASS':
        interface=json.loads((PACK/'interface.json').read_text())
        interface['qualification']={'sourceCommit':args.head,'receipt':pin(receipt),'status':'PASS'}
        (PACK/'interface.json').write_text(json.dumps(interface,indent=2)+'\n')
    return 0 if status=='PASS' else 1


if __name__=='__main__':sys.exit(main())
