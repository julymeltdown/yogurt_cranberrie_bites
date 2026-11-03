import type { SessionController } from '../auth/model/session'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { HttpRequest } from '../../shared/api/http'
import { blockListSchema,blockSchema,decodeReceipt,freeze,parseReportLink,reasons,reportInputSchema,reportListSchema,reportSchema,sameScope,targetSchema,uuid,
 type BlockList,type CommandBinding,type Receipt,type Report,type ReportTarget,type SafetyOperation } from './contract'
/** Local owner projection of fresh server target/evidence authority. Not a new HTTP DTO. */
export interface ReportAuthorization{readonly scope:PrivacyScope;readonly target:ReportTarget;readonly expiresAt:number;readonly allowed:boolean;readonly evidenceMediaIds:readonly string[]}
export interface ReportAuthorizationPort{check(target:ReportTarget,evidenceMediaIds:readonly string[],scope:PrivacyScope,signal:AbortSignal):Promise<ReportAuthorization>}
export interface SafetyOptions{auth:SessionController;authorization?:ReportAuthorizationPort;reportsAvailable?:boolean;blocksAvailable?:boolean;
 onRestricted?:(targetUserId:string)=>void;onReportSubmitted?:(report:Report)=>void;now?:()=>number;newKey?:()=>string}
export interface SafetyState{readonly phase:'idle'|'authorizing'|'ready'|'submitting'|'pending'|'unknown'|'failed'|'succeeded'|'closed';readonly target:ReportTarget|null;
 readonly reason:typeof reasons[number];readonly text:string;readonly evidence:readonly string[];readonly report:Report|null;readonly blocks:BlockList|null;readonly blocksPhase:'idle'|'loading'|'ready'|'error';readonly receipt:Receipt|null;readonly message:string}
const initial=():SafetyState=>({phase:'idle',target:null,reason:'HARASSMENT',text:'',evidence:[],report:null,blocks:null,blocksPhase:'idle',receipt:null,message:''})
export function createSafetyController(options:SafetyOptions){
 const {auth}=options,scope=auth.privacy.capture(),now=options.now??Date.now,newKey=options.newKey??(()=>crypto.randomUUID())
 let state=freeze(initial()),disposed=false,sequence=0,intent:CommandBinding|null=null,flight:AbortController|null=null,listFlight:AbortController|null=null
 const listeners=new Set<()=>void>(),publish=(patch:Partial<SafetyState>)=>{state=freeze({...state,...patch});for(const f of Array.from(listeners)){try{f()}catch{/* Isolate observers. */}}}
 const current=()=>!disposed&&auth.privacy.isCurrent(scope)
 const cleanup=()=>{++sequence;flight?.abort();listFlight?.abort();flight=listFlight=null;intent=null;publish({...initial(),phase:'closed',message:'Private views were cleared. Reopen safety controls with your current session.'})}
 const unsubscribe=auth.privacy.registerCleanup(cleanup)
 const base=`/api/concert/events/${scope.eventId}`
 async function request<T>(input:Omit<HttpRequest<T>,'signal'>,signal:AbortSignal){return auth.privacy.run(scope,async context=>{
  const r=await auth.request({...input,signal:context.signal});if(r.scope.userId!==scope.userId||r.scope.eventId!==scope.eventId||r.scope.generation!==scope.generation)throw new Error('Changed identity');return r.data
 },signal)}
 const busy=()=>['submitting','pending','unknown'].includes(state.phase)||intent!==null
 async function authorize(target:ReportTarget,evidence:readonly string[],signal:AbortSignal){
  if(!options.authorization||options.reportsAvailable===false)throw new Error('Unavailable report authority')
  const proof=await auth.privacy.run(scope,context=>options.authorization!.check(target,evidence,scope,context.signal),signal)
  if(!proof.allowed||!sameScope(proof.scope,scope)||proof.target.target_type!==target.target_type||proof.target.target_id!==target.target_id
   ||!Number.isFinite(proof.expiresAt)||proof.expiresAt<=now()||proof.evidenceMediaIds.length!==evidence.length||evidence.some(id=>!proof.evidenceMediaIds.includes(id)))throw new Error('Target or evidence unavailable')
 }
 async function open(search:string){
  if(!current()||busy())return
  const target=parseReportLink(search),ticket=++sequence;flight?.abort();flight=new AbortController();const signal=flight.signal
  publish({...initial(),phase:'authorizing'})
  if(!target){publish({phase:'closed',message:'This report link is unavailable.'});return}
  try{await authorize(target,[],signal);if(current()&&ticket===sequence)publish({phase:'ready',target,message:'Your identity and report details are shared only with authorized reviewers.'})}
  catch{if(current()&&ticket===sequence)publish({phase:'closed',target:null,message:'This target is unavailable or reporting is not enabled. No private target data was opened.'})}
 }
 function restrict(target:string){
  if(!current())return
  auth.privacy.changePolicy(scope,'block')
  try{if(auth.session.isCurrent(scope))options.onRestricted?.(target)}catch{/* Privacy cleanup already completed. */}
  // // Both block and unblock invalidate private projections; old consent never returns.
 }
 function acceptReport(value:unknown,b:CommandBinding){const p=reportSchema.parse(value)
  if(p.event_id!==scope.eventId||p.target_type!==b.target.target_type||p.target_id!==b.target.target_id||b.resourceId!==null&&p.report_id!==b.resourceId)throw new Error('Unbound report')
  return freeze(p)
 }
 function acceptBlock(value:unknown,b:CommandBinding){const p=blockSchema.parse(value)
  if(p.target_user_id!==b.target.target_id||p.blocked_by_me!==(b.operation==='blockConcertUser'))throw new Error('Unbound block')
  return p
 }
 async function execute(operation:SafetyOperation,target:ReportTarget,body:ReturnType<typeof reportInputSchema.parse>|null){
  if(!current()||busy()||operation!=='createConcertReport'&&(options.blocksAvailable!==true||target.target_type!=='USER'||target.target_id===scope.userId))return
  const key=newKey();if(!uuid.safeParse(key).success)return
  const b=freeze({scope,operation,key,target,resourceId:operation==='createConcertReport'?null:target.target_id}) satisfies CommandBinding
  intent=b;const controller=new AbortController();flight=controller;const ticket=++sequence
  publish({phase:'submitting',message:'Submitting…',receipt:null,report:null})
  let dispatched=false
  const path=operation==='createConcertReport'?`${base}/reports`:`${base}/me/blocks/${target.target_id}`
  try{
   if(body)await authorize(target,body.evidence_media_ids,controller.signal)
   if(!current()||ticket!==sequence)return
   dispatched=true
   const value=await request({path,method:operation==='createConcertReport'?'POST':operation==='blockConcertUser'?'PUT':'DELETE',access:'protected',errorFamily:'domain',domainOperation:operation,idempotencyKey:key,
    ...(body?{body:{json:body}}:operation==='blockConcertUser'?{body:{json:{}}}:{}),decode:(v,c)=>{
     if(c.path!==path)throw new Error('Changed operation')
     if(c.status===202){const r=decodeReceipt(v,b,now());if(r.state!=='PENDING')throw new Error('Invalid acceptance');return{kind:'pending' as const,receipt:r}}
     if(c.status!==200)throw new Error('Invalid status')
     return operation==='createConcertReport'?{kind:'report' as const,report:acceptReport(v,b)}:{kind:'block' as const,block:acceptBlock(v,b)}
    }},controller.signal)
   if(!current()||ticket!==sequence)return
   if(value.kind==='pending')publish({phase:'pending',receipt:value.receipt,message:'Request pending. Check its status; it has not been confirmed complete.'})
   else if(value.kind==='report'){publish({phase:'succeeded',report:value.report,text:'',evidence:[],message:'Report submitted. Only authorized reviewers can access its details.'});try{if(current())options.onReportSubmitted?.(value.report)}catch{/* Safe result remains confirmed. */}}
   else restrict(value.block.target_user_id)
  }catch{if(current()&&ticket===sequence){if(!dispatched){intent=null;publish({phase:'ready',message:'Target or evidence authorization is unavailable. Nothing was submitted. Review your evidence before trying again.'})}else publish({phase:'unknown',message:'The outcome is not confirmed. Your draft is kept. Check status before any further action.'})}}
 }
 async function checkStatus(){
  const b=intent;if(!current()||!b||!['pending','unknown','failed'].includes(state.phase))return
  const controller=new AbortController();flight?.abort();flight=controller;const ticket=++sequence,path=`${base}/commands`
  publish({message:'Checking the original request…'})
  try{
   const r=await request({path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'getConcertCommandStatus',query:{operation_id:b.operation,idempotency_key:b.key},decode:v=>decodeReceipt(v,b,now())},controller.signal)
   if(!current()||ticket!==sequence)return
   if(r.state==='SUCCEEDED'){
    if(b.resourceId!==null&&r.resource_id!==b.resourceId)throw new Error('Changed resource')
    const bound=freeze({...b,resourceId:r.resource_id});intent=bound
    if(b.operation==='createConcertReport'){
     const page=await request({path:`${base}/me/reports`,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'listOwnConcertReports',query:{limit:100},decode:v=>reportListSchema.parse(v)},controller.signal)
     if(new Set(page.items.map(p=>p.report_id)).size!==page.items.length||page.items.some(p=>p.event_id!==scope.eventId))throw new Error('Invalid report list')
     const p=page.items.find(p=>p.report_id===r.resource_id);if(!p||p.revision<r.resource_revision!)throw new Error('Current report unavailable')
     if(!current()||ticket!==sequence)return;const report=acceptReport(p,bound)
     publish({phase:'succeeded',receipt:r,report,text:'',evidence:[],message:'Report submitted and its current status verified.'});try{if(current())options.onReportSubmitted?.(report)}catch{/* Observer cannot undo receipt. */}
    }else{
     const page=await request({path:`${base}/me/blocks`,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'listConcertBlocks',query:{limit:100},decode:v=>blockListSchema.parse(v)},controller.signal)
     if(new Set(page.items.map(p=>p.target_user_id)).size!==page.items.length||page.items.some(p=>!p.blocked_by_me||p.target_user_id===scope.userId))throw new Error('Invalid block list')
     const p=page.items.find(p=>p.target_user_id===b.target.target_id)
     if(p&&p.revision<r.resource_revision!)throw new Error('Stale block')
     if(b.operation==='blockConcertUser'?!p?.blocked_by_me:page.next_cursor!==null||p?.blocked_by_me)throw new Error('Current block state not verified')
     if(current()&&ticket===sequence)restrict(b.target.target_id)
    }
   }else publish({phase:r.state==='PENDING'?'pending':'failed',receipt:r,message:r.state==='PENDING'?'Request still pending. No action was replayed.':'Request failed or its receipt was retired. No replacement action was sent.'})
  }catch{if(current()&&ticket===sequence)publish({phase:'unknown',message:'Status or current resource is unavailable. This does not prove nonexecution. No action was replayed.'})}
 }
 async function loadBlocks(cursor?:string){
  if(!current())return
  if(options.blocksAvailable!==true){publish({blocksPhase:'error',blocks:null,message:'Block controls are unavailable.'});return}
  const controller=new AbortController();listFlight?.abort();listFlight=controller
  publish({blocksPhase:'loading',blocks:null})
  try{const page=await request({path:`${base}/me/blocks`,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'listConcertBlocks',query:{limit:20,...(cursor?{cursor}:{})},decode:v=>{
   const p=blockListSchema.parse(v);if(p.items.length>20||new Set(p.items.map(b=>b.target_user_id)).size!==p.items.length||p.items.some(b=>!b.blocked_by_me||b.target_user_id===scope.userId))throw new Error('Invalid block list');return p
  }},controller.signal)
  if(current()&&listFlight===controller)publish({blocksPhase:'ready',blocks:page})
  }catch{if(current()&&listFlight===controller)publish({blocksPhase:'error',blocks:null,message:'Blocked users could not be loaded. Try again.'})}
 }
 return{scope,blocksAvailable:options.blocksAvailable===true,read:()=>state,subscribe(f:()=>void){listeners.add(f);return()=>{listeners.delete(f)}},open,checkStatus,loadBlocks,
  edit(patch:Partial<Pick<SafetyState,'reason'|'text'|'evidence'>>){if(current()&&state.phase==='ready'&&!intent)publish(patch)},
  async report(){if(!state.target||state.phase!=='ready'||!current())return;const parsed=reportInputSchema.safeParse({...state.target,reason:state.reason,text:state.text,evidence_media_ids:state.evidence})
   if(!parsed.success){publish({message:'Choose a valid reason, at most 2000 text characters and up to four distinct evidence items.'});return}await execute('createConcertReport',state.target,parsed.data)},
  block(userId:string){const p=targetSchema.safeParse({target_type:'USER',target_id:userId});return p.success?execute('blockConcertUser',p.data,null):Promise.resolve()},
  unblock(userId:string){const p=targetSchema.safeParse({target_type:'USER',target_id:userId});return p.success?execute('unblockConcertUser',p.data,null):Promise.resolve()},
  close(){
   if(intent&&state.phase!=='succeeded'){++sequence;flight?.abort();flight=null;publish({...initial(),phase:'unknown',message:'An earlier request has an unresolved outcome. Check that original request; no new action will be sent.'})}
   else cleanup()
  },dispose(){if(disposed)return;cleanup();disposed=true;unsubscribe();listeners.clear()},
 }
}
export type SafetyController=ReturnType<typeof createSafetyController>
