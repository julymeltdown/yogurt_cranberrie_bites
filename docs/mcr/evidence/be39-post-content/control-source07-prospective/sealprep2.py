"""Bounded incident and prospective-control seal; no census or payload."""
import ast, hashlib, json, os, shutil, subprocess, time
from pathlib import Path
O=Path(__file__).parent
OWNER='01a1a21a-a088-7c31-83e3-19de119ba873'
INDEX=Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts')/(OWNER+'.json')
def put(p,v):
 t=p.with_suffix(p.suffix+'.tmp');t.write_text(json.dumps(v,indent=2)+'\n');os.replace(t,p)
old=json.loads(INDEX.read_text());assert old['state']=='FULL'
row=subprocess.check_output(['/bin/ps','-p',str(os.getpid()),'-o','ppid=,pgid=,lstart=,comm='],text=True).split(maxsplit=7)
now=time.time();offer='POST-INCIDENT-SEALPREP2-0350'
ident={'pid':os.getpid(),'ppid':int(row[0]),'pgid':int(row[1]),'birth':' '.join(row[2:7]),'executable':row[7].strip(),'script':str(Path(__file__).absolute()),'cwd':os.getcwd(),'outputRoot':str(O)}
rec={'ownerThreadId':OWNER,'offerId':offer,'state':'ACCEPTED','kind':'ordinary','acceptedAtEpoch':now,'workDeadlineEpoch':1793852100,'fullDeadlineEpoch':None,'processCap':None,'rssCapBytes':None,'remainingNewUpperBytes':2097152,'controller':ident,'guardControllers':[],'closedHistory':old['closedHistory'],'admissionDisposition':old['admissionDisposition'],'receiptPaths':{'accept':str(O/(offer+'-accept.json')),'full':str(O/(offer+'-FULL.json'))},'authorization':'Explicit2MiB ENTIRE once inside amended same640MiB family; incident docs/Git/prospective controls/readset only; no census/test/guardprobe/runtime'}
put(Path(rec['receiptPaths']['accept']),rec);put(INDEX,rec)
failure=None
try:
 assert now<1793852100
 book=json.loads((O/'source-budget-amendment.json').read_text())
 book.update(state='EXPLICIT_SAME_FAMILY_640_NOT_RETROACTIVE_PASS',shared_new_bytes=640*1024**2,ordinary_total_bytes=640*1024**2,minimum_cumulative_carry_bytes=547319808,minimum_whole_highwater_bytes=58987859968,accept_by_utc='2026-11-05T04:15:00Z',ordinary_expires_utc='2026-11-05T04:15:00Z',source06_never_accepted=True,old512breach_preserved=True,ordinarySealPrep={'offerId':offer,'entireUpperBytesChargedOnce':2097152},allocatorInstruction='Distinct07/actualrepair08;4PID1GiBRSS/WHOLE64GiB/WORK120FULL180; exactly10 unrun fixtures; no Rust/native/Cargo/PG/HTTP/browser/target')
 put(O/'source-budget-640-amendment.json',book)
 pins=json.loads((O/'source-pins.json').read_text())
 pins[str(O/'source-budget-640-amendment.json')]=hashlib.sha256((O/'source-budget-640-amendment.json').read_bytes()).hexdigest()
 maps=[{'path':str(p),'SHA256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in sorted(O.glob('source-high-*.json.gz'))]
 freeze={'epoch':time.time(),'retainedMaps':maps,'baselineSHA256':hashlib.sha256((O/'source-baseline.json.gz').read_bytes()).hexdigest(),'minimumNEW':547319808,'minimumWHOLE':58987859968,'allHigherAndUncapturedRetained':True,'old512breach':True,'Source06':'NEVER_ACCEPTED_UNREADY_YIELD','Source07':'NOT_ACCEPTED','prep2MiBEntireOnce':True,'noCensusOrTests':True}
 put(O/'source07-retained-inputs.json',freeze)
 pins[str(O/'source07-retained-inputs.json')]=hashlib.sha256((O/'source07-retained-inputs.json').read_bytes()).hexdigest()
 put(O/'source-pins.json',pins)
 dst=Path('docs/mcr/evidence/be39-post-content/control-source07-prospective');dst.mkdir()
 for name in ['source06-cap-refusal-0345.json','inprocess-1793850290499129000-FULL.json','inprocess-1793850290499129000-failure.json','source07-retained-inputs.json','source-budget-640-amendment.json']:
  shutil.copyfile(O/name,dst/name)
 doc=Path('docs/mcr/backend/post-content-moderation.md')
 doc.write_text(doc.read_text()+'\n\nSource06 remained never accepted. A later ordinary preaccept complete census refused the original cumulative512MiB cap; retained maps establish NEW at least545,222,656bytes (at least8,351,744bytes over536,870,912), with later uncaptured growth still open. Controller19523 and bootstrap guardian19526 exited before ordinary FULL1793850296.78805. No Source06 execution clock or fixture was created. The allocator explicitly amended the SAME family to640MiB through04:15UTC, preserving this breach, the original baseline, every retained higher map and unqualified gap. It also supplied a bounded ENTIRE2MiB incident-seal preparation allowance inside that family and distinct Source07/actual-repair08, each4PID/1GiBRSS/WHOLE64GiB/WORK120/FULL180. WHOLE carries at least58,987,859,968bytes and all higher historical values. These are prospective permissions, not a retrospective PASS; Source06 is never resumed. Exactly ten decoder/guard methods remain NOT_RUN, and backend/runtime acceptance remains open.\n')
 print(json.dumps({'state':'INCIDENT_READSET_SEALED_NOT_EXECUTION','freeze':freeze,'ordinaryRemainingUpper':2097152}),flush=True)
except BaseException as e:
 failure=type(e).__name__+': '+str(e);raise
finally:
 full={'ownerThreadId':OWNER,'offerId':offer,'state':'FULL','acceptedAtEpoch':now,'actualFullEpoch':time.time(),'resourcesReleased':True,'remainingNewUpperBytes':0,'resources':{'workflows':0,'guards':0,'children':0,'target':0},'failure':failure,'entire2MiBReservationRetained':True}
 put(Path(rec['receiptPaths']['full']),full);rec.update(state='FULL',actualFullEpoch=full['actualFullEpoch'],resourcesReleased=True,remainingNewUpperBytes=0);put(INDEX,rec)
