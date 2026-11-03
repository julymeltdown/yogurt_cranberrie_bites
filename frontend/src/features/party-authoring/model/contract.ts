import { z } from 'zod'
import spec from '../../../../../contracts/generated/mcr-domain.openapi.json'
import type { Decoder } from '../../../shared/api/http'
import { DOMAIN_OPERATIONS, type DomainOperationId } from '../../../shared/api/domain-operations.generated'
import type { PrivacyScope } from '../../../shared/query/privacy'
export { uuid, type Party, type Receipt } from '../../party-discovery/model/contract'
export const utc = (v: string) => /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?Z$/.test(v) && Number.isFinite(Date.parse(v)) && new Date(v).toISOString().slice(0, 19) === v.slice(0, 19)
const text = (max: number, min = 0) => z.string().refine(v => Array.from(v).length >= min && Array.from(v).length <= max, `Use ${min}–${max} characters.`)
export const partyDraftSchema = z.strictObject({ title: text(120, 1).refine(v => v.trim().length > 0, 'Enter a title.'), description: text(2000),
  ticket_section: text(120), capacity: z.number().int().min(2).max(50), meeting_starts_at: z.string().refine(utc, 'Enter a valid UTC time.'),
  meeting_ends_at: z.string().refine(utc, 'Enter a valid UTC time.'), requirements: text(1000), public_place_id: z.uuid(),
  state: z.enum(['OPEN', 'CLOSED', 'COMPLETED']) }).refine(v => Date.parse(v.meeting_starts_at) < Date.parse(v.meeting_ends_at), { path: ['meeting_ends_at'], message: 'End time must be after start time.' })
export type PartyDraft = z.infer<typeof partyDraftSchema>
export const emptyDraft = (): PartyDraft => ({ title: '', description: '', ticket_section: '', capacity: 2, meeting_starts_at: '', meeting_ends_at: '', requirements: '', public_place_id: '', state: 'OPEN' })

// Interpret only the closed, reachable C04 schemas; reject unknown keywords rather than silently weakening validation.
type Schema = { $ref?: string; type?: string; const?: unknown; enum?: unknown[]; anyOf?: Schema[]; properties?: Record<string, Schema>; required?: string[]; additionalProperties?: boolean; items?: Schema; minItems?: number; maxItems?: number; minLength?: number; maxLength?: number; minimum?: number; maximum?: number; format?: string; pattern?: string }
const schemas = spec.components.schemas as unknown as Record<string, Schema>
export function contractValue<T>(name: string, value: unknown): T {
  const match = (v: unknown, s: Schema, depth: number): boolean => {
    if (depth > 30 || Object.keys(s).some(k => !['$ref','type','const','enum','anyOf','properties','required','additionalProperties','items','minItems','maxItems','minLength','maxLength','minimum','maximum','format','pattern'].includes(k))) return false
    if (s.$ref) { const key = s.$ref.replace('#/components/schemas/', ''); return s.$ref === `#/components/schemas/${key}` && !!schemas[key] && match(v, schemas[key]!, depth + 1) }
    if (s.anyOf) return s.anyOf.some(child => match(v, child, depth + 1))
    if ('const' in s && v !== s.const || s.enum && !s.enum.includes(v)) return false
    if (s.type === 'null') return v === null
    if (s.type === 'object') { if (!v || typeof v !== 'object' || Array.isArray(v) || s.additionalProperties !== false) return false
      return (s.required ?? []).every(k => Object.hasOwn(v,k)) && Object.entries(v).every(([k,item]) => !!s.properties?.[k] && match(item,s.properties[k]!,depth+1)) }
    if (s.type === 'array') return Array.isArray(v) && !!s.items && v.length >= (s.minItems ?? 0) && v.length <= (s.maxItems ?? 100) && v.every(item => match(item,s.items!,depth+1))
    if (s.type === 'integer') return typeof v === 'number' && Number.isSafeInteger(v) && v >= (s.minimum ?? 0) && v <= (s.maximum ?? Number.MAX_SAFE_INTEGER)
    if (s.type === 'boolean') return typeof v === 'boolean'
    if (s.type === 'string') return typeof v === 'string' && Array.from(v).length >= (s.minLength ?? 0) && Array.from(v).length <= (s.maxLength ?? 10000)
      && (!s.pattern || new RegExp(s.pattern,'u').test(v)) && (!s.format || s.format === 'uuid' && z.uuid().safeParse(v).success || s.format === 'date-time' && utc(v))
    return 'const' in s || !!s.enum
  }
  if (!schemas[name] || !match(value, schemas[name]!, 0)) throw new Error('Unverified contract value')
  return structuredClone(value) as T
}
export interface Binding { scope: PrivacyScope; operation: DomainOperationId; key: string; target: string | null }
export function receiptDecoder(b: Binding, path: string): Decoder<import('../../party-discovery/model/contract').Receipt> {
  return (value, context) => {
    const r = contractValue<import('../../party-discovery/model/contract').Receipt>('Domain_common_CommandReceipt', value)
    if (context.path !== path || context.status !== 200 && context.status !== 202 || context.scope.userId !== b.scope.userId || context.scope.eventId !== b.scope.eventId || context.scope.generation !== b.scope.generation
      || r.actor_id !== b.scope.userId || r.event_id !== b.scope.eventId || r.operation_id !== b.operation || r.idempotency_key !== b.key || Date.parse(r.created_at) >= Date.parse(r.expires_at)) throw new Error('Unverified receipt context')
    if (context.status === 202 && r.state !== 'PENDING') throw new Error('Invalid pending response')
    if (r.state === 'SUCCEEDED') {
      if (!r.resource_id || !r.resource_revision || r.result_status !== 200 || r.failure_reason !== null || b.target && r.resource_id !== b.target) throw new Error('Invalid successful receipt')
    } else if (r.state === 'FAILED') {
      const errors = Object.values(DOMAIN_OPERATIONS[b.operation].errors).flat() as readonly string[]
      if (!r.failure_reason || !errors.includes(r.failure_reason) || r.result_status !== null || r.resource_id !== null || r.resource_revision !== null) throw new Error('Invalid failed receipt')
    } else if (r.resource_id !== null || r.resource_revision !== null || r.result_status !== null || r.failure_reason !== null) throw new Error('Invalid pending receipt')
    return r
  }
}
