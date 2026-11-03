import type {HttpRequest} from '../../../shared/api/http'
import type {PrivacyScope} from '../../../shared/query/privacy'
import {TransportError} from '../../../shared/api/errors'
import type {GiftAuth,GiftContextPort} from '../../gift-discovery/model/controller'
import {contextual,giftAccess,giftContext,reservationContext,samePrivacy,type Giveaway,type Reservation,type PickupMeeting} from '../../giveaway-authoring/model/contract'
import {contractValue,receiptDecoder,uuid,type Binding,type Receipt} from '../../party-authoring/model/contract'
import {createSessionIntentJournal,validateIntent,type IntentJournal,type PickupIntent} from './journal'
interface PickupDetail {reservation:Reservation;meeting:PickupMeeting}
interface PickupCode {reservation_id:string;confirmation_code:string;expires_at:string;reservation_revision:number}
interface PickupReceipt {receipt_id:string;reservation_id:string;giveaway_id:string;state:'CLAIMED';revision:number;claimed_at:string}
export interface PickupPlaceAuthority {scope:PrivacyScope;placeId:string;placeRevision:number;mapRevision:number;expiresAt:number}
export interface GiftPickupOptions {auth:GiftAuth;context:GiftContextPort;giveawayId?:string;reservationId?:string;journal?:IntentJournal;now?:()=>number|null;newKey?:()=>string;placeAuthority?:{read():PickupPlaceAuthority|null;subscribe(f:()=>void):()=>void}}
type Phase='idle'|'loading'|'ready'|'sending'|'pending'|'uncertain'|'checking'|'succeeded'|'failed'|'denied'|'storage-unavailable'|'code-unavailable'
interface State {readonly epoch:number;readonly phase:Phase;readonly gift:Giveaway|null;readonly reservation:Reservation|null;readonly pickup:PickupDetail|null;readonly code:PickupCode|null;readonly receipt:PickupReceipt|null;readonly confirmationCode:string;readonly cancelConfirmed:boolean;readonly message:string;readonly retryAt:number;readonly access:boolean}
export function createGiftPickup(o:GiftPickupOptions){const{auth,context}=o,now=o.now??Date.now,listeners=new Set<()=>void>()
 let state:State=Object.freeze({epoch:0,phase:'idle',gift:null,reservation:null,pickup:null,code:null,receipt:null,confirmationCode:'',cancelConfirmed:false,message:'',retryAt:0,access:giftAccess(auth,context)}),disposed=false,flight:AbortController|null=null,intent:PickupIntent|null=null,binding:Binding|null=null,journal:IntentJournal|null=null,expiry:ReturnType<typeof setTimeout>|undefined,journalHealthy=true
 try{journal=o.journal??createSessionIntentJournal()}catch{/* Failing closed is required before any mutation. */}
 const emit=(p:Partial<State>)=>{if(disposed)return;state=Object.freeze({...state,...p});for(const f of Array.from(listeners)){try{f()}catch{}}}
 const base=(s:PrivacyScope)=>`/api/concert/events/${s.eventId}`
 const live=(s:PrivacyScope,c:AbortController)=>!disposed&&flight===c&&!c.signal.aborted&&auth.privacy.isCurrent(s)&&giftAccess(auth,context)
 const busy=()=>!['ready','failed','succeeded','code-unavailable'].includes(state.phase)
 function clearJournal(){const s=auth.session.read();if(s.userId&&s.eventId)journal?.clear(s.userId,s.eventId)}
 function scrub(previous?:PrivacyScope,retainJournal=false){clearTimeout(expiry);flight?.abort();flight=null;intent=null;binding=null;try{const s=previous??auth.privacy.capture();if(!retainJournal&&s.userId&&s.eventId)journal?.clear(s.userId,s.eventId)}catch{}emit({epoch:state.epoch+1,phase:'idle',gift:null,reservation:null,pickup:null,code:null,receipt:null,confirmationCode:'',cancelConfirmed:false,message:'',retryAt:0,access:giftAccess(auth,context)})}
 const cleanup=auth.privacy.registerCleanup(change=>scrub(change.previous)),stop=context.subscribe(()=>{if(!giftAccess(auth,context))scrub();else emit({access:true})}),stopPlace=o.placeAuthority?.subscribe(()=>{if(!physicalReady())emit({pickup:null,code:null});armExpiry()})
 async function request<T>(s:PrivacyScope,c:AbortController,input:Omit<HttpRequest<T>,'signal'>){return auth.privacy.run(s,async({signal})=>(await auth.request({...input,signal})).data,c.signal)}
 function reservationCheck(r:Reservation,s:PrivacyScope,id?:string){reservationContext(r,s,state.gift?.giveaway_id??intent?.giveawayId??o.giveawayId,id);if(r.recipient_id!==s.userId&&state.gift?.owner.user_id!==s.userId)throw new Error('Unauthorized reservation projection')}
 function currentTime(){const value=now();return typeof value==='number'&&Number.isFinite(value)?value:null}
 function active(r=state.reservation){const n=currentTime();return !!r&&n!==null&&(!state.gift||['OPEN','PAUSED'].includes(state.gift.state))&&['HELD','READY'].includes(r.state)&&Date.parse(r.expires_at)>n}
 function qualifiedPickup(p:PickupDetail|null){const a=o.placeAuthority?.read(),s=auth.privacy.capture(),n=currentTime();return !!p&&!!a&&n!==null&&samePrivacy(a.scope,s)&&a.placeId===p.meeting.place_id&&a.placeRevision===p.meeting.place_revision&&a.mapRevision===p.meeting.map_revision&&a.expiresAt>n&&Date.parse(p.meeting.expires_at)>n&&active(p.reservation)}
 function physicalReady(){return qualifiedPickup(state.pickup)}
 function armExpiry(){clearTimeout(expiry);const n=currentTime();if(n===null){emit({pickup:null,code:null});return}
  const times=[state.retryAt,state.reservation&&Date.parse(state.reservation.expires_at),state.pickup&&Date.parse(state.pickup.meeting.expires_at),state.code&&Date.parse(state.code.expires_at),o.placeAuthority?.read()?.expiresAt].filter((v):v is number=>typeof v==='number'&&v>0)
  const due=times.filter(t=>t>n);if(!active())emit({pickup:null,code:null});else if(!physicalReady())emit({pickup:null,code:null});else if(state.code&&Date.parse(state.code.expires_at)<=n)emit({code:null,message:'The pickup code expired. It does not confirm collection.'})
  emit({});if(due.length)expiry=setTimeout(armExpiry,Math.min(Math.max(1,Math.min(...due)-n),2147483647))
 }
 function failure(e:unknown,sent=false){const t=e instanceof TransportError?e:null,r=t?.server?.reason,unknown=sent&&(!t||t.uncertain||!t.status||t.status>=500||t.status===404||r==='IDEMPOTENCY_CONFLICT'||r==='IDEMPOTENCY_EXPIRED')
  if((t?.status===401||t?.status===403)&&r!=='PICKUP_CODE_INVALID'){scrub();emit({phase:'denied',message:'Pickup access is unavailable for this account.'});return}
  if(!unknown){try{clearJournal()}catch{}intent=null;binding=null}
  emit({phase:unknown?'uncertain':'failed',pickup:null,code:null,confirmationCode:'',cancelConfirmed:false,retryAt:t?.status===429?(currentTime()??Date.now())+Math.max(1000,t.retryAfterMs??1000):0,message:unknown?'Outcome unknown. Check the original command. Reload will preserve its identity, not repeat it.':r==='GIVEAWAY_SOLD_OUT'?'This giveaway is sold out. No reservation was confirmed.':r==='RESERVATION_EXPIRED'?'This reservation expired. No pickup was confirmed.':r==='PICKUP_CODE_INVALID'?'The pickup code was rejected. No collection was confirmed.':r==='DUPLICATE_ACTIVE_RESERVATION'?'You already have an active reservation. Read your current reservation.':r==='VERSION_CONFLICT'?'This reservation or stock changed. Read current information again.':'The request failed. No collection is confirmed.'});armExpiry()
 }
 async function readGift(s:PrivacyScope,c:AbortController,id:string){const path=`${base(s)}/giveaways/${id}`;return request(s,c,{path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'getConcertGiveaway',decode:contextual<Giveaway>('Domain_giveaway_GiveawayPublic',path,s,g=>{giftContext(g,s,id);if(g.state==='DRAFT'&&g.owner.user_id!==s.userId)throw new Error('Private draft')})})}
 async function readReservation(s:PrivacyScope,c:AbortController,id:string){const path=`${base(s)}/reservations/${id}`;return request(s,c,{path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'getConcertReservation',decode:contextual<Reservation>('Domain_giveaway_Reservation',path,s,r=>reservationCheck(r,s,id))})}
 function restore(){if(!giftAccess(auth,context))return;const s=auth.privacy.capture();try{if(!journal)throw new Error('Missing journal');const saved=journal.read(s.userId!,s.eventId!);if(saved){const record=validateIntent(saved);if(record.actor!==s.userId||record.event!==s.eventId)throw new Error('Wrong journal actor');intent=record;binding={scope:s,operation:record.operation,key:record.key,target:record.reservationId};emit({phase:'uncertain',message:'An unfinished command was restored. Check its original status before any new action.'})}}catch{journalHealthy=false;emit({phase:'storage-unavailable',message:'Recovery storage is unavailable or invalid. New commands are disabled.'})}}
 restore()
 async function load(){if(!intent)restore();if(!giftAccess(auth,context)||['loading','sending','checking'].includes(state.phase)||(currentTime()??0)<state.retryAt)return
  const s=auth.privacy.capture(),c=new AbortController();flight?.abort();flight=c;const unresolved=!!intent;emit({phase:'loading',pickup:null,code:null,confirmationCode:'',message:unresolved?state.message:''})
  try{const giftId=intent?.giveawayId??o.giveawayId??state.gift?.giveaway_id,reservationId=intent?.reservationId??o.reservationId??state.reservation?.reservation_id
   // Recipient's reservation endpoint authorizes itself; validate event/target before resolving its gift.
   let r:Reservation|null=null,g:Giveaway|null=null
   if(giftId){if(!uuid(giftId))throw new Error('Invalid ID');g=await readGift(s,c,giftId);if(live(s,c))emit({gift:g})}
   if(reservationId){if(!uuid(reservationId))throw new Error('Invalid ID');if(!g){const path=`${base(s)}/reservations/${reservationId}`;r=await request(s,c,{path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'getConcertReservation',decode:contextual<Reservation>('Domain_giveaway_Reservation',path,s,v=>{reservationContext(v,s,undefined,reservationId);if(v.recipient_id!==s.userId)throw new Error('Owner must enter through giveaway-bound route')})});g=await readGift(s,c,r.giveaway_id);if(live(s,c))emit({gift:g})}else r=await readReservation(s,c,reservationId)}
   if(live(s,c)){emit({gift:g,reservation:r,phase:unresolved?'uncertain':journal&&journalHealthy?'ready':'storage-unavailable',message:unresolved?'Original command still needs status recovery.':''});armExpiry()}
  }catch(e){if(live(s,c))failure(e,unresolved)}
 }
 async function loadPickup(){const r=state.reservation;if(!r||!giftAccess(auth,context)||(currentTime()??0)<state.retryAt||busy()||!active()||r.recipient_id!==auth.session.read().userId)return
  const s=auth.privacy.capture(),c=new AbortController();flight?.abort();flight=c;const path=`${base(s)}/reservations/${r.reservation_id}/pickup`;emit({phase:'loading',pickup:null,code:null})
  try{const p=await request(s,c,{path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'getConcertPickup',decode:contextual<PickupDetail>('Domain_giveaway_PickupDetails',path,s,v=>{reservationCheck(v.reservation,s,r.reservation_id);if(v.reservation.recipient_id!==s.userId||!active(v.reservation))throw new Error('Unavailable pickup')})})
   if(live(s,c)){const qualified=qualifiedPickup(p);emit({reservation:p.reservation,pickup:qualified?p:null,phase:'ready',message:qualified?'':'Current reviewed place/map authority is unavailable; private pickup guidance is hidden.'});armExpiry()}
  }catch(e){if(live(s,c))failure(e)}
 }
 async function finishReceipt(r:Receipt,s:PrivacyScope,c:AbortController){const i=intent!;const n=currentTime()
  if(r.state==='PENDING'){emit({phase:'pending',message:'The command is still pending. Check again explicitly.'});return}
  if(r.state==='RETIRED'||n===null||Date.parse(r.expires_at)<=n){emit({phase:'uncertain',message:'The receipt is retired or expired. This does not prove the command did not execute.'});return}
  if(r.state==='FAILED'){clearJournal();intent=null;binding=null;emit({phase:'failed',message:'The original command failed. Read current state before another intent.'});return}
  const id=i.reservationId??r.resource_id!;const reservation=await readReservation(s,c,id);if(!live(s,c))return
  if(i.operation!=='reserveConcertGiveaway'&&r.resource_revision!<=i.expectedRevision||reservation.revision<r.resource_revision!||i.operation==='reserveConcertGiveaway'&&reservation.recipient_id!==s.userId||i.operation==='cancelConcertReservation'&&reservation.state!=='CANCELLED'||i.operation==='claimConcertReservation'&&(reservation.state!=='CLAIMED'||!reservation.claim_receipt_id))throw new Error('Receipt state not reconciled')
  clearJournal();intent=null;binding=null
  if(i.operation==='reserveConcertGiveaway'&&!active(reservation)){emit({reservation,pickup:null,code:null,phase:'ready',message:`The reservation is now ${reservation.state.toLowerCase()} or expired; pickup is unavailable.`});armExpiry();return}
  emit({reservation,pickup:null,code:null,confirmationCode:'',phase:i.operation==='issueConcertPickupCode'?'code-unavailable':'succeeded',message:i.operation==='issueConcertPickupCode'?'Code issuance was confirmed, but plaintext cannot be recovered. Read current pickup details, then explicitly request a new code.':i.operation==='claimConcertReservation'?'The server confirmed one-time collection.':'The original command was confirmed by current authorized state.'});armExpiry()
 }
 async function send(operation:PickupIntent['operation']){if(!intent)restore();if(!giftAccess(auth,context,operation==='cancelConcertReservation'?'safety':'normal')||busy()||intent||(currentTime()??0)<state.retryAt||!state.gift)return
  const g=state.gift,r=state.reservation,s=auth.privacy.capture(),n=currentTime();if(n===null)return
  if(operation==='reserveConcertGiveaway'&&(g.owner.user_id===s.userId||g.state!=='OPEN'||g.available<=0||Date.parse(g.pickup_ends_at)<=n||!!r))return
  if(operation!=='reserveConcertGiveaway'&&(!r||!active(r)))return
  if(operation==='cancelConcertReservation'&&!state.cancelConfirmed)return
  if(operation==='issueConcertPickupCode'&&(r!.recipient_id!==s.userId||r!.state!=='READY'||!physicalReady()))return
  if(operation==='claimConcertReservation'&&(g.owner.user_id!==s.userId||r!.state!=='READY'||!/^\d{6}$/.test(state.confirmationCode)))return
  const key=(o.newKey??(()=>crypto.randomUUID()))();if(!uuid(key))return
  const i:PickupIntent={version:1,actor:s.userId!,event:s.eventId!,operation,key,giveawayId:g.giveaway_id,reservationId:operation==='reserveConcertGiveaway'?null:r!.reservation_id,expectedRevision:operation==='reserveConcertGiveaway'?g.revision:r!.revision}
  try{if(!journal||!journalHealthy)throw new Error('Missing journal');journal.write(i)}catch{journalHealthy=false;emit({phase:'storage-unavailable',message:'Cannot preserve command identity for reload recovery. No command was sent.'});return}
  intent=i;binding={scope:s,operation,key,target:i.reservationId};const b=binding,c=new AbortController();flight?.abort();flight=c
  const path=operation==='reserveConcertGiveaway'?`${base(s)}/giveaways/${g.giveaway_id}/reservations`:`${base(s)}/reservations/${r!.reservation_id}${operation==='claimConcertReservation'?'/claim':operation==='issueConcertPickupCode'?'/pickup-code':''}`
  const body=operation==='reserveConcertGiveaway'?{giveaway_revision:g.revision}:operation==='claimConcertReservation'?{expected_revision:r!.revision,confirmation_code:state.confirmationCode}:{expected_revision:r!.revision}
  contractValue(operation==='reserveConcertGiveaway'?'Domain_giveaway_Reserve':operation==='claimConcertReservation'?'Domain_giveaway_ClaimReservation':operation==='issueConcertPickupCode'?'Domain_giveaway_IssuePickupCode':'Domain_common_RevisionRequest',body)
  emit({phase:'sending',code:null,confirmationCode:'',cancelConfirmed:false,message:''})
  try{const result=await request<Reservation|PickupCode|PickupReceipt|Receipt>(s,c,{path,method:operation==='cancelConcertReservation'?'DELETE':'POST',access:'protected',errorFamily:'domain',domainOperation:operation,idempotencyKey:key,...operation==='cancelConcertReservation'?{query:{expected_revision:r!.revision}}:{body:{json:body}},decode:(v,ctx)=>{
   if(ctx.status===202)return receiptDecoder(b,path)(v,ctx)
   if(operation==='issueConcertPickupCode')return contextual<PickupCode>('Domain_giveaway_PickupCode',path,s,value=>{if(value.reservation_id!==r!.reservation_id||value.reservation_revision<=r!.revision||Date.parse(value.expires_at)<=n||Date.parse(value.expires_at)>Math.min(n+300000,Date.parse(r!.expires_at)))throw new Error('Invalid code context')})(v,ctx)
   if(operation==='claimConcertReservation')return contextual<PickupReceipt>('Domain_giveaway_PickupReceipt',path,s,value=>{if(value.reservation_id!==r!.reservation_id||value.giveaway_id!==g.giveaway_id||value.revision<=r!.revision)throw new Error('Invalid pickup receipt')})(v,ctx)
   return contextual<Reservation>('Domain_giveaway_Reservation',path,s,value=>{reservationCheck(value,s,i.reservationId??undefined);if(operation==='reserveConcertGiveaway'&&(value.recipient_id!==s.userId||!['HELD','READY'].includes(value.state)||!active(value))||operation==='cancelConcertReservation'&&(value.state!=='CANCELLED'||value.revision<=r!.revision))throw new Error('Invalid reservation result')})(v,ctx)
  }})
  if(!live(s,c))return
  if('receipt_id'in result&&'operation_id'in result){await finishReceipt(result as Receipt,s,c);return}
  if('confirmation_code'in result){const code=result as PickupCode;const latest=await readReservation(s,c,r!.reservation_id);if(!live(s,c))return;if(latest.revision!==code.reservation_revision||latest.state!=='READY')throw new Error('Stale issued code');clearJournal();intent=null;binding=null;emit({phase:'succeeded',reservation:latest,code,message:'Show this short-lived code only at the current pickup. It does not confirm collection.'});armExpiry();return}
  if('claimed_at'in result){const receipt=result as PickupReceipt,latest=await readReservation(s,c,r!.reservation_id);if(!live(s,c))return;if(latest.state!=='CLAIMED'||latest.claim_receipt_id!==receipt.receipt_id||latest.revision<receipt.revision)throw new Error('Unconfirmed claim');clearJournal();intent=null;binding=null;emit({phase:'succeeded',reservation:latest,receipt,pickup:null,code:null,message:'The server confirmed one-time collection.'});armExpiry();return}
  const reservation=result as Reservation;clearJournal();intent=null;binding=null;emit({phase:'succeeded',reservation,pickup:null,code:null,message:operation==='cancelConcertReservation'?'Reservation cancellation confirmed.':'Your reservation is confirmed; collection is still pending.'});armExpiry()
  }catch(e){if(live(s,c))failure(e,true)}
 }
 async function checkStatus(){const b=binding;if(!b||!intent||!giftAccess(auth,context)||!auth.privacy.isCurrent(b.scope)||!['pending','uncertain'].includes(state.phase)||(currentTime()??0)<state.retryAt)return
  const c=new AbortController();flight?.abort();flight=c;const path=`${base(b.scope)}/commands`;emit({phase:'checking',confirmationCode:''})
  try{const result=await request(b.scope,c,{path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'getConcertCommandStatus',query:{operation_id:b.operation,idempotency_key:b.key},decode:receiptDecoder(b,path)});if(live(b.scope,c))await finishReceipt(result,b.scope,c)}catch(e){if(live(b.scope,c))failure(e,true)}
 }
 return{read:()=>state,subscribe(f:()=>void){listeners.add(f);return()=>{listeners.delete(f)}},load,loadPickup,checkStatus,physicalReady,active,retryReady:()=>currentTime()!==null&&currentTime()!>=state.retryAt,canCheckStatus:()=>['pending','uncertain'].includes(state.phase)&&currentTime()!==null&&currentTime()!>=state.retryAt,canWrite:()=>giftAccess(auth,context,'normal'),isOwner:()=>state.gift?.owner.user_id===auth.session.read().userId,isRecipient:()=>state.reservation?.recipient_id===auth.session.read().userId,
  reserve:()=>send('reserveConcertGiveaway'),cancel:()=>send('cancelConcertReservation'),issueCode:()=>send('issueConcertPickupCode'),claim:()=>send('claimConcertReservation'),
  setClaimCode(code:string){if(!busy()&&!intent)emit({confirmationCode:code.slice(0,6)})},confirmCancel(value:boolean){if(!busy()&&!intent)emit({cancelConfirmed:value})},
  dispose(){if(disposed)return;scrub(undefined,true);disposed=true;cleanup();stop();stopPlace?.();listeners.clear()}}
}
export type GiftPickupController=ReturnType<typeof createGiftPickup>
