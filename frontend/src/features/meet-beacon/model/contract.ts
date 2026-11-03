import { z } from 'zod'
import type { Decoder } from '../../../shared/api/http'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { DOMAIN_OPERATIONS } from '../../../shared/api/domain-operations.generated'
import common from '../../../../../contracts/domain/v1/common.schema.json'

export const uuid = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
export const revision = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
export const utc = z.iso.datetime().refine(s => Number.isFinite(Date.parse(s)) && new Date(s).toISOString().slice(0, 19) === s.slice(0, 19))
const text = (max: number) => z.string().refine(s => Array.from(s).length >= 1 && Array.from(s).length <= max)
export const audienceSchema = z.union([
  z.strictObject({ kind: z.literal('PARTY_MEMBERS'), party_id: uuid }),
  z.strictObject({ kind: z.literal('EXPLICIT_USERS'), user_ids: z.array(uuid).min(1).max(50).refine(ids => new Set(ids).size === ids.length) }),
])
export const meetingSchema = z.strictObject({ place_id: uuid, map_revision: revision, place_revision: revision,
  instructions: text(1000), identification: text(240), audience: audienceSchema, expires_at: utc })
export const beaconSchema = z.strictObject({ beacon_id: uuid, event_id: uuid, owner_id: uuid, meeting: meetingSchema,
  created_at: utc, updated_at: utc, state: z.enum(['ACTIVE', 'EXPIRED', 'REVOKED']), revision })
  .refine(b => Date.parse(b.created_at) <= Date.parse(b.updated_at) && Date.parse(b.updated_at) < Date.parse(b.meeting.expires_at))
export const statusSchema = z.strictObject({ beacon_id: uuid, event_id: uuid, state: z.enum(['ACTIVE', 'EXPIRED', 'REVOKED']), revision })
const reasons = common.$defs.CommandReceipt.properties.failure_reason.anyOf[0]!.enum!
export const receiptSchema = z.strictObject({ receipt_id: uuid, event_id: uuid.nullable(), actor_id: uuid,
  operation_id: z.string().min(1).max(100), idempotency_key: uuid, state: z.enum(['PENDING', 'SUCCEEDED', 'FAILED', 'RETIRED']),
  resource_id: uuid.nullable(), resource_revision: revision.nullable(), failure_reason: z.enum(reasons).nullable(),
  created_at: utc, expires_at: utc, result_status: z.number().int().min(200).max(299).nullable() })
export type Audience = z.infer<typeof audienceSchema>
export type MeetingDescription = z.infer<typeof meetingSchema>
export type MeetBeacon = z.infer<typeof beaconSchema>
export type BeaconStatus = z.infer<typeof statusSchema>
export type Receipt = z.infer<typeof receiptSchema>
export type BeaconOperation = 'createConcertBeacon' | 'updateConcertBeacon' | 'revokeConcertBeacon'
export interface CommandBinding { scope: PrivacyScope; operation: BeaconOperation; key: string; beaconId: string | null; expectedRevision: number }
export function audienceKey(a: Audience): string { return a.kind === 'PARTY_MEMBERS' ? `party:${a.party_id}` : `users:${[...a.user_ids].sort().join(',')}` }
export function sameMeeting(a: MeetingDescription, b: MeetingDescription): boolean {
  return a.place_id === b.place_id && a.place_revision === b.place_revision && a.map_revision === b.map_revision
    && a.instructions === b.instructions && a.identification === b.identification && audienceKey(a.audience) === audienceKey(b.audience)
    && Date.parse(a.expires_at) === Date.parse(b.expires_at)
}
export function freeze<T>(value: T): T {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) { Object.values(value).forEach(freeze); Object.freeze(value) }
  return value
}
function receipt(value: unknown, b: CommandBinding): Receipt {
  const r = receiptSchema.parse(value)
  if (r.event_id !== b.scope.eventId || r.actor_id !== b.scope.userId || r.operation_id !== b.operation || r.idempotency_key !== b.key
    || Date.parse(r.created_at) >= Date.parse(r.expires_at)) throw new Error('Invalid command binding')
  if (r.state === 'PENDING' || r.state === 'RETIRED') {
    if (r.resource_id !== null || r.resource_revision !== null || r.failure_reason !== null || r.result_status !== null) throw new Error('Invalid pending receipt')
  } else if (r.state === 'SUCCEEDED') {
    if (r.result_status !== 200 || r.failure_reason !== null || !r.resource_id || r.resource_revision === null
      || r.resource_revision <= b.expectedRevision || b.beaconId !== null && r.resource_id !== b.beaconId) throw new Error('Invalid success receipt')
  } else if (r.result_status !== null || !r.failure_reason || !(Object.values(DOMAIN_OPERATIONS[b.operation].errors).flat() as string[]).includes(r.failure_reason)) {
    throw new Error('Invalid failure receipt')
  }
  return freeze(r)
}
export function decodeReceipt(binding: CommandBinding): Decoder<Receipt> { return (value, c) => {
  if (c.status !== 200 || c.scope.userId !== binding.scope.userId || c.scope.eventId !== binding.scope.eventId
    || c.scope.generation !== binding.scope.generation) throw new Error('Invalid receipt context')
  return receipt(value, binding)
} }
export const decodeBeacon: Decoder<MeetBeacon> = (value, c) => {
  const b = beaconSchema.parse(value)
  if (c.status !== 200 || b.event_id !== c.scope.eventId || b.beacon_id !== c.path.split('/').at(-1)) throw new Error('Invalid meeting binding')
  return freeze(b)
}
export function decodeCommand(binding: CommandBinding): Decoder<MeetBeacon | BeaconStatus | Receipt> { return (value, c) => {
  if (c.scope.userId !== binding.scope.userId || c.scope.eventId !== binding.scope.eventId || c.scope.generation !== binding.scope.generation) throw new Error('Invalid scope')
  if (c.status === 202) { const r = receipt(value, binding); if (r.state !== 'PENDING') throw new Error('Invalid pending command'); return r }
  if (c.status !== 200) throw new Error('Invalid status')
  const result = binding.operation === 'revokeConcertBeacon' ? statusSchema.parse(value) : beaconSchema.parse(value)
  if (result.event_id !== binding.scope.eventId || binding.beaconId !== null && result.beacon_id !== binding.beaconId
    || result.revision <= binding.expectedRevision || binding.operation === 'createConcertBeacon' && result.revision !== 1
    || result.state !== (binding.operation === 'revokeConcertBeacon' ? 'REVOKED' : 'ACTIVE')
    || 'owner_id' in result && result.owner_id !== binding.scope.userId) throw new Error('Invalid command result')
  return freeze(result)
} }
