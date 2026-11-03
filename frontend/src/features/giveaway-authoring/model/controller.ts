import type {HttpRequest} from '../../../shared/api/http'
import {TransportError} from '../../../shared/api/errors'
import type {PrivacyScope} from '../../../shared/query/privacy'
import type {GiftAuth,GiftContextPort} from '../../gift-discovery/model/controller'
import type {QualifiedPlace} from '../../party-authoring/model/controller'
import {contractValue,receiptDecoder,uuid,type Binding,type Receipt} from '../../party-authoring/model/contract'
import {contextual,emptyGiftDraft,giftAccess,giftContext,giftDraftSchema,reservationContext,sameScope,samePrivacy,type GiftDraft,type Giveaway,type Management,type QualifiedImagePort,type QualifiedPickupMeeting} from './contract'
export interface GiveawayAuthoringOptions {auth:GiftAuth;context:GiftContextPort;places:()=>readonly QualifiedPlace[];giveawayId?:string;image?:QualifiedImagePort;meeting?:{read():QualifiedPickupMeeting|null;subscribe(listener:()=>void):()=>void};now?:()=>number;newKey?:()=>string}
type Phase='idle'|'loading'|'sending'|'pending'|'uncertain'|'checking'|'succeeded'|'failed'|'conflict'|'denied'
interface State {readonly epoch:number;readonly phase:Phase;readonly draft:GiftDraft;readonly gift:Giveaway|null;readonly latest:Giveaway|null;readonly management:Management|null;readonly saved:GiftDraft|null;readonly imageChoice:'keep'|'none'|'upload';readonly message:string;readonly retryAt:number;readonly confirmCancel:boolean;readonly access:boolean}
export function createGiveawayAuthoring(o:GiveawayAuthoringOptions){const{auth,context,giveawayId}=o,now=o.now??Date.now,listeners=new Set<()=>void>()
 let state:State=Object.freeze({epoch:0,phase:'idle',draft:emptyGiftDraft(),gift:null,latest:null,management:null,saved:null,imageChoice:'keep',message:'',retryAt:0,confirmCancel:false,access:giftAccess(auth,context)}),disposed=false,flight:AbortController|null=null,binding:Binding|null=null,action:'save'|'meeting'='save'
 const emit=(patch:Partial<State>)=>{if(disposed)return;state=Object.freeze({...state,...patch});for(const f of Array.from(listeners)){try{f()}catch{}}}
 const live=(s:PrivacyScope,c:AbortController)=>!disposed&&flight===c&&!c.signal.aborted&&auth.privacy.isCurrent(s)&&giftAccess(auth,context)
 const busy=()=>!['idle','failed','conflict'].includes(state.phase)
 const base=(s:PrivacyScope)=>`/api/concert/events/${s.eventId}`
 function scrub(){flight?.abort();flight=null;binding=null;o.image?.cancel();emit({epoch:state.epoch+1,phase:'idle',draft:emptyGiftDraft(),gift:null,latest:null,management:null,saved:null,imageChoice:'keep',message:'',retryAt:0,confirmCancel:false,access:giftAccess(auth,context)})}
 const cleanup=auth.privacy.registerCleanup(scrub),stop=context.subscribe(()=>{if(!giftAccess(auth,context))scrub();else emit({access:true})}),stopImage=o.image?.subscribe(()=>emit({})),stopMeeting=o.meeting?.subscribe(()=>emit({}))
 function own(g:Giveaway){if(g.owner.user_id!==auth.session.read().userId)throw new TransportError('http',{status:403})}
 async function request<T>(s:PrivacyScope,c:AbortController,input:Omit<HttpRequest<T>,'signal'>){return auth.privacy.run(s,async({signal})=>(await auth.request({...input,signal})).data,c.signal)}
 function from(g:Giveaway):GiftDraft{return{title:g.title,description:g.description,quantity_total:g.quantity_total,pickup_starts_at:g.pickup_starts_at,pickup_ends_at:g.pickup_ends_at,public_place_id:g.public_place_id,state:g.state}}
 function fail(e:unknown,sent=false){const t=e instanceof TransportError?e:null,r=t?.server?.reason,uncertain=sent&&(!t||t.uncertain||!t.status||t.status>=500||t.status===404||r==='IDEMPOTENCY_CONFLICT'||r==='IDEMPOTENCY_EXPIRED')
  if(t?.status===401||t?.status===403){scrub();emit({phase:'denied',message:'Owner tools are unavailable for this account.'});return}
  emit({phase:uncertain?'uncertain':r==='VERSION_CONFLICT'?'conflict':'failed',confirmCancel:false,retryAt:t?.status===429?now()+Math.max(1000,t.retryAfterMs??1000):0,message:uncertain?'Outcome unknown. Check the original command; do not submit another.':r==='VERSION_CONFLICT'?'Stock or settings changed. Your form is preserved; read the latest giveaway.':r==='MEDIA_NOT_READY'?'The image is not ready. Keep your draft and check the qualified upload.':'The request failed. Your form is preserved.'})
 }
 async function getManagement(s:PrivacyScope,c:AbortController,id:string,cursor?:string){const path=`${base(s)}/giveaways/${id}/management`;return request(s,c,{path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'getConcertGiveawayManagement',query:{limit:20,...cursor?{cursor}:{}},decode:contextual<Management>('Domain_giveaway_GiveawayManagement',path,s,m=>{giftContext(m.giveaway,s,id);own(m.giveaway);if(m.reservations.length>20||new Set(m.reservations.map(r=>r.reservation_id)).size!==m.reservations.length||m.giveaway.quantity_total!==m.giveaway.available+m.reserved_unclaimed+m.quantity_claimed)throw new Error('Invalid stock');m.reservations.forEach(r=>reservationContext(r,s,id));if(m.reservations.filter(r=>r.state==='HELD'||r.state==='READY').length>m.reserved_unclaimed||m.reservations.filter(r=>r.state==='CLAIMED').length>m.quantity_claimed)throw new Error('Inconsistent stock page')})})}
 async function loadLatest(){const id=giveawayId??state.gift?.giveaway_id;if(!id||!uuid(id)||!giftAccess(auth,context)||!['idle','failed','conflict','succeeded'].includes(state.phase)||state.retryAt>now())return
  const s=auth.privacy.capture(),c=new AbortController();flight?.abort();flight=c;const preserve=!!state.gift;emit({phase:'loading',message:''})
  try{const m=await getManagement(s,c,id);if(live(s,c))emit({phase:preserve?'conflict':'idle',management:m,gift:preserve?state.gift:m.giveaway,latest:preserve?m.giveaway:null,...preserve?{}:{draft:from(m.giveaway)},message:preserve?'Current stock loaded. Explicitly keep your draft with the new revision.':''})}catch(e){if(live(s,c))fail(e)}
 }
 async function moreReservations(){const m=state.management;if(!m?.next_cursor||!giftAccess(auth,context)||busy())return
  const s=auth.privacy.capture(),c=new AbortController();flight?.abort();flight=c;emit({phase:'loading'})
  try{const next=await getManagement(s,c,m.giveaway.giveaway_id,m.next_cursor);if(next.giveaway.revision!==m.giveaway.revision||next.next_cursor===m.next_cursor)throw new Error('Changed continuation');const reservations=[...m.reservations,...next.reservations];if(new Set(reservations.map(r=>r.reservation_id)).size!==reservations.length)throw new Error('Duplicate reservation');if(live(s,c))emit({management:{...next,reservations},phase:'idle'})}catch(e){if(live(s,c))fail(e)}
 }
 function attachment(){const u=o.image?.read(),s=auth.privacy.capture();if(!u||u.phase!=='ready'||!u.scope||!sameScope(u.scope,s)||!u.attachment)return null
  try{const a=contractValue<import('./contract').ReadyAttachment>('Domain_feed_Media',u.attachment);return a.purpose==='GIVEAWAY'&&a.read_path===`/api/concert/events/${s.eventId}/media/${a.media_id}/content`?a:null}catch{return null}
 }
 function meeting(){const m=o.meeting?.read(),s=auth.privacy.capture();return m&&samePrivacy(m.scope,s)&&m.giveawayId===(state.gift?.giveaway_id??giveawayId)&&m.expiresAt>now()&&Date.parse(m.meeting.expires_at)>now()?m:null}
 async function receiptResult(r:Receipt,s:PrivacyScope,c:AbortController){
  if(r.state==='PENDING'){emit({phase:'pending',message:'The command is pending. Check its status.'});return}
  if(r.state==='RETIRED'||Date.parse(r.expires_at)<=now()){emit({phase:'uncertain',message:'Receipt unavailable or expired; the write may have executed.'});return}
  if(r.state==='FAILED'){emit({phase:r.failure_reason==='VERSION_CONFLICT'?'conflict':'failed',message:'The command failed. Your draft is preserved.'});return}
  const m=await getManagement(s,c,r.resource_id!);if(!live(s,c))return
  if(m.giveaway.revision<r.resource_revision!)throw new Error('Stale stock result')
  emit({phase:'succeeded',management:m,gift:m.giveaway,saved:null,message:action==='meeting'?'The server confirmed pickup instructions.':'The server confirmed your giveaway.'})
 }
 async function send(draft:GiftDraft,kind:'save'|'meeting'='save',ack=false){if(!giftAccess(auth,context,'normal')||busy()||state.phase==='conflict'||state.retryAt>now()||giveawayId&&!state.gift)return
  const parsed=giftDraftSchema.safeParse(draft);if(!parsed.success||!o.places().some(p=>p.id===draft.public_place_id)||Date.parse(draft.pickup_ends_at)<=now()){emit({message:'Check the title, quantity, future time window and qualified place.'});return}
  const g=state.gift,m=state.management,photo=attachment(),qualifiedMeeting=meeting()
  if(kind==='meeting'&&(!ack||!g||!qualifiedMeeting))return
  if(kind==='save'&&g&&m&&(draft.quantity_total<m.reserved_unclaimed+m.quantity_claimed||m.reserved_unclaimed>0&&(draft.pickup_starts_at!==g.pickup_starts_at||draft.pickup_ends_at!==g.pickup_ends_at||draft.public_place_id!==g.public_place_id))){emit({message:'Reserved or claimed stock cannot be removed, and outstanding pickup windows/places must stay consistent.'});return}
  if(kind==='save'&&draft.state==='CANCELLED'&&!state.confirmCancel){emit({message:'Confirm cancellation before saving.'});return}
  if(kind==='save'&&state.imageChoice==='upload'&&!photo){emit({message:'A qualified READY giveaway image is required for the selected upload.'});return}
  const s=auth.privacy.capture(),key=(o.newKey??(()=>crypto.randomUUID()))();if(!uuid(key))return
  const id=g?.giveaway_id??giveawayId,operation=kind==='meeting'?'setConcertPickupMeeting':id?'updateConcertGiveaway':'createConcertGiveaway',path=`${base(s)}/giveaways${id?`/${id}`:''}${kind==='meeting'?'/pickup-meeting':''}`
  const{state:lifecycle,...fields}=parsed.data,json=kind==='meeting'?{expected_revision:g!.revision,meeting:{...qualifiedMeeting!.meeting},physical_presence_acknowledged:true}:{...fields,image_media_id:state.imageChoice==='upload'?photo!.media_id:state.imageChoice==='none'?null:g?.image_media_id??null,...id?{expected_revision:g!.revision,state:lifecycle}:{}}
  contractValue(kind==='meeting'?'Domain_giveaway_SetPickupMeeting':id?'Domain_giveaway_UpdateGiveaway':'Domain_giveaway_CreateGiveaway',json)
  const b:Binding={scope:s,operation,key,target:id??null};binding=b;action=kind;const c=new AbortController();flight?.abort();flight=c;emit({phase:'sending',draft:{...parsed.data},message:'',confirmCancel:false})
  try{const result=await request<Giveaway|Receipt|{giveaway_id:string;revision:number;meeting:unknown}>(s,c,{path,method:kind==='meeting'?'PUT':id?'PATCH':'POST',access:'protected',errorFamily:'domain',domainOperation:operation,idempotencyKey:key,body:{json},decode:(v,ctx)=>{if(ctx.status===202)return receiptDecoder(b,path)(v,ctx)
   if(kind==='meeting')return contextual<{giveaway_id:string;revision:number;meeting:unknown}>('Domain_giveaway_PickupMeetingState',path,s,value=>{if(value.giveaway_id!==id||value.revision<=g!.revision)throw new Error('Invalid pickup meeting result')})(v,ctx)
   return contextual<Giveaway>('Domain_giveaway_GiveawayPublic',path,s,value=>{giftContext(value,s,id);own(value);if(g&&value.revision<=g.revision||!id&&value.state!=='DRAFT')throw new Error('Invalid giveaway result')})(v,ctx)
  }})
   if(!live(s,c))return
   if('receipt_id'in result)await receiptResult(result,s,c)
   else{const resultId='giveaway_id'in result?result.giveaway_id:id!,management=await getManagement(s,c,resultId);if(management.giveaway.revision<result.revision)throw new Error('Stale post-command inventory');if(live(s,c))emit({phase:'succeeded',gift:management.giveaway,management,saved:null,message:kind==='meeting'?'The server confirmed pickup instructions.':'The server confirmed your giveaway.'})}
  }catch(e){if(live(s,c))fail(e,true)}
 }
 async function checkStatus(){const b=binding;if(!b||!giftAccess(auth,context)||!auth.privacy.isCurrent(b.scope)||!['pending','uncertain'].includes(state.phase)||state.retryAt>now())return
  const c=new AbortController();flight?.abort();flight=c;const path=`${base(b.scope)}/commands`;emit({phase:'checking'})
  try{const r=await request(b.scope,c,{path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'getConcertCommandStatus',query:{operation_id:b.operation,idempotency_key:b.key},decode:receiptDecoder(b,path)});if(live(b.scope,c))await receiptResult(r,b.scope,c)}catch(e){if(live(b.scope,c))fail(e,true)}
 }
 return{read:()=>state,subscribe(f:()=>void){listeners.add(f);return()=>{listeners.delete(f)}},loadLatest,moreReservations,checkStatus,submit:(d:GiftDraft)=>send(d),setMeeting:(ack:boolean)=>send(state.draft,'meeting',ack),meeting,
  image:()=>state.access?o.image?.read()??null:null,hasUpload:()=>!!o.image,selectImage:async(file:File)=>{if(!busy()&&giftAccess(auth,context,'normal')){emit({imageChoice:'upload'});await o.image?.select(file)}},uploadImage:()=>!busy()&&giftAccess(auth,context,'normal')?o.image?.upload():Promise.resolve(),recoverImage:()=>giftAccess(auth,context)?o.image?.recover():Promise.resolve(),removeImage(){if(!busy()){o.image?.cancel();emit({imageChoice:'none'})}},
  places:()=>state.access?o.places():[],edit(d:GiftDraft){if(!busy()&&giftAccess(auth,context))emit({draft:{...d}})},confirmCancel(value=true){if(!busy())emit({confirmCancel:value})},canWrite:()=>giftAccess(auth,context,'normal'),
  keepDraftWithLatest(){if(state.phase==='conflict'&&state.latest&&giftAccess(auth,context))emit({gift:state.latest,latest:null,phase:'idle',message:'Draft retained with the reviewed current revision.'})},
  continueEditing(){if(state.phase==='succeeded'&&state.gift)emit({phase:'idle',draft:from(state.gift),epoch:state.epoch+1,imageChoice:'keep'})},saveDraft(){if(!busy()&&giftAccess(auth,context))emit({saved:{...state.draft},message:'Draft saved for this active session.'})},restoreDraft(){if(!busy()&&state.saved&&giftAccess(auth,context))emit({draft:{...state.saved},epoch:state.epoch+1})},
  dispose(){if(disposed)return;scrub();disposed=true;cleanup();stop();stopImage?.();stopMeeting?.();listeners.clear()}}
}
export type GiveawayAuthoringController=ReturnType<typeof createGiveawayAuthoring>
