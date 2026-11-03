import { z } from 'zod'
import { DOMAIN_OPERATIONS } from '../api/domain-operations.generated'
import type { SessionScope } from '../api/http'
import { dimensions, IMAGE_TYPES, MAX_IMAGE_BYTES } from './image-policy'
import type { ReadyMedia, MediaPurpose } from './types'
const uuidSchema = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
export const uuid = (v: unknown): v is string => uuidSchema.safeParse(v).success
const revision = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
const timestamp = z.iso.datetime({ precision: null }).refine(v => v.endsWith('Z') && new Date(v).toISOString().slice(0, 19) === v.slice(0, 19))
const purpose = z.enum(['POST', 'GIVEAWAY', 'REPORT'])
const mime = z.enum(['image/jpeg', 'image/png', 'image/webp'])
const intentSchema = z.strictObject({ media_id: uuidSchema, state: z.literal('PENDING'), upload_path: z.string(),
  max_bytes: z.literal(MAX_IMAGE_BYTES), expires_at: timestamp, upload_method: z.literal('PUT'), revision })
const uploadedSchema = z.strictObject({ media_id: uuidSchema, state: z.literal('UPLOADED'), revision })
const mediaSchema = z.strictObject({ media_id: uuidSchema, state: z.literal('READY'), revision, width: revision.max(4096), height: revision.max(4096),
  content_type: mime, purpose, read_path: z.string() })
const receiptSchema = z.strictObject({ receipt_id: uuidSchema, event_id: uuidSchema.nullable(), actor_id: uuidSchema,
  operation_id: z.string().min(1).max(100), idempotency_key: uuidSchema, state: z.enum(['PENDING', 'SUCCEEDED', 'FAILED', 'RETIRED']),
  resource_id: uuidSchema.nullable(), resource_revision: revision.nullable(), failure_reason: z.string().nullable(),
  created_at: timestamp, expires_at: timestamp, result_status: z.number().int().min(200).max(299).nullable() })
export type UploadIntent = z.infer<typeof intentSchema>
export type UploadedBytes = z.infer<typeof uploadedSchema>
export type MediaReceipt = z.infer<typeof receiptSchema>
export type MediaOperation = 'initConcertUpload' | 'putConcertUploadBytes' | 'finalizeConcertUpload'
export interface CommandBinding {
  readonly scope: SessionScope; readonly operation: MediaOperation; readonly key: string; readonly mediaId: string | null
}
export type MediaCommandResult<T> = { readonly kind: 'result'; readonly value: T } | { readonly kind: 'pending'; readonly receipt: MediaReceipt }
const fail = (): never => { throw new Error('Invalid media response') }
export const mediaBase = (scope: SessionScope) => {
  if (!uuid(scope.eventId) || !uuid(scope.userId)) fail()
  return `/api/concert/events/${scope.eventId}/media`
}
export function decodeIntent(v: unknown, scope: SessionScope, now = Date.now()): UploadIntent {
  const i = intentSchema.parse(v)
  if (i.upload_path !== `${mediaBase(scope)}/${i.media_id}/bytes` || Date.parse(i.expires_at) <= now) fail()
  return Object.freeze(i)
}
export function decodeUploaded(v: unknown, mediaId: string): UploadedBytes {
  const b = uploadedSchema.parse(v); if (b.media_id !== mediaId) fail(); return Object.freeze(b)
}
export function decodeMedia(v: unknown, scope: SessionScope, mediaId: string, expectedPurpose: MediaPurpose): ReadyMedia {
  const m = mediaSchema.parse(v)
  dimensions(m.width, m.height)
  if (m.media_id !== mediaId || m.purpose !== expectedPurpose || !IMAGE_TYPES.includes(m.content_type)
    || m.read_path !== `${mediaBase(scope)}/${mediaId}/content`) fail()
  return Object.freeze(m)
}
export function decodeReceipt(v: unknown, b: CommandBinding): MediaReceipt {
  const r = receiptSchema.parse(v)
  if (r.actor_id !== b.scope.userId || r.event_id !== b.scope.eventId || r.operation_id !== b.operation || r.idempotency_key !== b.key
    || Date.parse(r.created_at) >= Date.parse(r.expires_at) || (b.mediaId !== null && r.resource_id !== null && r.resource_id !== b.mediaId)) fail()
  if (r.state === 'PENDING' || r.state === 'RETIRED') {
    if (r.resource_id !== null || r.resource_revision !== null || r.failure_reason !== null || r.result_status !== null) fail()
  } else if (r.state === 'SUCCEEDED') {
    if (r.result_status !== 200 || r.failure_reason !== null || !uuid(r.resource_id) || r.resource_revision === null) fail()
  } else if (!r.failure_reason || r.result_status !== null
    || !(Object.values(DOMAIN_OPERATIONS[b.operation].errors).flat() as readonly string[]).includes(r.failure_reason)) fail()
  return Object.freeze(r)
}
