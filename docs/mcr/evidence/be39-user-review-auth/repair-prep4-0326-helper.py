import ctypes, hashlib, json, os, shutil, sys, time
from pathlib import Path

OWNER='01a1a232-21e2-75c2-8474-a5fa6ae895cd'
BOOK='AUTH232-CLOSED05-REPAIRPREP4-0326'
DEADLINE=1793851200
record_path=Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts')/(OWNER+'.json')

class BSDInfo(ctypes.Structure):
    _fields_=[('prefix',ctypes.c_uint32*12),('comm',ctypes.c_char*16),('name',ctypes.c_char*32),
              ('counts',ctypes.c_uint32*5),('nice',ctypes.c_int32),('start_sec',ctypes.c_uint64),('start_usec',ctypes.c_uint64)]

# Kernel self-inspection only: no shell transition, subprocess, peer census or
# target access precedes publication of the factual ordinary descriptor.
proc=ctypes.CDLL('/usr/lib/libproc.dylib',use_errno=True)
info=BSDInfo();assert proc.proc_pidinfo(os.getpid(),3,0,ctypes.byref(info),ctypes.sizeof(info))==ctypes.sizeof(info)
image=ctypes.create_string_buffer(4096);assert proc.proc_pidpath(os.getpid(),image,len(image))>0
libc=ctypes.CDLL(None,use_errno=True);mib=(ctypes.c_int*3)(1,49,os.getpid())
size=ctypes.c_size_t(256*1024);args=ctypes.create_string_buffer(size.value)
assert libc.sysctl(mib,3,args,ctypes.byref(size),None,0)==0
raw=args.raw[:size.value];argc=int.from_bytes(raw[:4],sys.byteorder)
offset=raw.index(b'\0',4)+1
while raw[offset:offset+1]==b'\0':offset+=1
argv=[]
for _ in range(argc):
    end=raw.index(b'\0',offset);argv.append(os.fsdecode(raw[offset:end]));offset=end+1
birth=' '.join(time.asctime(time.localtime(info.start_sec)).split())
def observation(ended=False,code=None):
    record=json.loads(record_path.read_text())
    if not ended:
        prior=record.get('ordinaryObservation')
        if prior:record.setdefault('ordinaryObservationHistory',[]).append(prior)
        record['ordinaryObservation']={'actualpid':os.getpid(),'birth':birth,'kernelStartEpoch':info.start_sec+info.start_usec/1e6,
            'executable':os.fsdecode(image.value),'argv':argv,'script':str(Path(__file__)),'cwd':os.getcwd(),
            'role':'ordinary closed05 evidence/docs and separate suffix-aware decoder authoring only; no probes/tests/compiler/app/target',
            'booking':BOOK,'observedAt':time.time(),'endedAt':None,'positiveUpperBytes':4*1024**2,'deadlineEpoch':DEADLINE}
    else:
        assert record['ordinaryObservation']['actualpid']==os.getpid()
        record['ordinaryObservation'].update(endedAt=time.time(),exitCode=code)
    tmp=record_path.with_suffix('.tmp');tmp.write_text(json.dumps(record,indent=2)+'\n');tmp.replace(record_path)

observation()
code=1
try:
    assert time.time()<DEADLINE
    closed=Path('/private/tmp/mcr-auth232-be39-nonignored-05')
    successor=Path('/private/tmp/mcr-auth232-be39-nonignored-06')
    assert not successor.exists()
    successor.mkdir()
    controls=['run.py','guardian.py','budget.py','accounting.py','protocol_fixtures.py','plan.json','prepared-inputs.json',
              'closed-history-index.json','historical-receipt-aliases.json','source-cohort-overrides.json']
    for name in controls:shutil.copy2(closed/name,successor/name)
    for name in ['closed-history','queue-release-history']:shutil.copytree(closed/name,successor/name)
    wt=Path('/Users/lhs/Desktop/mcr-worktrees/be-auth-concert-deletion-232')
    evidence=wt/'docs/mcr/evidence/be39-user-review-auth'
    raw_dest=evidence/'validation-05-raw';raw_dest.mkdir()
    control_dest=evidence/'validation-05-controls';control_dest.mkdir()
    raw_names=['acceptance.json','TARGET.json','FULL.json','primary.failure.json','guardian.failure.structured.json','guardian.failure.json','guardian.result.json']
    raw_hashes={}
    for name in raw_names:
        data=(closed/name).read_bytes();(raw_dest/name).write_bytes(data);raw_hashes[name]=hashlib.sha256(data).hexdigest()
    assert raw_hashes['TARGET.json']=='6016caf2efaf6561dfdfdf7fb78232dcd6b9a7158546fbd09dc52880ddd19f23'
    assert raw_hashes['FULL.json']=='a4d019d3fc4b22550727fc9c0104a6d732dd72bfe3c490e6ca0d90661c2e9626'
    for name in controls:shutil.copy2(closed/name,control_dest/name)
    metadata={'task_ids':['BE39-AUTH-LIFECYCLE','QA09-AUTH-DELETION-OBSERVATION'],'status':'IN_PROGRESS',
        'attempt':'AUTH232-NONIGNORED05-0233','source':'a7c4c905ef4702811795dae6583b7b21629f60dc',
        'implementation_unchanged_from':'b0f2b56217bb48ab2c0c1f7c0e42720de6cbf8bc',
        'result':'FAILED_OWN_HISTORICAL_FULL_JSONL_DECODER_BEFORE_INITIALS_AND_PRODUCT_COMMANDS',
        'accepted_epoch':1793848796.0535738,'work_deadline':1793849696.0535738,'full_deadline':1793849876.0535738,
        'target_release_epoch':1793848797.28386,'full_release_epoch':1793848797.383717,
        'target_access':'NONE','workflows':0,'owned_children':0,'remaining_pids':[],'remaining_new_reservation':0,
        'complete_initial_samples':'NOT_ESTABLISHED','protocol_fixtures':'NOT_RUN','formatter':'NOT_RUN','cargo':'NOT_RUN','product_tests':'NOT_RUN',
        'commands':[],'copies':[],'qualified_fresh_whole_or_new_peak':'NONE','retained_incoming_whole_floor':58895065088,
        'retained_new_floor':143654912,'frozen_plan_sha256':'fb8caf570c42917ce67fbaedeb7c556cb0e597f7603ad615898cbc30e11bcf23',
        'raw_sha256':raw_hashes,'callsite':str(closed/'budget.py')+':209 reconciled_cohorts raw=proof.read_bytes();json.loads(raw)',
        'saved_exception':'JSONDecodeError: Extra data: line 2 column 1 (char 368)',
        'exact_runtime_input_path':'NOT_CAPTURED; do not recertify from the mutable membership file',
        'verified_shared_jsonl_example':{'path':'/Users/lhs/Desktop/mcr-worktrees/fe-runtime-preferences/docs/mcr/evidence/parade-ink-foundation/native10-controller.jsonl',
            'sha256':'c7abe5064ec8ecb470ea9513ae81ee95f201ed0ce0fa76996c92a6d0296bd968','first_line_bytes':368,
            'inference':'Likely runtime input because offset matches; not a saved runtime pathname witness'},
        'secondary_peer_observation':{'owner':'01a1a213-61d6-7a20-a8e7-476649312f8c','round':'PRIVATE-NATIVE14-0314',
            'observed_guard_pid':30618,'observed_command':'<defunct>','disposition':'Historical strict identity failure preserved; no retroactive PASS or exemption'},
        'release_routing':'Actual TARGET/FULL sent directly21d/207/World before diagnosis/docs; publisher412e and PI12 subsequently received factual closure',
        'closed_round_immutable':True,'preparation_booking':BOOK,'preparation_positive_upper_bytes':4194304,
        'preparation_deadline_epoch':DEADLINE,'preparation_charged_entire_once_inside_same_auth_2gib':True,
        'minimum_new_retention_after_preparation':147849216,
        'goal_metadata':'Historical BE33 goal remains blocked and incompatible; no false reset/completion',
        'remaining':['Separate suffix-aware decoder repair and authored regression, no execution under ordinary book.',
            'Fresh distinct execution offer and fair queue before any successor runtime work.',
            'Developer validation/fixes and qualified direct publisher handoff; real native/PG/mTLS still NOT_RUN.']}
    (evidence/'validation-05.json').write_text(json.dumps(metadata,indent=2)+'\n')
    def relocate(v):
        if isinstance(v,str):return v.replace(str(closed),str(successor))
        if isinstance(v,list):return [relocate(x) for x in v]
        if isinstance(v,dict):return {k:relocate(x) for k,x in v.items()}
        return v
    for name in ['plan.json','prepared-inputs.json','historical-receipt-aliases.json','source-cohort-overrides.json']:
        value=relocate(json.loads((successor/name).read_text()));(successor/name).write_text(json.dumps(value,indent=2)+'\n')
    p=json.loads((successor/'plan.json').read_text());request_offer='AUTH232-NONIGNORED06-REQUEST-0326'
    p.update(offer=request_offer,state='PREPARED_SUCCESSOR_REQUEST_NOT_OFFERED_NOT_ACCEPTED',execution_grant_issued=False,
        queue_ready=False,retained_new_floor=141*1024**2,offer_expires_epoch=0)
    p['roots']['closed_nonignored05']=str(closed)
    p['preparation_authority']={'offer':BOOK,'deadlineEpoch':DEADLINE,'additionalUpperBytes':4194304,
        'chargedEntireOnceInsideSameCumulative2GiB':True,'retainOld137MiBPlusEntireFresh4MiB':True,'executionPermitted':False}
    p['queue_requirements_remaining']=['fresh distinct execution offer','actual fair-queue predecessor releases and fresh capacity']
    p['pending']=['suffix-aware decoder repair and regression authoring','final committed source/control freeze','fresh execution grant; no06 execution currently authorized']
    p['predecessor_proofs']=[]
    p['control_sha256']={}
    p['final_custody_basis']='NO_CURRENT_SUCCESSOR_TARGET_CUSTODY; closed05 literal receipts routed207next/Worldlater'
    p['source_amendment']='Separate successor request derived from closed05; decoder repair not executed; backend unchangedb0.'
    (successor/'plan.json').write_text(json.dumps(p,indent=2)+'\n')
    prepared=json.loads((successor/'prepared-inputs.json').read_text());prepared.update(offer=request_offer,state='PREPARED_SUCCESSOR_NOT_OFFERED_NOT_ACCEPTED',pending=p['pending'])
    (successor/'prepared-inputs.json').write_text(json.dumps(prepared,indent=2)+'\n')
    history=json.loads((successor/'closed-history-index.json').read_text())
    history.append({'offerId':'AUTH232-NONIGNORED05-0233','state':'FULL','acceptedAtEpoch':1793848796.0535738,
        'actualFullEpoch':1793848797.383717,'resourcesReleased':True,'fullPath':str(closed/'FULL.json'),
        'fullSHA256':raw_hashes['FULL.json'],'qualification':'Closed own JSONL decoder failure before complete initials/product work'})
    (successor/'closed-history-index.json').write_text(json.dumps(history,indent=2)+'\n')
    print('PRESERVED_CLOSED05_AND_CREATED_UNOFFERED06; no imported controls/probes/tests')
    code=0
finally:observation(True,code)
