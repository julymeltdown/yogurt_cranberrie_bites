import {z} from 'zod'
import {validCapabilities} from '../../../app/capabilities'
import type {SessionScope,Decoder} from '../../../shared/api/http'
import type {PrivacyScope} from '../../../shared/query/privacy'
import type {GiftAuth,GiftContextPort} from '../../gift-discovery/model/controller'
import type {Giveaway,Reservation} from '../../gift-discovery/model/contract'
import {contractValue,utc,uuid} from '../../party-authoring/model/contract'
export {type Giveaway,type Reservation}
export const sameScope=(a:SessionScope,b:SessionScope)=>a.userId===b.userId&&a.eventId===b.eventId&&a.generation===b.generation
export const samePrivacy=(a:PrivacyScope,b:PrivacyScope)=>sameScope(a,b)&&a.policyGeneration===b.policyGeneration&&a.gate===b.gate
export function giftAccess(auth:GiftAuth,context:GiftContextPort,kind:'read'|'normal'|'safety'='read'){
 const s=auth.privacy.capture(),c=context.read();if(!auth.privacy.isCurrent(s)||!uuid(s.userId)||!uuid(s.eventId)||!c||!samePrivacy(c.scope,s)||!validCapabilities(c.capabilities)||!c.capabilities.gifts.available)return false
 return kind==='read'||['OPEN','LIVE'].includes(c.eventPhase)&&c.capabilities.mutations.available||kind==='safety'&&c.eventPhase==='READ_ONLY'&&!c.capabilities.mutations.available&&c.capabilities.mutations.reason==='EVENT_READ_ONLY'
}
export function contextual<T>(name:string,path:string,scope:SessionScope,check:(value:T)=>void):Decoder<T>{return(v,c)=>{if(c.path!==path||c.status!==200||!sameScope(c.scope,scope))throw new Error('Invalid context');const value=contractValue<T>(name,v);check(value);return value}}
export function giftContext(g:Giveaway,s:SessionScope,id?:string){if(g.event_id!==s.eventId||id&&g.giveaway_id!==id||g.available>g.quantity_total||Date.parse(g.pickup_starts_at)>=Date.parse(g.pickup_ends_at))throw new Error('Invalid giveaway context')}
export function reservationContext(r:Reservation,s:SessionScope,giftId?:string,id?:string){if(r.event_id!==s.eventId||giftId&&r.giveaway_id!==giftId||id&&r.reservation_id!==id||Date.parse(r.created_at)>=Date.parse(r.expires_at)||r.state==='CLAIMED'&&!r.claim_receipt_id||r.state!=='CLAIMED'&&r.claim_receipt_id!==null)throw new Error('Invalid reservation context')}
const text=(max:number,min=0)=>z.string().refine(v=>Array.from(v).length>=min&&Array.from(v).length<=max,`Use ${min}–${max} characters.`)
export const giftDraftSchema=z.strictObject({title:text(120,1).refine(v=>v.trim().length>0,'Enter a title.'),description:text(2000),quantity_total:z.number().int().min(1).max(10000),pickup_starts_at:z.string().refine(utc,'Enter a valid UTC time.'),pickup_ends_at:z.string().refine(utc,'Enter a valid UTC time.'),public_place_id:z.uuid(),state:z.enum(['DRAFT','OPEN','PAUSED','CLOSED','CANCELLED'])}).refine(v=>Date.parse(v.pickup_starts_at)<Date.parse(v.pickup_ends_at),{path:['pickup_ends_at'],message:'End time must be after start time.'})
export type GiftDraft=z.infer<typeof giftDraftSchema>
export const emptyGiftDraft=():GiftDraft=>({title:'',description:'',quantity_total:1,pickup_starts_at:'',pickup_ends_at:'',public_place_id:'',state:'DRAFT'})
export interface Management {giveaway:Giveaway;reserved_unclaimed:number;quantity_claimed:number;reservations:readonly Reservation[];next_cursor:string|null}
export interface ReadyAttachment {readonly media_id:string;readonly state:'READY';readonly revision:number;readonly width:number;readonly height:number;readonly content_type:'image/jpeg'|'image/png'|'image/webp';readonly purpose:'POST'|'GIVEAWAY'|'REPORT';readonly read_path:string}
/** Structural subset of the media owner's published FE04 port, not a second upload implementation. */
export interface QualifiedImagePort {read():{readonly phase:string;readonly scope:SessionScope|null;readonly attachment:ReadyAttachment|null;readonly previewUrl:string|null;readonly error:string|null};subscribe(listener:()=>void):()=>void;select(file:File):Promise<void>;upload():Promise<void>;recover():Promise<void>;cancel():void}
export interface PickupMeeting {place_id:string;map_revision:number;place_revision:number;instructions:string;identification:string;expires_at:string}
export interface QualifiedPickupMeeting {scope:PrivacyScope;giveawayId:string;meeting:PickupMeeting;expiresAt:number}
