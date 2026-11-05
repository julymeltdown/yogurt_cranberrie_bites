"""Literal release after the exact actual source groups have exited."""
import hashlib,json,os,subprocess,sys,time
from pathlib import Path
O=Path(__file__).parent
owner='01a1a21a-a088-7c31-83e3-19de119ba873'
index=Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts')/(owner+'.json')
def put(p,v):
 t=p.with_suffix(p.suffix+'.tmp');t.write_text(json.dumps(v,indent=2)+'\n');os.replace(t,p)
n=sys.argv[1];assert n in ['07','08']
a=json.loads((O/('source-run-'+n)/'accept.json').read_text())
rec=json.loads(index.read_text());assert rec['offerId']==a['offerId'] and rec['acceptedAtEpoch']==a['acceptedAtEpoch'] and rec['state']=='ACCEPTED'
assert time.time()<a['fullDeadlineEpoch']
r=subprocess.check_output(['/bin/ps','-p',str(os.getpid()),'-o','lstart=,comm='],text=True).split(maxsplit=5)
obs={'actualpid':os.getpid(),'birth':' '.join(r[:5]),'executable':r[5].strip(),'cwd':os.getcwd(),'script':str(Path(__file__).absolute()),'role':'ordinary exact source process-group release metadata and literal FULL publication before log inspection','observedAt':time.time()}
rec['ordinaryObservation']=obs;put(index,rec)
rows=subprocess.check_output(['/bin/ps','-axo','pid=,pgid='],text=True)
groups={v['pgid'] for v in [a['controller'],*a['guardControllers']]}
left=[line for line in rows.splitlines() if len(v:=line.split())==2 and int(v[1]) in groups]
assert not left,'actual execution group remains live'
full={'ownerThreadId':owner,'offerId':a['offerId'],'state':'FULL','acceptedAtEpoch':a['acceptedAtEpoch'],'actualFullEpoch':time.time(),'resourcesReleased':True,'remainingNewUpperBytes':0,'resources':{'workflows':0,'children':0,'guards':0,'tests':0,'target':0,'Cargo':0,'PG':0,'browser':0,'ports':0,'cleanup':0},'absentExecutionGroups':sorted(groups),'originalFullDeadlineEpoch':a['fullDeadlineEpoch'],'noRuntimeOrGrowthPASSInferred':True,'target_access':'NONE'}
assert full['actualFullEpoch']<a['fullDeadlineEpoch']
p=Path(a['receiptPaths']['full']);put(p,full)
rec.update(state='FULL',actualFullEpoch=full['actualFullEpoch'],remainingNewUpperBytes=0,resourcesReleased=True,resources=full['resources'])
rec['ordinaryObservation']={**obs,'endedAt':time.time(),'exit':0};put(index,rec)
print(json.dumps({'FULL':full,'path':str(p),'SHA256':hashlib.sha256(p.read_bytes()).hexdigest()}),flush=True)
