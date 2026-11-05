import ast,fcntl,json,os,pathlib,subprocess,sys
p=pathlib.Path(__file__).resolve().parent
with (p/'source11/sampling.lock').open('a') as lock:
 fcntl.flock(lock,fcntl.LOCK_EX)
 code="import fcntl,sys;f=open(sys.argv[1],'a');\ntry:fcntl.flock(f,fcntl.LOCK_EX|fcntl.LOCK_NB)\nexcept BlockingIOError:sys.exit(0)\nelse:sys.exit(1)"
 result=subprocess.run([sys.executable,'-B','-c',code,str(p/'source11/sampling.lock')],timeout=5);assert result.returncode==0,'sampling lock did not exclude competing process'
 assert int(subprocess.check_output(['ps','-p',str(os.getpid()),'-o','pid='],text=True).strip())==os.getpid()
print(json.dumps({'newBoundary':'exclusive_cross_process_sampling_lock_and_serial_OS_helper','status':'PASS'}),flush=True)
for name in ['controller.py','guardian.py']:
 tree=ast.parse((p/name).read_text());nodes=[n for n in ast.walk(tree) if isinstance(n,ast.Assert) and '125112320 <= NEW_CARRY' in ast.unparse(n.test)];assert len(nodes)==1
 expr=compile(ast.Expression(nodes[0].test),str(p/name),'eval');os.environ['MCR_PI_FAMILY_CAP']='383037440'
 assert eval(expr,{'NEW_CARRY':304283648,'os':os}) is True
 assert eval(expr,{'NEW_CARRY':383037441,'os':os}) is False
 os.environ['MCR_PI_FAMILY_CAP']='383037441';assert eval(expr,{'NEW_CARRY':304283648,'os':os}) is False
 print(json.dumps({'newBoundary':name+' exact_authorized_carry_and_excess_rejection','status':'PASS'}),flush=True)
