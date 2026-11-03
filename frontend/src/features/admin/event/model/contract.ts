import { z } from 'zod'
import common from '../../../../../../contracts/domain/v1/common.schema.json'
import { DOMAIN_OPERATIONS } from '../../../../shared/api/domain-operations.generated'

export const uuid = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
export const revision = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
export const text = (max: number) => z.string().refine(s => s.trim().length > 0 && Array.from(s).length <= max)
export const utc = z.iso.datetime().refine(s => Number.isFinite(Date.parse(s)) && new Date(s).toISOString().slice(0, 19) === s.slice(0, 19))
export const source = z.url().max(2048).refine(value => {
  try { const u = new URL(value); return u.protocol === 'https:' && !u.username && !u.password && !/[\s\\]/u.test(value) } catch { return false }
})
export const cursor = z.string().regex(/^[A-Za-z0-9_-]+$/).max(1024).nullable()
export const eventSchema = z.strictObject({ event_id: uuid, slug: z.string().regex(/^[a-z0-9][a-z0-9-]{0,63}$/), title: text(120),
  phase: z.enum(['DRAFT', 'OPEN', 'LIVE', 'READ_ONLY', 'ARCHIVED']), timezone: z.literal('Asia/Seoul'), revision })
export const featuresSchema = z.strictObject({ parties: z.boolean(), giveaways: z.boolean(), intros: z.boolean(), social: z.boolean(), mural: z.boolean() })
const settingsFields = { features: featuresSchema, beacon_ttl_seconds: z.number().int().min(1).max(900),
  pickup_grace_seconds: z.number().int().min(0).max(86400), social_grant_ttl_seconds: z.number().int().min(1).max(86400),
  mural_cooldown_ms: z.number().int().min(0).max(3600000) }
export const settingsSchema = z.strictObject({ event: eventSchema, revision, ...settingsFields }).refine(s => s.revision === s.event.revision)
export const setSettingsSchema = z.strictObject({ expected_revision: revision, ...settingsFields, reason: text(500) })
export const setPhaseSchema = z.strictObject({ expected_revision: revision, phase: z.enum(['OPEN', 'LIVE']), reason: text(500) })
export const overviewSchema = z.strictObject({ event_id: uuid, revision, health: z.enum(['AVAILABLE', 'DEGRADED', 'UNAVAILABLE']),
  open_reports: z.number().int().min(0).max(2147483647), pending_jobs: z.number().int().min(0).max(2147483647) })
const noticeFields = { title: text(120), text: text(4000), source_uri: source, confirmed_at: utc, state: z.enum(['DRAFT', 'PUBLISHED', 'WITHDRAWN']) }
export const createNoticeSchema = z.strictObject(noticeFields)
export const updateNoticeSchema = z.strictObject({ expected_revision: revision, ...noticeFields })
export const noticeSchema = z.strictObject({ notice_id: uuid, event_id: uuid, ...noticeFields, created_at: utc, revision })
export const noticesSchema = z.strictObject({ items: z.array(noticeSchema).max(100), next_cursor: cursor })
  .refine(s => new Set(s.items.map(n => n.notice_id)).size === s.items.length)
export const receiptSchema = z.strictObject({ receipt_id: uuid, event_id: uuid.nullable(), actor_id: uuid,
  operation_id: z.string().min(1).max(100), idempotency_key: uuid, state: z.enum(['PENDING', 'SUCCEEDED', 'FAILED', 'RETIRED']),
  resource_id: uuid.nullable(), resource_revision: revision.nullable(),
  failure_reason: z.enum(common.$defs.CommandReceipt.properties.failure_reason.anyOf[0]!.enum!).nullable(),
  created_at: utc, expires_at: utc, result_status: z.number().int().min(200).max(299).nullable() })
export type Settings = z.infer<typeof settingsSchema>
export type SetSettings = z.infer<typeof setSettingsSchema>
export type SetPhase = z.infer<typeof setPhaseSchema>
export type Notice = z.infer<typeof noticeSchema>
export type CreateNotice = z.infer<typeof createNoticeSchema>
export type UpdateNotice = z.infer<typeof updateNoticeSchema>
export type Overview = z.infer<typeof overviewSchema>
export type Receipt = z.infer<typeof receiptSchema>
export type Operation = 'setConcertSettings' | 'setConcertEventPhase' | 'setConcertPlace' | 'createConcertNotice' | 'updateConcertNotice'
export function freeze<T>(value: T): T {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) { Object.values(value).forEach(freeze); Object.freeze(value) }
  return value
}
export function equal(a: unknown, b: unknown): boolean {
  if (a === b) return true
  if (!a || !b || typeof a !== 'object' || typeof b !== 'object') return false
  const left = Object.keys(a), right = Object.keys(b)
  return left.length === right.length && left.every(key => Object.hasOwn(b, key) && equal(Reflect.get(a, key), Reflect.get(b, key)))
}
export function boundReceipt(value: unknown, binding: { actor: string; event: string; operation: Operation; key: string; target: string | null; revision: number }, now: number): Receipt {
  const r = receiptSchema.parse(value)
  if (r.actor_id !== binding.actor || r.event_id !== binding.event || r.operation_id !== binding.operation || r.idempotency_key !== binding.key
    || Date.parse(r.created_at) > now || Date.parse(r.created_at) >= Date.parse(r.expires_at)
    || binding.target !== null && r.resource_id !== null && r.resource_id !== binding.target) throw new Error('Unbound receipt')
  if (r.state === 'PENDING' || r.state === 'RETIRED') {
    if (r.resource_id !== null || r.resource_revision !== null || r.failure_reason !== null || r.result_status !== null) throw new Error('Invalid pending receipt')
  } else if (r.state === 'SUCCEEDED') {
    if (r.result_status !== 200 || r.failure_reason !== null || r.resource_id === null || r.resource_revision === null
      || r.resource_revision <= binding.revision || binding.target !== null && r.resource_id !== binding.target) throw new Error('Unbound success')
  } else if (r.result_status !== null || r.failure_reason === null
    || !(Object.values(DOMAIN_OPERATIONS[binding.operation].errors).flat() as string[]).includes(r.failure_reason)) throw new Error('Invalid failed receipt')
  return freeze(r)
}
