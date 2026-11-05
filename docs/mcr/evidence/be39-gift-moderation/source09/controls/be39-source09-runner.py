import os,sys,json,time,hashlib,subprocess,signal,shutil,threading,ast
from pathlib import Path
sys.dont_write_bytecode=True
ROOT=Path("/Users/lhs/Desktop/mcr-worktrees/be-gifts-live")
CONTROL=Path("/private/tmp/mcr-gift-public-ready-z8ch5q3t")
sys.path.insert(0,str(CONTROL))
from be39_parse_guard_source09 import IndependentGiftGuardian,allocated_bytes,sample_global_guard
GIB=1024**3
allocation=json.loads((CONTROL/"be39-source09-actual-accept.json").read_text())
output=Path(allocation["output_dir"])
started=allocation["actual_accept_epoch"]
work=started+120
full=started+180
roots=[ROOT,Path("/Users/lhs/Desktop/mcr/.git"),CONTROL,Path(allocation["global_controls"]["receipt_directory"])]
source_baseline={}
carry=allocation["retained_carry_bytes"]
whole_cap=allocation["whole_cap_bytes"]
peaks={"new":14880768,"rss":0,"processes":0,"whole":carry+14880768}
errors=[]
lock=threading.RLock()
stop=threading.Event()
process=None
guardian=None
watcher=None
report={"allocation_id":allocation["allocation_id"],"source_revision":allocation["source_revision"],"result":"FAIL","target_access":"NONE; saved baseline only","qualification":"ISOLATED_METADATA_AND_PARSE_ONLY_NOT_TYPES_SQL_NATIVE_RUNTIME","private_packet_commit":allocation["private_packet_commit"],"work_deadline_epoch":work,"full_deadline_epoch":full,"retained_carry_bytes":carry,"commands":[]}
controller_started=subprocess.check_output(["/bin/ps","-p",str(os.getpid()),"-o","lstart="],text=True,timeout=2).strip()
actual_executable=subprocess.check_output(["/bin/ps","-ww","-p",str(os.getpid()),"-o","comm="],text=True,timeout=2).strip()
executable_pin=json.loads((CONTROL/"be39-source09-readset.json").read_text())
if actual_executable!=executable_pin["actual_python_executable"] or hashlib.sha256(Path(actual_executable).read_bytes()).hexdigest()!=executable_pin["actual_python_executable_sha256"]: raise RuntimeError("actual macOS runtime executable changed")
cleaning=False
controller_receipt={"controller_started":controller_started,"controller_pid":os.getpid(),"controller_path":str(Path(__file__).resolve()),"actual_accept_epoch":started,"work_deadline_epoch":work,"full_deadline_epoch":full,"allocation_id":allocation["allocation_id"]}
with os.fdopen(os.open(output/"controller.json",os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600),"w") as stream: json.dump(controller_receipt,stream)
owner_record={"ownerThreadId":allocation["global_controls"]["owner_thread_id"],"offerId":allocation["allocation_id"],"cohort":"GiftBE39Source09","state":"ACCEPTED","acceptedAtEpoch":started,"workDeadlineEpoch":work,"fullDeadlineEpoch":full,"kind":"source","processCap":6,"rssCapBytes":512*1024**2,"remainingNewUpperBytes":16*1024**2,"controller":{"pid":os.getpid(),"birth":controller_started,"executable":actual_executable,"script":str(Path(__file__).resolve()),"cwd":str(Path.cwd()),"outputRoot":str(output)},"guardControllers":[],"receiptPaths":{"accept":str(CONTROL/"be39-source09-actual-accept.json"),"full":str(output/"FULL.json")}}
def publish_owner():
 path=Path(allocation["global_controls"]["receipt_directory"])/(owner_record["ownerThreadId"]+".json")
 temporary=path.with_suffix(".267.tmp")
 with os.fdopen(os.open(temporary,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600),"w") as stream: json.dump(owner_record,stream)
 os.replace(temporary,path)
publish_owner()
source_baseline={str(p):allocated_bytes(p) for p in roots}
print(json.dumps(controller_receipt),flush=True)
def require(c,m):
 if not c: raise RuntimeError(m)
def check():
 with lock:
  require(time.time()<work,"original120WORKdeadline")
  require(shutil.disk_usage(output).free>=8*GIB,"live8GiBfloor")
  growth=sum(max(0,allocated_bytes(p)-source_baseline[str(p)]) for p in roots)
  peaks["new"]=max(peaks["new"],allocated_bytes(Path(allocation.get("allocation_output_root",str(output))))+growth+14880768)
  peaks["whole"]=max(peaks["whole"],carry+peaks["new"])
  require(peaks["new"]<=16*1024**2 and peaks["whole"]<=whole_cap,"inclusiveNEW/WHOLEcap")
  rows=subprocess.check_output(["/bin/ps","-axo","pid=,ppid=,rss="],text=True,timeout=2)
  entries={int(p[0]):(int(p[1]),int(p[2])*1024) for row in rows.splitlines() if len(p:=row.split())==3}
  owned={os.getpid()}
  while True:
   more={p for p,(parent,_) in entries.items() if parent in owned}-owned
   if not more: break
   owned.update(more)
  rss=sum(entries[p][1] for p in owned if p in entries)
  peaks["rss"]=max(peaks["rss"],rss);peaks["processes"]=max(peaks["processes"],len(owned))
  require(len(owned)<=6 and rss<=512*1024**2,"6ALL/512MiBRSS")
  global_sample=sample_global_guard(config)
  report["global_sample"]=global_sample
  peaks["global_pids"]=max(peaks.get("global_pids",0),global_sample["distinct_workflow_pids"])
  peaks["global_rss"]=max(peaks.get("global_rss",0),global_sample["actual_rss_bytes"])
def watch():
 while not stop.wait(.25):
  try: check()
  except BaseException as e:
   errors.append(type(e).__name__+": "+str(e));os.kill(os.getpid(),signal.SIGTERM);return
def interrupt(*_):
 if cleaning: errors.append("guardian cancellation during cleanup")
 else: raise KeyboardInterrupt("source guard stopped")
signal.signal(signal.SIGTERM,interrupt);signal.signal(signal.SIGINT,interrupt)
try:
 require(time.time()<work,"expiredallocation")
 free=shutil.disk_usage(output).free
 require(free>=11*GIB and free-allocation["other_growth_bytes"]-16*1024**2>=9*GIB,"11/9admission")
 readset=json.loads((CONTROL/"be39-source09-readset.json").read_text())
 require(carry>=readset["retained_carry_bytes"] and whole_cap==readset["whole_limit"]==48*GIB,"nonshrinking retained carry/amended cap")
 require(hashlib.sha256(Path(readset["metadata_tool_executable"]).read_bytes()).hexdigest()==readset["metadata_tool_sha256"],"metadata executable changed")
 for path,sha in readset["inputs"].items(): require(hashlib.sha256((ROOT/path).read_bytes()).hexdigest()==sha,"readsetdrift")
 for path,sha in readset["private_paths"].items(): require(hashlib.sha256(Path(path).read_bytes()).hexdigest()==sha,"privateSDKdrift")
 for path,sha in readset["private_control_paths"].items(): require(hashlib.sha256(Path(path).read_bytes()).hexdigest()==sha,"privatecontrolchanged")
 for name in ("python","rustfmt"):
  require(hashlib.sha256(Path(readset[name+"_executable"]).read_bytes()).hexdigest()==readset[name+"_executable_sha256"],"runtimeexecutablechanged")
 require(subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip()==readset["source_revision"],"HEADdrift")
 require(not subprocess.check_output(["git","status","--porcelain"],cwd=ROOT,text=True),"dirtysource")
 config=dict(allocation_id=allocation["allocation_id"],stage="pure",allocation_output_root=allocation["allocation_output_root"],output=str(output),carry=carry,work_deadline=work,full_deadline=full,start_free=11*GIB,stop_free=8*GIB,other_growth=allocation["other_growth_bytes"],process_cap=6,rss_cap=512*1024**2,target_peak=0,source_peak=0,outside_peak=14880768,preparation_upper_bytes=14880768,source_baseline=source_baseline,target_cap=0,outside_cap=16*1024**2,whole_cap=whole_cap,admission_required=True,reserved_new_bytes=16*1024**2,after_all_floor=9*GIB,controller_started=controller_started,actual_accept_epoch=started,**allocation["global_controls"])
 guardian=IndependentGiftGuardian(config,CONTROL);guardian.start()
 guard_birth=subprocess.check_output(["/bin/ps","-p",str(guardian.process.pid),"-o","lstart="],text=True,timeout=2).strip()
 guard_executable=subprocess.check_output(["/bin/ps","-ww","-p",str(guardian.process.pid),"-o","comm="],text=True,timeout=2).strip()
 if guard_executable!=actual_executable: raise RuntimeError("guardian actual executable differs")
 owner_record["guardControllers"]=[{"pid":guardian.process.pid,"birth":guard_birth,"executable":actual_executable,"script":str(CONTROL/"be39_parse_guard_source09.py")}]
 publish_owner()
 check();watcher=threading.Thread(target=watch,daemon=True);watcher.start()
 tmp=output/"tmp";tmp.mkdir(mode=0o700)
 env=dict(os.environ,TMPDIR=str(tmp),PYTHONDONTWRITEBYTECODE="1")
 commands=[]
 for path in readset["private_control_paths"]: ast.parse(Path(path).read_text(),filename=path)
 report["python_ast"]="PASS"
 report["retained_qualified_source08"]="5metadata cases, 2firstRust parses, whitespace; overall08FAIL retained"
 for path in readset["parse_paths"]:
  commands.append([readset["rustfmt_executable"],"--emit","stdout","--edition","2021","--config","skip_children=true",path])
 for index,argv in enumerate(commands):
  check()
  with (output/("source-"+str(index)+".stdout")).open("xb") as stdout,(output/("source-"+str(index)+".stderr")).open("xb") as stderr:
   process=subprocess.Popen(argv,cwd=ROOT,env=env,stdout=stdout,stderr=stderr,start_new_session=True)
   while process.poll() is None:
    check();time.sleep(.1)
   code=process.wait()
  report["commands"].append({"argv":argv,"exit_code":code})
  require(code==0,"metadata/sourceparse/whitespacefailed")
 for path,sha in readset["private_paths"].items(): require(hashlib.sha256(Path(path).read_bytes()).hexdigest()==sha,"postparseprivateSDKdrift")
 check();require(not errors,"watchdogfailure")
 report["result"]="PASS"
except BaseException as e:
 report["failure"]=type(e).__name__+": "+str(e)
finally:
 cleaning=True
 stop.set()
 if watcher: watcher.join(timeout=3)
 if guardian:
  try: guardian.begin_cleanup()
  except Exception as e: errors.append(str(e))
 if process is not None and process.poll() is None:
  try: os.killpg(process.pid,signal.SIGTERM)
  except OSError as e: errors.append(str(e))
  try: process.wait(timeout=3)
  except subprocess.TimeoutExpired:
   try: os.killpg(process.pid,signal.SIGKILL)
   except OSError as e: errors.append(str(e))
   process.wait(timeout=3)
 if guardian:
  try:
   report["independent_guardian"]=guardian.stop()
   if report["independent_guardian"].get("result")!="PASS": report["result"]="FAIL"
  except Exception as e: errors.append(str(e))
 if errors: report["result"]="FAIL"
 report.update(finished_unix=time.time(),peaks=peaks,cleanup_errors=errors,owned_children=0 if process is None or process.poll() is not None else "UNKNOWN")
 path=output/"report.json";path.write_text(json.dumps(report,indent=2)+"\n");path.chmod(0o600)
 print(json.dumps(report))
sys.exit(report["result"]!="PASS")
