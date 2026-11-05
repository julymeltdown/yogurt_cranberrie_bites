"""Actual bounded ordinary authoring observation; never runs product checks."""
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
sys.path.insert(0,str(Path(__file__).resolve().parent.parent))
from inventory import Inventory

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[5]
OWNER='01a1a231-0dc4-7b50-92bd-c0f0f56bd970'
RECORD=Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts')/(OWNER+'.json')
OFFER='ART-PC-STRICT-SEAL4-0427'
CAP=4*1024**2
DEADLINE=dt.datetime.fromisoformat('2026-11-05T05:00:00+00:00').timestamp()
accepted=time.time();prior=json.loads(RECORD.read_text());assert prior['state']=='FULL'
birth=' '.join(subprocess.check_output(['ps','-p',str(os.getpid()),'-o','lstart='],text=True).split())
executable=subprocess.check_output(['ps','-p',str(os.getpid()),'-o','comm='],text=True).strip()
old=Path(prior['receiptPaths']['full'])
history=prior.get('closedOffers',[])+[dict(ownerThreadId=OWNER,offerId=prior['offerId'],acceptedAtEpoch=prior['acceptedAtEpoch'],actualFullEpoch=dt.datetime.fromisoformat(prior['actual']['at']).timestamp(),fullPath=str(old),fullSha256=hashlib.sha256(old.read_bytes()).hexdigest(),resourcesReleased=True)]
receipt=HERE/'seal.json'
record=dict(ownerThreadId=OWNER,offerId=OFFER,state='ACCEPTED',phase='ORDINARY_AUTHORING',acceptedAtEpoch=accepted,workDeadlineEpoch=DEADLINE,fullDeadlineEpoch=DEADLINE,kind='ordinary',processCap=None,rssCapBytes=None,remainingNewUpperBytes=CAP,controller=dict(pid=os.getpid(),birth=birth,executable=executable,launcherExecutable=sys.executable,script=str(Path(__file__).resolve()),cwd=str(ROOT),outputRoot=str(HERE),role='ordinary strict-repair source control, metadata and exact-path Git sealing only',commandEncoding='argv',argv=[sys.executable,'-B','-u',str(Path(__file__).resolve())]),guardControllers=[],receiptPaths=dict(accept=str(receipt),full=str(receipt)),closedOffers=history)
def publish():
 pending=RECORD.with_suffix('.pending');pending.write_text(json.dumps(record,indent=2)+'\n');pending.replace(RECORD)
receipt.write_text(json.dumps(dict(event='ACCEPT',offer=OFFER,acceptedAtEpoch=accepted,controller=record['controller']),indent=2)+'\n');publish()
roots=[str(ROOT),'/Users/lhs/Desktop/mcr/.git','/Library/Frameworks/Python.framework/Versions/3.14','/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64','/Users/lhs/Desktop/mcr/frontend/node_modules',str(RECORD)]
inventory=Inventory(roots,4348166684)
peak=0;whole=inventory.carry;minimum=shutil.disk_usage(ROOT).free;status='FULL'
print(json.dumps(dict(event='ACCEPT',offer=OFFER,pid=os.getpid(),birth=birth,executable=executable,whole=whole,logical=inventory.logical,cap=CAP)),flush=True)
try:
 while True:
  w,n=inventory.sample();peak=max(peak,n-34*1024**2);whole=max(whole,w);minimum=min(minimum,shutil.disk_usage(ROOT).free)
  assert peak<=CAP and time.time()<DEADLINE and minimum>=11*1024**3
  if select.select([sys.stdin],[],[],.3)[0]:
   line=sys.stdin.readline();assert line,'EOF is not release'
   if line.strip()=='FULL':break
except BaseException as error:status='FAIL';print(repr(error),flush=True)
finally:
 w,n=inventory.sample();peak=max(peak,n-34*1024**2);whole=max(whole,w)
 result=dict(event='FULL',offer=OFFER,status=status,acceptedAtEpoch=accepted,at=dt.datetime.now(dt.timezone.utc).isoformat(),cap=CAP,positivePeak=peak,wholePeak=whole,minFree=minimum,allChildrenExited=True,roots=roots,historicalNewPeak=221435639)
 receipt.write_text(json.dumps(result,indent=2)+'\n');record.update(state='FULL',remainingNewUpperBytes=0,actual=result);publish();print(json.dumps(result),flush=True)
