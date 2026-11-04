from pathlib import Path
import subprocess,os,time,signal,json,shutil,datetime,hashlib
root=Path('/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish'); front=root/'frontend'; tmp=Path('/private/tmp/mcr-composed-preferences-source02'); out=Path('/private/tmp/mcr-composed-preferences-source02-results'); out.mkdir(exist_ok=False)
node='/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'; link=front/'node_modules'; ownlink=False; proc=None; started=time.monotonic(); peak=0; peaktemp=0; peakdurable=0; minfree=10**30; entries=[]; failure=None
commands=[ [node,'node_modules/vitest/vitest.mjs','run','src/world/preferences/__tests__','src/features/theme/__tests__/runtime-preferences','src/world/__tests__/runtime.test.ts','src/features/settings/runtime-adoption/__tests__','src/app/account-entry/__tests__/root.test.tsx','src/world/composition/__tests__/presentation-release.test.ts','src/app/__tests__/reference-frame-footer/background-contrast.test.ts','--config',str(tmp/'vitest.config.mjs'),'--configLoader','runner','--cache=false','--no-file-parallelism','--maxWorkers=1'],[node,'node_modules/typescript/bin/tsc','--project','tsconfig.json','--pretty','false','--incremental','false'],[node,'node_modules/oxlint/bin/oxlint','src/world/preferences','src/features/settings/runtime-adoption','src/world/runtime.ts','src/world/scenes/MapScene.ts','src/features/theme/model/controller.ts','src/world/composition/index.ts','src/app/world-panels/index.tsx','src/app/account-entry/panels.tsx','src/features/settings/SettingsPage.tsx','src/features/theme/ui/ThemeSelector.tsx','src/app/__tests__/cozy-backgrounds-v1','src/app/__tests__/reference-frame-footer/background-contrast.test.ts']]
def size(p):
 return sum(f.stat().st_blocks*512 for f in p.rglob('*') if f.is_file() and not f.is_symlink())
def guard():
 global peak,peaktemp,peakdurable,minfree
 t=size(tmp);d=size(out);w=t+d;free=shutil.disk_usage(root).free; peak=max(peak,w);peaktemp=max(peaktemp,t);peakdurable=max(peakdurable,d);minfree=min(minfree,free)
 if time.monotonic()-started>120 or w>64*1024**2 or t>48*1024**2 or d>8*1024**2 or free<8*1024**3: raise RuntimeError('resource-guard')
try:
 pin=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(); assert pin=='6eebba3e03910607007143a309b4c9aab4caa513';assert not subprocess.check_output(['git','status','--porcelain'],cwd=root);assert not link.exists() and not link.is_symlink();assert shutil.disk_usage(root).free>=16*1024**3
 runnerhash=hashlib.sha256((tmp/'vitest.config.mjs').read_bytes()).hexdigest();assert runnerhash=='c5325627051c7002797ea86ccda1b0e8fa4a96b7bc6519c1bf858216b2b79384'
 link.symlink_to('/Users/lhs/Desktop/mcr/frontend/node_modules',target_is_directory=True);ownlink=True
 env={**os.environ,'MCR_FE14_PYTHON':'/Users/lhs/Desktop/mcr-tools/contracts-validation-venv/bin/python','PATH':str(Path(node).parent)+':/Users/lhs/Desktop/mcr-tools/contracts-validation-venv/bin:'+os.environ['PATH'],'TMPDIR':str(tmp),'XDG_CACHE_HOME':str(tmp/'cache'),'CI':'true'}
 for i,cmd in enumerate(commands):
  at=datetime.datetime.now(datetime.timezone.utc).isoformat();log=out/f'{i+1}.log'
  with log.open('wb') as f:
   proc=subprocess.Popen(cmd,cwd=front,env=env,stdout=f,stderr=subprocess.STDOUT,start_new_session=True)
   while proc.poll() is None:guard();time.sleep(.2)
   guard();code=proc.returncode
  entries.append({'command':cmd,'started_at':at,'exit_code':code,'log':log.name});print(json.dumps(entries[-1]),flush=True)
  if code: raise RuntimeError(f'command-{i+1}-exit-{code}')
except BaseException as e: failure=str(e)
finally:
 if proc is not None:
  try:os.killpg(proc.pid,signal.SIGTERM)
  except ProcessLookupError:pass
 if ownlink and link.is_symlink():link.unlink()
 shutil.rmtree(tmp,ignore_errors=True)
 result={'source_revision':locals().get('pin'),'runner_sha256':locals().get('runnerhash'),'commands':entries,'failure':failure,'elapsed_seconds':time.monotonic()-started,'peak_whole_bytes':peak,'peak_temp_bytes':peaktemp,'peak_durable_bytes':peakdurable,'minimum_free_bytes':minfree,'owned_temp_absent':not tmp.exists(),'owned_link_absent':not link.exists(),'finished_at':datetime.datetime.now(datetime.timezone.utc).isoformat()};(out/'report.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
raise SystemExit(1 if failure else 0)
