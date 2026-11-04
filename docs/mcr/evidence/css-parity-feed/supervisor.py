import os,time,subprocess,signal,pathlib,shutil,json
root=pathlib.Path('/Users/lhs/.codex/worktrees/d51f/mcr')
slot=pathlib.Path('/tmp/mcr-css-feed-slot')
env=os.environ.copy();env['PATH']='/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin:'+env['PATH'];env['TMPDIR']=str(slot);env['MCR_FEED_REPO']=str(root);env['MCR_FEED_SLOT']='CSS-PARITY-FEED-202611040938Z'
start=time.monotonic();deadline=start+max(0,1793785108.973363+220-time.time())
node='/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
report={'started':time.time(),'commands':[],'freeMin':shutil.disk_usage(root).free,'wholePeak':0}
def total(p):
 return sum(f.stat().st_size for f in p.rglob('*') if f.is_file() and not f.is_symlink())
def run(args,phase=None):
 e=env.copy()
 if phase:e['MCR_FEED_PHASE']=phase
 log=root/'docs/mcr/evidence/css-parity-feed'/((phase or 'tests')+'.log')
 with log.open('w') as out:
  proc=subprocess.Popen(args,cwd=root/'frontend' if phase is None else root,env=e,stdout=out,stderr=subprocess.STDOUT,start_new_session=True)
  while proc.poll() is None:
   free=shutil.disk_usage(root).free;size=total(slot)+total(root/'docs/mcr/evidence/css-parity-feed');report['freeMin']=min(report['freeMin'],free);report['wholePeak']=max(report['wholePeak'],size)
   if time.monotonic()>deadline or free<8*1024**3 or size>193273528:
    os.killpg(proc.pid,signal.SIGTERM);time.sleep(1)
    if proc.poll() is None:os.killpg(proc.pid,signal.SIGKILL)
    raise RuntimeError('deadline/disk/whole guard')
   time.sleep(.3)
  report['commands'].append({'argv':args,'phase':phase,'exit':proc.returncode,'seconds':time.monotonic()-start})
  print(json.dumps(report['commands'][-1]),flush=True)
  return proc.returncode
try:
 driver=str(root/'docs/mcr/evidence/css-parity-feed/native.mjs')
 if run([node,driver],'before'):raise RuntimeError('baseline failed')
 run([node,driver],'round1')
 print('AWAIT_OWNER_FLAGS round2 or finish',flush=True)
 while time.monotonic()<deadline-35:
  if (slot/'round2').exists():
   (slot/'round2').unlink();run([node,driver],'round2');print('AWAIT_OWNER_FLAGS round2 or finish',flush=True)
  if (slot/'finish').exists():break
  time.sleep(.3)
 if time.monotonic()<deadline-15:
  run([node,str(root/'frontend/node_modules/vitest/vitest.mjs'),'run','--configLoader','runner','--no-cache','--maxWorkers','1','src/features/event-feed/__tests__/controller.test.ts','src/features/event-feed/__tests__/presentation.test.tsx','src/features/feed-thread/__tests__/controller.test.ts','src/features/feed-thread/__tests__/ui.test.tsx'])
finally:
 report['elapsed']=time.monotonic()-start
 (root/'docs/mcr/evidence/css-parity-feed/supervisor.json').write_text(json.dumps(report,indent=2)+'\n')
 print('SUPERVISOR_DONE',flush=True)
