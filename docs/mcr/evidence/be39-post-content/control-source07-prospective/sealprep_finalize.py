"""Continuation of the same ENTIRE2MiB preparation booking, not a new budget."""
import ast,hashlib,json,os,subprocess,time,sys
from pathlib import Path
O=Path(__file__).parent
index=Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts/01a1a21a-a088-7c31-83e3-19de119ba873.json')
def put(p,v):
 t=p.with_suffix(p.suffix+'.tmp');t.write_text(json.dumps(v,indent=2)+'\n');os.replace(t,p)
v=json.loads(index.read_text());assert v['state']=='FULL'
r=subprocess.check_output(['/bin/ps','-p',str(os.getpid()),'-o','lstart=,comm='],text=True).split(maxsplit=5)
obs={'actualpid':os.getpid(),'birth':' '.join(r[:5]),'executable':r[5].strip(),'cwd':os.getcwd(),'script':str(Path(__file__).absolute()),'role':'ordinary same SEALPREP2 continuation: exact prospective-control readset and AST syntax seal; no census/test/import/guardprobe','observedAt':time.time(),'booking':{'id':'POST-INCIDENT-SEALPREP2-0350','additionalUpperBytes':2097152,'expiresAt':'2026-11-05T04:15:00Z'}}
v['ordinaryObservation']=obs;put(index,v)
try:
 assert time.time()<1793852100
 dst=Path('docs/mcr/evidence/be39-post-content/control-source07-prospective')
 manifest=[]
 for name in ['budget.py','source_control.py','test_process_receipts.py','accounting.py','receipt_proof.py','release_source.py','sealprep2.py','sealprep_finalize.py']:
  data=(O/name).read_bytes();tree=ast.parse(data)
  (dst/name).write_bytes(data)
  methods=sum(isinstance(n,ast.FunctionDef) and n.name.startswith('test_') for n in ast.walk(tree))
  if name=='test_process_receipts.py':assert methods==10
  manifest.append({'path':name,'SHA256':hashlib.sha256(data).hexdigest(),'AST':'PASS','testMethods':methods})
 put(dst/'qualification.json',{'epoch':time.time(),'state':'AST_ONLY_PROSPECTIVE_SOURCE07_NOT_ACCEPTED','files':manifest,'all10Fixtures':'NOT_RUN','backendUnitsAndNative':'NOT_RUN','Source06':'NEVER_ACCEPTED_YIELD','prepEntire2MiBOnce':True,'same640Family':True,'kernelArgvFailure':'HISTORICAL_UNDIAGNOSED_NO_WAIVER','yieldDecoder':'AUTHORED_NOT_RUN_RETAINS_OLD_ACCEPTANCE','publication':'DIRECT_ROOT153_L03_PREDECESSOR_FRESH_CAPS_REQUIRED'})
 print(json.dumps({'AST':'PASS','fixtureMethods':10,'execution':'NOT_RUN','bytesCopied':sum((dst/e['path']).stat().st_size for e in manifest)}))
 if sys.argv[1:]==['commit']:
  subprocess.run(['git','diff','--check'],check=True)
  subprocess.run(['git','add','--','docs/mcr/backend/post-content-moderation.md',str(dst)],check=True)
  subprocess.run(['git','commit','-m','docs(be39): retain cumulative cap incident and Source07 preparation'],check=True)
  subprocess.run(['git','status','--short'],check=True)
finally:
 v=json.loads(index.read_text());v['ordinaryObservation']={**obs,'endedAt':time.time(),'exit':0};put(index,v)
