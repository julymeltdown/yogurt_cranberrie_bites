from pathlib import Path
import subprocess,os,json,time,signal,shutil,sys
r=Path('/Users/lhs/.codex/worktrees/2411/mcr');out=r/'docs/mcr/evidence/css-parity-giveaway';node='/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node';link=r/'frontend/node_modules';target='/Users/lhs/Desktop/mcr/frontend/node_modules';round=sys.argv[1];temp=Path('/private/tmp/mcr-css-giveaway-'+round);cutoff=1793788779
assert time.time()<cutoff;assert not link.exists() and not link.is_symlink();link.symlink_to(target,target_is_directory=True)
proc=None;info={'started':time.time(),'job':'CSS-PARITY-GIVEAWAY-202611041034Z','activeCutoffEpoch':cutoff,'round':round}
try:
 env=dict(os.environ,PATH=str(Path(node).parent)+':'+os.environ['PATH'],TMPDIR=str(temp),MCR_GIVEAWAY_JOB=info['job'],MCR_GIVEAWAY_ROUND=round)
 with (out/(round+'.log')).open('w') as log:
  proc=subprocess.Popen([node,str(out/'capture.mjs')],cwd=r,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);info['pid']=proc.pid
  try:info['exitCode']=proc.wait(timeout=max(1,cutoff-time.time()-2))
  except subprocess.TimeoutExpired:
   info['timedOut']=True;os.killpg(proc.pid,signal.SIGTERM);time.sleep(1)
   try:os.killpg(proc.pid,signal.SIGKILL)
   except ProcessLookupError:pass
   info['exitCode']=proc.wait()
finally:
 if proc:
  try:os.killpg(proc.pid,signal.SIGTERM)
  except ProcessLookupError:pass
 if link.is_symlink() and os.readlink(link)==target:link.unlink()
 if temp.exists():shutil.rmtree(temp)
 info.update(finished=time.time(),ownLinkAbsent=not link.exists(),ownTempAbsent=not temp.exists(),durableBytes=sum(p.stat().st_size for p in out.rglob('*') if p.is_file()));(out/('supervisor-'+round+'.json')).write_text(json.dumps(info,indent=2)+'\n');print(json.dumps(info))
