import { z } from 'zod'
import type { Decoder } from '../../../shared/api/http'
export const uuid = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
export const platform = z.enum(['INSTAGRAM', 'X'])
export type Platform = z.infer<typeof platform>
const revision = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
const creationRevision = z.number().int().min(0).max(Number.MAX_SAFE_INTEGER)
const timestamp = z.iso.datetime({ offset: true }).refine(value => Number.isFinite(Date.parse(value)))
const cursor = z.string().min(1).max(1024).regex(/^[A-Za-z0-9_-]+$/).nullable()
export const handleState = z.strictObject({ user_id: uuid, platform, version: creationRevision, configured: z.boolean() })
  .refine(h => !h.configured || h.version > 0)
export type HandleState = z.infer<typeof handleState>
export const handleList = z.strictObject({ items: z.array(handleState).length(2) }).refine(v => new Set(v.items.map(h => h.platform)).size === 2)
export const consent = z.strictObject({ platform, requester_handle_version: revision, recipient_handle_version: revision, requester_consents: z.literal(true) })
export const requestState = z.enum(['PENDING', 'APPROVED', 'REJECTED', 'CANCELLED', 'REVOKED', 'EXPIRED'])
export type RequestStatus = z.infer<typeof requestState>
export const socialRequest = z.strictObject({
  request_id: uuid, event_id: uuid, requester_id: uuid, recipient_id: uuid, intro_id: uuid, intro_revision: revision,
  consents: z.array(consent).min(1).max(2), approved_platforms: z.array(platform).max(2),
  expires_at: timestamp, created_at: timestamp, state: requestState, revision,
}).refine(r => r.requester_id !== r.recipient_id && Date.parse(r.expires_at) > Date.parse(r.created_at)
  && new Set(r.consents.map(c => c.platform)).size === r.consents.length && new Set(r.approved_platforms).size === r.approved_platforms.length
  && r.approved_platforms.every(p => r.consents.some(c => c.platform === p))
  && (r.state !== 'APPROVED' || r.approved_platforms.length > 0)
  && (!['PENDING', 'REJECTED', 'CANCELLED'].includes(r.state) || r.approved_platforms.length === 0))
export type SocialRequest = z.infer<typeof socialRequest>
export const requestList = z.strictObject({ items: z.array(socialRequest).max(100), next_cursor: cursor })
export const offer = z.strictObject({ intro_id: uuid, intro_revision: revision, recipient_id: uuid,
  available_handles: z.array(z.strictObject({ platform, version: revision })).max(2),
}).refine(o => new Set(o.available_handles.map(h => h.platform)).size === o.available_handles.length)
export type SharingOffer = z.infer<typeof offer>
export const reveal = z.strictObject({ request_id: uuid, platform, owner_id: uuid, handle_version: revision,
  handle: z.string().min(1).max(30), grant_revision: revision, expires_at: timestamp,
}).refine(r => validHandle(r.platform, r.handle))
export type PrivateReveal = z.infer<typeof reveal>
export const receipt = z.strictObject({ receipt_id: uuid, event_id: uuid.nullable(), actor_id: uuid,
  operation_id: z.string().min(1).max(100), idempotency_key: uuid, state: z.enum(['PENDING', 'SUCCEEDED', 'FAILED', 'RETIRED']),
  resource_id: uuid.nullable(), resource_revision: revision.nullable(), failure_reason: z.string().min(1).max(100).nullable(),
  created_at: timestamp, expires_at: timestamp, result_status: z.number().int().min(200).max(299).nullable(),
})
export type Receipt = z.infer<typeof receipt>
export function validHandle(p: Platform, value: string): boolean {
  return p === 'X' ? /^[A-Za-z0-9_]{1,15}$/.test(value) : /^[A-Za-z0-9_](?:[A-Za-z0-9._]{0,28}[A-Za-z0-9_])?$/.test(value) && !value.includes('..')
}
export function freeze<T>(value: T): T {
  if (value && typeof value === 'object') { Object.values(value).forEach(freeze); Object.freeze(value) }
  return value
}
export function decode<T>(schema: z.ZodType<T>, check: (data: T) => boolean = () => true): Decoder<T> {
  return value => { const data = schema.parse(value); if (!check(data)) throw new Error('Invalid social context'); return freeze(data) }
}
