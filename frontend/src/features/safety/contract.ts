import { z } from 'zod'
import type { PrivacyScope } from '../../shared/query/privacy'
import { DOMAIN_OPERATIONS } from '../../shared/api/domain-operations.generated'
export const uuid = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
const revision = z.number().int().safe().min(1)
export const timestamp = z.iso.datetime()
export const targetTypes=['USER','POST','COMMENT','PARTY','GIVEAWAY','MURAL'] as const
export const reasons=['HARASSMENT','PRIVACY','UNSAFE_MEETING','SPAM','OTHER'] as const
export const targetSchema=z.strictObject({target_type:z.enum(targetTypes),target_id:uuid})
export type ReportTarget=z.infer<typeof targetSchema>
export const reportInputSchema=targetSchema.extend({reason:z.enum(reasons),text:z.string().refine(s=>Array.from(s).length<=2000&&!/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/u.test(s)),evidence_media_ids:z.array(uuid).max(4).refine(a=>new Set(a).size===a.length)})
export const reportSchema=targetSchema.extend({report_id:uuid,event_id:uuid,state:z.enum(['OPEN','IN_REVIEW','RESOLVED','DISMISSED']),revision,created_at:timestamp})
export type Report=z.infer<typeof reportSchema>
export const blockSchema=z.strictObject({target_user_id:uuid,blocked_by_me:z.boolean(),revision})
export const blockListSchema=z.strictObject({items:z.array(blockSchema).max(100),next_cursor:z.string().regex(/^[A-Za-z0-9_-]+$/).max(1024).nullable()})
export const reportListSchema=z.strictObject({items:z.array(reportSchema).max(100),next_cursor:z.string().regex(/^[A-Za-z0-9_-]+$/).max(1024).nullable()})
export type Block=z.infer<typeof blockSchema>
export type BlockList=z.infer<typeof blockListSchema>
export type SafetyOperation='createConcertReport'|'blockConcertUser'|'unblockConcertUser'
export interface CommandBinding{readonly scope:PrivacyScope;readonly operation:SafetyOperation;readonly key:string;readonly target:ReportTarget;readonly resourceId:string|null}
const receiptSchema=z.strictObject({receipt_id:uuid,event_id:uuid.nullable(),actor_id:uuid,operation_id:z.string().min(1).max(100),idempotency_key:uuid,
 state:z.enum(['PENDING','SUCCEEDED','FAILED','RETIRED']),resource_id:uuid.nullable(),resource_revision:revision.nullable(),failure_reason:z.string().nullable(),created_at:timestamp,expires_at:timestamp,result_status:z.number().int().min(200).max(299).nullable()})
export type Receipt=z.infer<typeof receiptSchema>
export const sameScope=(a:PrivacyScope,b:PrivacyScope)=>a.userId===b.userId&&a.eventId===b.eventId&&a.generation===b.generation&&a.policyGeneration===b.policyGeneration&&a.gate===b.gate
export function freeze<T>(v:T):T{if(v&&typeof v==='object'){Object.values(v).forEach(freeze);Object.freeze(v)}return v}
export function decodeReceipt(value:unknown,b:CommandBinding,now:number):Receipt{
 const r=receiptSchema.parse(value)
 if(r.actor_id!==b.scope.userId||r.event_id!==b.scope.eventId||r.operation_id!==b.operation||r.idempotency_key!==b.key
  ||Date.parse(r.created_at)>now||Date.parse(r.created_at)>=Date.parse(r.expires_at)||Date.parse(r.expires_at)<=now
  ||b.resourceId!==null&&r.resource_id!==null&&r.resource_id!==b.resourceId)throw new Error('Unbound receipt')
 if(r.state==='PENDING'||r.state==='RETIRED'){
  if(r.resource_id!==null||r.resource_revision!==null||r.result_status!==null||r.failure_reason!==null)throw new Error('Invalid pending receipt')
 }else if(r.state==='SUCCEEDED'){
  if(r.result_status!==200||r.failure_reason!==null||!r.resource_id||!r.resource_revision||b.resourceId!==null&&r.resource_id!==b.resourceId)throw new Error('Invalid success receipt')
 }else if(r.result_status!==null||!r.failure_reason||!(Object.values(DOMAIN_OPERATIONS[b.operation].errors).flat() as readonly string[]).includes(r.failure_reason))throw new Error('Invalid failed receipt')
 return freeze(r)
}
export function parseReportLink(search:string):ReportTarget|null{
 const p=new URLSearchParams(search),entries=[...p]
 if(entries.length!==2||p.getAll('targetType').length!==1||p.getAll('targetId').length!==1)return null
 const target=targetSchema.safeParse({target_type:p.get('targetType'),target_id:p.get('targetId')})
 return target.success?freeze(target.data):null
}
