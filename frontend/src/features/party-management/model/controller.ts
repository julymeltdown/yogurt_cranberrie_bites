import { validCapabilities } from '../../../app/capabilities'
import { TransportError } from '../../../shared/api/errors'
import type { Decoder, HttpRequest } from '../../../shared/api/http'
import type { PrivacyScope } from '../../../shared/query/privacy'
import type { SessionController } from '../../auth/model/session'
import type { PartyAccessPort } from '../../party-discovery/model/controller'
import { decodeParty, type Party, type Application, type Member, type Page, type Receipt } from '../../party-discovery/model/contract'
import { contractValue, receiptDecoder, uuid, type Binding } from '../../party-authoring/model/contract'
export type RallyState='ON_MY_WAY'|'HERE'|'MET'|'NEED_HELP'|'LEFT'
export interface RallyEntry {user_id:string;state:RallyState;meeting_revision:number;revision:number;self_reported_at:string}
export interface Rally {party_id:string;event_id:string;meeting_revision:number;beacon_id:string|null;entries:readonly RallyEntry[]}
export interface MeetingAuthority { readonly scope:PrivacyScope;readonly partyId:string;readonly meetingRevision:number;readonly expiresAt:number;readonly description:string }
export interface ManagementOptions {auth:Pick<SessionController,'session'|'privacy'|'request'>;access:PartyAccessPort;partyId:string;authority?:{read():MeetingAuthority|null;subscribe(listener:()=>void):()=>void};newKey?:()=>string;now?:()=>number}
type Phase='idle'|'loading'|'ready'|'sending'|'uncertain'|'pending'|'checking'|'succeeded'|'failed'|'denied'
interface State {readonly phase:Phase;readonly party:Party|null;readonly applications:Page<Application>|null;readonly members:Page<Member>|null;readonly rally:Rally|null;readonly message:string;readonly confirmation:{kind:'remove'|'cancel';id:string}|null;readonly retryAt:number}
export function createPartyManagement(o:ManagementOptions){
 const {auth,access,partyId}=o,now=o.now??Date.now,listeners=new Set<()=>void>()
 let state:State=Object.freeze({phase:'idle',party:null,applications:null,members:null,rally:null,message:'',confirmation:null,retryAt:0}),disposed=false,flight:AbortController|null=null,binding:Binding|null=null
 let expiry:ReturnType<typeof setTimeout>|undefined
 let commandTarget:{kind:'decision'|'remove'|'cancel'|'rally';id:string;desired:string;revision:number;applicant?:string;meetingRevision?:number}|null=null
 const emit=(p:Partial<State>)=>{if(disposed)return;state=Object.freeze({...state,...p});for(const f of Array.from(listeners)){try{f()}catch{}}}
 const same=(a:PrivacyScope,b:PrivacyScope)=>a.userId===b.userId&&a.eventId===b.eventId&&a.generation===b.generation&&a.policyGeneration===b.policyGeneration&&a.gate===b.gate
 function capability(safety=false){const s=auth.privacy.capture(),a=access.read();return !disposed&&uuid(partyId)&&auth.privacy.isCurrent(s)&&uuid(s.eventId)&&uuid(s.userId)&&!!a&&same(a.scope,s)&&validCapabilities(a.capabilities)&&a.capabilities.meet.available&&(['OPEN','LIVE'].includes(a.phase)&&a.capabilities.mutations.available||safety&&a.phase==='READ_ONLY'&&!a.capabilities.mutations.available&&a.capabilities.mutations.reason==='EVENT_READ_ONLY')}
 function readable(){const a=access.read(),s=auth.privacy.capture();return !disposed&&uuid(partyId)&&auth.privacy.isCurrent(s)&&!!a&&same(a.scope,s)&&validCapabilities(a.capabilities)&&a.capabilities.meet.available}
 const owner=()=>!!state.party&&state.party.owner.user_id===auth.session.read().userId
 const active=()=>!!state.party&&['OPEN','CLOSED'].includes(state.party.state)
 const blocked=()=>!['ready','succeeded','failed'].includes(state.phase)||state.retryAt>now()
 function scrub(){clearTimeout(expiry);flight?.abort();flight=null;binding=null;commandTarget=null;emit({phase:'idle',party:null,applications:null,members:null,rally:null,message:'',confirmation:null,retryAt:0})}
 const cleanup=auth.privacy.registerCleanup(scrub),stop=access.subscribe(()=>{if(!readable())scrub()})
 function base(s:PrivacyScope){return `/api/concert/events/${s.eventId}`}
 async function request<T>(s:PrivacyScope,c:AbortController,input:Omit<HttpRequest<T>,'signal'>){return auth.privacy.run(s,async({signal})=>(await auth.request({...input,signal})).data,c.signal)}
 const live=(s:PrivacyScope,c:AbortController)=>!disposed&&flight===c&&!c.signal.aborted&&auth.privacy.isCurrent(s)&&readable()
 function failure(e:unknown,sent=false){const t=e instanceof TransportError?e:null,r=t?.server?.reason,unknown=sent&&(!t||t.uncertain||!t.status||t.status>=500||t.status===404||r==='IDEMPOTENCY_EXPIRED'||r==='IDEMPOTENCY_CONFLICT')
  if(t?.status===401||t?.status===403){scrub();emit({phase:'denied',message:'Party access was removed or is unavailable.'});return}
  emit({phase:unknown?'uncertain':'failed',confirmation:null,retryAt:t?.status===429?now()+Math.max(1000,t.retryAfterMs??1000):0,message:unknown?'Outcome unknown. Check the original command before another action.':r==='PARTY_FULL'?'The last slot was taken. Current party data must be read again.':r==='VERSION_CONFLICT'?'The party changed. Read current information again.':'This action is unavailable. Read current information again.'})
 }
 function decoder<T>(name:string,path:string,check:(v:T)=>void):Decoder<T>{return(v,c)=>{if(c.path!==path||c.status!==200)throw new Error('Invalid context');const data=contractValue<T>(name,v);check(data);return data}}
 function memberContext(m:Member,s:PrivacyScope){if(m.event_id!==s.eventId||m.party_id!==partyId)throw new Error('Invalid member')}
 function appContext(a:Application,s:PrivacyScope){if(a.event_id!==s.eventId||a.party_id!==partyId)throw new Error('Invalid application')}
 function page<T>(p:Page<T>,id:(t:T)=>string){if(p.items.length>20||new Set(p.items.map(id)).size!==p.items.length)throw new Error('Invalid page')}
 async function snapshot(s:PrivacyScope,c:AbortController){const path=`${base(s)}/parties/${partyId}`,p=await request(s,c,{path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'getConcertParty',decode:decodeParty(partyId,path)})
  if(!live(s,c))return
  // Always scrub old meeting/rally immediately; superseded revision cannot remain actionable.
  emit({party:p,rally:null,applications:null,members:null});authorityChanged()
  if(!['OPEN','CLOSED'].includes(p.state))return
  const mp=`${path}/members`,members=await request(s,c,{path:mp,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'listConcertMembers',query:{limit:20},decode:decoder<Page<Member>>('Domain_party_MemberList',mp,v=>{page(v,m=>m.user_id);v.items.forEach(m=>memberContext(m,s))})})
  if(!live(s,c))return;emit({members})
  if(p.owner.user_id===s.userId){const ap=`${path}/applications`,apps=await request(s,c,{path:ap,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'listConcertPartyApplications',query:{limit:20},decode:decoder<Page<Application>>('Domain_party_ApplicationList',ap,v=>{page(v,a=>a.application_id);v.items.forEach(a=>appContext(a,s))})});if(live(s,c))emit({applications:apps})}
 }
 async function load(){if(!readable()||['sending','checking','loading'].includes(state.phase)||state.retryAt>now())return
  const uncertain=['pending','uncertain'].includes(state.phase),s=auth.privacy.capture(),c=new AbortController();flight?.abort();flight=c;emit({phase:'loading',message:'',confirmation:null})
  try{await snapshot(s,c);if(live(s,c))emit({phase:uncertain?'uncertain':'ready',message:uncertain?'Current information loaded. The original command still needs recovery.':''})}catch(e){if(live(s,c))failure(e,uncertain)}
 }
 async function more(kind:'members'|'applications'){const old=state[kind],s=auth.privacy.capture();if(!readable()||blocked()||!old?.next_cursor||kind==='applications'&&!owner())return
  const c=new AbortController();flight?.abort();flight=c;emit({phase:'loading'})
  const path=`${base(s)}/parties/${partyId}/${kind}`,isMember=kind==='members'
  try{const p=await request(s,c,{path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:isMember?'listConcertMembers':'listConcertPartyApplications',query:{limit:20,cursor:old.next_cursor},decode:decoder<Page<Member|Application>>(isMember?'Domain_party_MemberList':'Domain_party_ApplicationList',path,v=>{page(v,item=>'user_id'in item?item.user_id:item.application_id);v.items.forEach(item=>{if('user_id'in item)memberContext(item,s);else appContext(item,s)})})})
   if(p.next_cursor===old.next_cursor)throw new Error('Repeated continuation');const items=[...old.items,...p.items];if(new Set(items.map(item=>'user_id'in item?item.user_id:item.application_id)).size!==items.length)throw new Error('Duplicate continuation')
   if(live(s,c))emit({...isMember?{members:{items:items as Member[],next_cursor:p.next_cursor}}:{applications:{items:items as Application[],next_cursor:p.next_cursor}},phase:'ready'})
  }catch(e){if(live(s,c))failure(e)}
 }
 function authority(){const a=o.authority?.read(),s=auth.privacy.capture();return a&&same(a.scope,s)&&a.partyId===partyId&&a.meetingRevision===state.party?.meeting_revision&&a.expiresAt>now()?a:null}
 function authorityChanged(){clearTimeout(expiry);const a=authority();if(!a){emit({rally:null});return}
  if(state.rally&&state.rally.meeting_revision!==a.meetingRevision)emit({rally:null});else emit({})
  expiry=setTimeout(authorityChanged,Math.min(Math.max(1,a.expiresAt-now()),2147483647))
 }
 const stopAuthority=o.authority?.subscribe(authorityChanged)
 async function loadRally(){if(!readable()||blocked()||!active()||!state.members||state.members.next_cursor||!state.members.items.some(m=>m.user_id===auth.session.read().userId&&m.state==='ACTIVE')||!authority())return
  const s=auth.privacy.capture(),c=new AbortController();flight?.abort();flight=c;const path=`${base(s)}/parties/${partyId}/rally`;emit({phase:'loading',rally:null})
  try{const r=await request(s,c,{path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'getConcertRally',decode:decoder<Rally>('Domain_party_Rally',path,v=>{if(v.party_id!==partyId||v.event_id!==s.eventId||v.meeting_revision!==state.party?.meeting_revision||new Set(v.entries.map(e=>e.user_id)).size!==v.entries.length||v.entries.some(e=>e.meeting_revision!==v.meeting_revision||!state.members?.items.some(m=>m.user_id===e.user_id&&m.state==='ACTIVE')))throw new Error('Invalid rally context')})})
   if(live(s,c)&&authority()){emit({rally:r,phase:'ready'});authorityChanged()}
  }catch(e){if(live(s,c))failure(e)}
 }
 async function recoverResult(s:PrivacyScope,c:AbortController,r:Receipt){
  if(r.state==='PENDING'){emit({phase:'pending',message:'Command pending. Check its status.'});return}
  if(r.state==='RETIRED'||Date.parse(r.expires_at)<=now()){emit({phase:'uncertain',message:'Receipt unavailable or expired. This does not prove nonexecution.'});return}
  if(r.state==='FAILED'){emit({phase:'failed',message:'The original command failed. Read current information before another action.'});await snapshot(s,c);return}
  const target=commandTarget!;await snapshot(s,c);if(!live(s,c))return
  // Receipt IDs never substitute for the exact event/party/user authorized reread.
  if(target.kind==='cancel'&&state.party?.state==='CANCELLED'&&state.party.revision>=r.resource_revision!){emit({phase:'succeeded',message:'Party cancellation confirmed.'});return}
  if(target.kind==='decision'){const path=`${base(s)}/party-applications/${target.id}`,a=await request(s,c,{path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'getConcertApplication',decode:decoder<Application>('Domain_party_Application',path,v=>{appContext(v,s);if(v.application_id!==target.id||v.applicant.user_id!==target.applicant||v.revision<r.resource_revision!||v.state!==target.desired)throw new Error('Invalid recovered decision')})});if(live(s,c)&&a)emit({phase:'succeeded',message:'Application decision confirmed.'});return}
  if(target.kind==='remove'){
   let members=state.members, found=members?.items.find(m=>m.user_id===target.id),pages=0;const seen=new Set<string>()
   while(!found&&members?.next_cursor&&pages++<10){const cursor=members.next_cursor;if(seen.has(cursor))throw new Error('Repeated member cursor');seen.add(cursor)
    const path=`${base(s)}/parties/${partyId}/members`;members=await request(s,c,{path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'listConcertMembers',query:{limit:20,cursor},decode:decoder<Page<Member>>('Domain_party_MemberList',path,v=>{page(v,m=>m.user_id);v.items.forEach(m=>memberContext(m,s))})});found=members.items.find(m=>m.user_id===target.id)
   }
   if(live(s,c)&&found?.state==='REMOVED'&&found.revision>=r.resource_revision!){emit({phase:'succeeded',message:'Member removal confirmed for this party.'});return}
  }
  if(target.kind==='rally'&&authority()&&state.party?.meeting_revision===target.meetingRevision){
   const path=`${base(s)}/parties/${partyId}/rally`,rally=await request(s,c,{path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'getConcertRally',decode:decoder<Rally>('Domain_party_Rally',path,v=>{if(v.event_id!==s.eventId||v.party_id!==partyId||v.meeting_revision!==target.meetingRevision||new Set(v.entries.map(e=>e.user_id)).size!==v.entries.length||v.entries.some(e=>e.meeting_revision!==v.meeting_revision))throw new Error('Invalid recovered rally')})})
   const self=rally.entries.find(e=>e.user_id===s.userId)
   if(live(s,c)&&authority()&&self&&self.state===target.desired&&self.revision>=r.resource_revision!){emit({phase:'succeeded',rally,message:'Your explicit self-report was confirmed for this meeting.'});return}
  }
  emit({phase:'uncertain',message:'Receipt found. The exact authorized result is not yet verified. Do not repeat the command.'})
 }
 async function send(kind:'decision'|'remove'|'cancel'|'rally',id:string,desired:string,ack=false){if(blocked()||!active()||!capability(kind==='remove'||kind==='cancel')||!state.party)return
  const p=state.party,a=state.applications?.items.find(a=>a.application_id===id),m=state.members?.items.find(m=>m.user_id===id),s=auth.privacy.capture()
  if(kind!=='rally'&&!owner()||kind==='decision'&&(!a||a.state!=='PENDING'||!['APPROVED','REJECTED'].includes(desired)||desired==='APPROVED'&&(p.state!=='OPEN'||p.member_count>=p.capacity))||kind==='remove'&&(!m||m.role==='OWNER'||m.state!=='ACTIVE'||state.confirmation?.kind!=='remove'||state.confirmation.id!==id)||kind==='cancel'&&(state.confirmation?.kind!=='cancel')||kind==='rally'&&(!ack||!authority()||!state.rally||!['ON_MY_WAY','HERE','MET','NEED_HELP','LEFT'].includes(desired)))return
  const operation=kind==='decision'?'decideConcertApplication':kind==='remove'?'removeConcertMember':kind==='cancel'?'cancelConcertParty':'setConcertRally'
  const path=kind==='decision'?`${base(s)}/party-applications/${id}/decision`:`${base(s)}/parties/${partyId}/${kind==='remove'?`members/${id}`:kind==='cancel'?'cancel':'rally/me'}`
  const self=state.rally?.entries.find(e=>e.user_id===s.userId),body=kind==='decision'?{expected_revision:a!.revision,party_revision:p.revision,decision:desired}:kind==='remove'?{expected_revision:m!.revision,party_revision:p.revision}:kind==='cancel'?{expected_revision:p.revision}:{expected_revision:self?.revision??0,state:desired,meeting_revision:p.meeting_revision,physical_presence_acknowledged:true}
  contractValue(kind==='decision'?'Domain_party_ApplicationDecision':kind==='remove'?'Domain_party_RemoveMember':kind==='cancel'?'Domain_common_RevisionRequest':'Domain_party_SetRally',body)
  const key=(o.newKey??(()=>crypto.randomUUID()))();if(!uuid(key))return
  binding={scope:s,operation,key,target:kind==='decision'||kind==='remove'?id:kind==='cancel'?partyId:s.userId};commandTarget={kind,id,desired,revision:kind==='decision'?a!.revision:kind==='remove'?m!.revision:kind==='cancel'?p.revision:self?.revision??0,...a?{applicant:a.applicant.user_id}:{},...kind==='rally'?{meetingRevision:p.meeting_revision}:{}}
  const b=binding,target=commandTarget,c=new AbortController();flight?.abort();flight=c;emit({phase:'sending',confirmation:null,message:''})
  try{const result=await request<unknown>(s,c,{path,method:kind==='remove'?'DELETE':kind==='rally'?'PUT':'POST',access:'protected',errorFamily:'domain',domainOperation:operation,idempotencyKey:key,...kind==='remove'?{query:body as {expected_revision:number;party_revision:number}}:{body:{json:body}},decode:(v,ctx)=>{
   if(ctx.status===202)return receiptDecoder(b,path)(v,ctx)
   if(ctx.status!==200||ctx.path!==path)throw new Error('Invalid command context')
   if(kind==='decision'){const a=contractValue<Application>('Domain_party_Application',v);appContext(a,s);if(a.application_id!==id||a.applicant.user_id!==target.applicant||a.state!==desired||a.revision<=target.revision)throw new Error('Invalid decision');return a}
   if(kind==='remove'){const m=contractValue<Member>('Domain_party_Member',v);memberContext(m,s);if(m.user_id!==id||m.state!=='REMOVED'||m.revision<=target.revision)throw new Error('Invalid removal');return m}
   if(kind==='cancel'){const party=decodeParty(partyId,path)(v,ctx);if(party.owner.user_id!==s.userId||party.state!=='CANCELLED'||party.revision<=p.revision)throw new Error('Invalid cancellation');return party}
   const r=contractValue<RallyEntry>('Domain_party_RallyEntry',v);if(r.user_id!==s.userId||r.meeting_revision!==p.meeting_revision||r.state!==desired||r.revision<=target.revision)throw new Error('Invalid rally');return r
  }})
  if(!live(s,c))return
  if(result&&typeof result==='object'&&'receipt_id'in result)await recoverResult(s,c,result as Receipt)
  else{await snapshot(s,c);if(live(s,c))emit({phase:'succeeded',message:kind==='rally'?'Your explicit self-report was confirmed.':'The server confirmed this action.'})}
  }catch(e){if(live(s,c)){failure(e,true);if(state.phase==='failed')try{await snapshot(s,c)}catch(err){if(live(s,c))failure(err)}}}
 }
 async function checkStatus(){const b=binding;if(!b||!readable()||!auth.privacy.isCurrent(b.scope)||!['uncertain','pending'].includes(state.phase)||state.retryAt>now())return
  const c=new AbortController();flight?.abort();flight=c;const path=`${base(b.scope)}/commands`;emit({phase:'checking'})
  try{const r=await request(b.scope,c,{path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'getConcertCommandStatus',query:{operation_id:b.operation,idempotency_key:b.key},decode:receiptDecoder(b,path)});if(live(b.scope,c))await recoverResult(b.scope,c,r)}catch(e){if(live(b.scope,c))failure(e,true)}
 }
 return {read:()=>state,subscribe(f:()=>void){listeners.add(f);return()=>{listeners.delete(f)}},load,more,loadRally,checkStatus,isOwner:owner,meeting:authority,
  decide:(id:string,decision:'APPROVED'|'REJECTED')=>send('decision',id,decision),
  confirm(kind:'remove'|'cancel',id=partyId){if(!blocked()&&owner())emit({confirmation:{kind,id}})},dismiss(){emit({confirmation:null})},
  remove:(id:string)=>send('remove',id,'REMOVED'),cancel:()=>send('cancel',partyId,'CANCELLED'),report:(state:RallyState,acknowledged:boolean)=>send('rally',auth.session.read().userId??'',state,acknowledged),
  reportIntent:(intent:{state:RallyState;meeting_revision:number;physical_presence_acknowledged:true})=>intent.meeting_revision===state.party?.meeting_revision?send('rally',auth.session.read().userId??'',intent.state,intent.physical_presence_acknowledged):Promise.resolve(),
  dispose(){if(disposed)return;scrub();disposed=true;cleanup();stop();stopAuthority?.();listeners.clear()}}
}
export type PartyManagementController=ReturnType<typeof createPartyManagement>
