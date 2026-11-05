"""Bounded ordinary source/docs/Git observation; no functional qualification."""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import select
import shutil
import subprocess
import sys
import time
from inventory import Inventory

ROOT=Path(__file__).resolve().parents[5]
EVIDENCE=Path(__file__).resolve().parent
RECORD=Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts/01a1a231-0dc4-7b50-92bd-c0f0f56bd970.json')
OFFER='PARADISECITY-ART-FIXDOC32-0142'
CAP=32*1024**2
DEADLINE=dt.datetime.fromisoformat('2026-11-05T02:10:00+00:00').timestamp()
accepted=time.time()
prior=json.loads(RECORD.read_text())
birth=' '.join(subprocess.check_output(['ps','-p',str(os.getpid()),'-o','lstart='],text=True).split())
receipt=EVIDENCE/'fixdoc32-booking.json'
actual_executable=subprocess.check_output(['ps','-p',str(os.getpid()),'-o','comm='],text=True).strip()
old_path=Path(prior['receiptPaths']['full'])
old_epoch=dt.datetime.fromisoformat(prior['actual']['fullAt']).timestamp()
record=dict(ownerThreadId=prior['ownerThreadId'],offerId=OFFER,state='ACCEPTED',acceptedAtEpoch=accepted,workDeadlineEpoch=DEADLINE,fullDeadlineEpoch=DEADLINE,kind='ordinary',processCap=None,rssCapBytes=None,remainingNewUpperBytes=CAP,controller=dict(pid=os.getpid(),birth=birth,executable=actual_executable,launcherExecutable=sys.executable,script=str(Path(__file__).resolve()),cwd=str(ROOT),outputRoot=str(EVIDENCE)),guardControllers=[],receiptPaths=dict(accept=str(receipt),full=str(receipt)),closedOffers=prior.get('closedOffers',[])+[dict(ownerThreadId=prior['ownerThreadId'],offerId=prior['offerId'],acceptedAtEpoch=prior['acceptedAtEpoch'],actualFullEpoch=old_epoch,fullPath=str(old_path),fullSha256=hashlib.sha256(old_path.read_bytes()).hexdigest(),resourcesReleased=True)])
def publish():
 p=RECORD.with_suffix('.pending');p.write_text(json.dumps(record,indent=2)+'\n');p.replace(RECORD)
publish()
inventory=Inventory([str(ROOT),'/Users/lhs/Desktop/mcr/.git','/Library/Frameworks/Python.framework/Versions/3.14','/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64',str(RECORD)],4141284523)
peak=0;whole=inventory.carry;minimum=shutil.disk_usage(ROOT).free
print(json.dumps(dict(event='ACCEPT',offer=OFFER,pid=os.getpid(),birth=birth,carry=whole,cap=CAP)),flush=True)
status='FULL'
try:
 while True:
  w,n=inventory.sample();peak=max(peak,n-34*1024**2);whole=max(whole,w);minimum=min(minimum,shutil.disk_usage(ROOT).free)
  assert peak<=CAP and time.time()<DEADLINE and minimum>=11*1024**3
  if select.select([sys.stdin],[],[],.3)[0]:
   line=sys.stdin.readline();assert line,'No implicit release on EOF'
   if line.strip()=='FULL':break
   command=json.loads(line)
   if 'python' in command:exec(command['python'],globals())
   elif 'git' in command:
    result=subprocess.run(['git',*command['git']],cwd=ROOT,text=True,capture_output=True);print(json.dumps(dict(git=command['git'],exit=result.returncode,stdout=result.stdout,stderr=result.stderr)),flush=True);assert result.returncode==0
   else:raise AssertionError('Unknown ordinary command')
except BaseException as error:
 status='FAIL';print(repr(error),flush=True)
finally:
 w,n=inventory.sample();peak=max(peak,n-34*1024**2);whole=max(whole,w)
 result=dict(offer=OFFER,status=status,acceptedAtEpoch=accepted,fullAt=dt.datetime.now(dt.timezone.utc).isoformat(),positivePeak=peak,wholePeak=whole,minFree=minimum,cap=CAP)
 receipt.write_text(json.dumps(result,indent=2)+'\n');record.update(state='FULL',remainingNewUpperBytes=0,actual=result);publish();print(json.dumps(dict(event='FULL',**result)),flush=True)
