import contract from '../../../../../contracts/generated/mcr-domain.openapi.json'
import { DOMAIN_OPERATIONS } from '../../../shared/api/domain-operations.generated'
import type { Decoder, SessionScope } from '../../../shared/api/http'

export const uuid = (s: unknown): s is string => typeof s === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(s)
export interface Profile { readonly user_id: string; readonly display_name: string; readonly bio: string | null; readonly avatar: unknown }
export interface Party {
  readonly party_id: string; readonly event_id: string; readonly owner: Profile; readonly title: string; readonly description: string
  readonly ticket_section: string | null; readonly capacity: number; readonly member_count: number; readonly meeting_starts_at: string
  readonly meeting_ends_at: string; readonly requirements: string; readonly created_at: string
  readonly state: 'OPEN' | 'CLOSED' | 'CANCELLED' | 'COMPLETED'; readonly revision: number; readonly public_place_id: string; readonly meeting_revision: number
}
export interface Application {
  readonly application_id: string; readonly event_id: string; readonly party_id: string; readonly applicant: Profile
  readonly note: string; readonly created_at: string; readonly state: 'PENDING' | 'APPROVED' | 'REJECTED' | 'CANCELLED'; readonly revision: number
}
export interface Member { readonly event_id: string; readonly party_id: string; readonly user_id: string; readonly role: 'OWNER' | 'MEMBER'
  readonly state: 'ACTIVE' | 'REMOVED' | 'LEFT'; readonly revision: number; readonly joined_at: string }
export interface Page<T> { readonly items: readonly T[]; readonly next_cursor: string | null }
export interface Activity { readonly owned: Page<Party>; readonly memberships: Page<Member>; readonly applications: Page<Application> }
export type CommandOperation = 'applyConcertParty' | 'cancelConcertApplication'
export interface Receipt {
  readonly receipt_id: string; readonly event_id: string | null; readonly actor_id: string; readonly operation_id: string
  readonly idempotency_key: string; readonly state: 'PENDING' | 'SUCCEEDED' | 'FAILED' | 'RETIRED'
  readonly resource_id: string | null; readonly resource_revision: number | null; readonly failure_reason: string | null
  readonly created_at: string; readonly expires_at: string; readonly result_status: number | null
}
export interface CommandBinding { readonly scope: SessionScope; readonly operation: CommandOperation; readonly key: string; readonly partyId: string; readonly applicationId?: string }
type Schema = { $ref?: string; type?: string; const?: unknown; enum?: unknown[]; anyOf?: Schema[]; oneOf?: Schema[]; properties?: Record<string, Schema>; required?: string[]; additionalProperties?: boolean
  items?: Schema; minItems?: number; maxItems?: number; minLength?: number; maxLength?: number; pattern?: string; format?: string; minimum?: number; maximum?: number }
const schemas = contract.components.schemas as unknown as Record<string, Schema>
const fail = (): never => { throw new Error('Invalid party response') }
/** Restricted interpreter for the reachable frozen C04/C03 response schemas. No permissive fallback. */
function conforms(value: unknown, schema: Schema, depth = 0): boolean {
  if (depth > 24 || Object.keys(schema).some(k => !['$ref', 'type', 'const', 'enum', 'anyOf', 'oneOf', 'properties', 'required', 'additionalProperties', 'items', 'minItems', 'maxItems', 'minLength', 'maxLength', 'pattern', 'format', 'minimum', 'maximum'].includes(k))) return false
  if (schema.$ref) {
    if (!/^#\/components\/schemas\/[A-Za-z0-9_]+$/.test(schema.$ref)) return false
    const next = schemas[schema.$ref.split('/').at(-1)!]
    return !!next && conforms(value, next, depth + 1)
  }
  if (schema.anyOf) return schema.anyOf.some(s => conforms(value, s, depth + 1))
  if (schema.oneOf) return schema.oneOf.filter(s => conforms(value, s, depth + 1)).length === 1
  if ('const' in schema && value !== schema.const) return false
  if (schema.enum && !schema.enum.includes(value)) return false
  if (schema.type === 'null') return value === null
  if (schema.type === 'object') {
    if (!value || typeof value !== 'object' || Array.isArray(value) || schema.additionalProperties !== false || !schema.properties) return false
    const v = value as Record<string, unknown>
    return (schema.required ?? []).every(k => Object.hasOwn(v, k)) && Object.keys(v).every(k => schema.properties![k] && conforms(v[k], schema.properties![k]!, depth + 1))
  }
  if (schema.type === 'array') return Array.isArray(value) && !!schema.items && value.length >= (schema.minItems ?? 0)
    && value.length <= (schema.maxItems ?? 100) && value.every(v => conforms(v, schema.items!, depth + 1))
  if (schema.type === 'integer') return typeof value === 'number' && Number.isSafeInteger(value) && value >= (schema.minimum ?? Number.MIN_SAFE_INTEGER) && value <= (schema.maximum ?? Number.MAX_SAFE_INTEGER)
  if (schema.type === 'string') {
    if (typeof value !== 'string' || Array.from(value).length < (schema.minLength ?? 0) || Array.from(value).length > (schema.maxLength ?? 10000)) return false
    if (schema.pattern && !new RegExp(schema.pattern, 'u').test(value)) return false
    if (schema.format === 'uuid' && !uuid(value)) return false
    if (schema.format === 'date-time') {
      if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/.test(value) || !Number.isFinite(Date.parse(value))) return false
      const date = new Date(value)
      if (date.toISOString().slice(0, 19) !== value.slice(0, 19)) return false
    }
    return true
  }
  return 'const' in schema || !!schema.enum
}
function decoded<T>(name: string, value: unknown): T { if (!schemas[name] || !conforms(value, schemas[name]!)) fail(); return structuredClone(value) as T }
function freeze<T>(v: T): T { if (v && typeof v === 'object') { Object.values(v).forEach(freeze); Object.freeze(v) }; return v }
function party(p: Party, scope: SessionScope) {
  if (p.event_id !== scope.eventId || p.member_count > p.capacity || Date.parse(p.meeting_starts_at) >= Date.parse(p.meeting_ends_at)) fail()
}
function application(a: Application, scope: SessionScope) { if (a.event_id !== scope.eventId || a.applicant.user_id !== scope.userId) fail() }
function page<T>(p: Page<T>, limit: number, id: (item: T) => string) { if (p.items.length > limit || new Set(p.items.map(id)).size !== p.items.length) fail() }
function context(path: string, expected: string, status: number) { if (path !== expected || status !== 200) fail() }
export function decodeList(limit: number, expected: string): Decoder<Page<Party>> { return (v, c) => {
  context(c.path, expected, c.status); const p = decoded<Page<Party>>('Domain_party_PartyList', v)
  page(p, limit, p => p.party_id); p.items.forEach(p => party(p, c.scope)); return freeze(p)
} }
export function decodeParty(id: string, expected: string): Decoder<Party> { return (v, c) => {
  context(c.path, expected, c.status); const p = decoded<Party>('Domain_party_PartyPublic', v)
  party(p, c.scope); if (p.party_id !== id) fail(); return freeze(p)
} }
export function decodeActivity(limit: number, expected: string): Decoder<Activity> { return (v, c) => {
  context(c.path, expected, c.status); const a = decoded<Activity>('Domain_party_PartyActivity', v)
  page(a.owned, limit, p => p.party_id); page(a.memberships, limit, m => m.party_id); page(a.applications, limit, a => a.application_id)
  a.owned.items.forEach(p => { party(p, c.scope); if (p.owner.user_id !== c.scope.userId) fail() })
  a.memberships.items.forEach(m => { if (m.event_id !== c.scope.eventId || m.user_id !== c.scope.userId) fail() })
  a.applications.items.forEach(a => application(a, c.scope)); return freeze(a)
} }
function receipt(v: unknown, b: CommandBinding): Receipt {
  const r = decoded<Receipt>('Domain_common_CommandReceipt', v)
  if (r.event_id !== b.scope.eventId || r.actor_id !== b.scope.userId || r.operation_id !== b.operation || r.idempotency_key !== b.key
    || Date.parse(r.created_at) >= Date.parse(r.expires_at)) fail()
  if (b.applicationId && r.resource_id !== null && r.resource_id !== b.applicationId) fail()
  if (r.state === 'PENDING' || r.state === 'RETIRED') {
    if (r.resource_id !== null || r.resource_revision !== null || r.result_status !== null || r.failure_reason !== null) fail()
  } else if (r.state === 'SUCCEEDED') {
    if (r.result_status !== 200 || r.failure_reason !== null || (b.applicationId && r.resource_id !== b.applicationId)) fail()
  } else {
    const allowed = Object.values(DOMAIN_OPERATIONS[b.operation].errors).flat() as readonly string[]
    if (!r.failure_reason || !allowed.includes(r.failure_reason) || r.result_status !== null) fail()
  }
  return freeze(r)
}
export function decodeReceipt(b: CommandBinding, expected: string): Decoder<Receipt> { return (v, c) => {
  context(c.path, expected, c.status); if (c.scope.eventId !== b.scope.eventId || c.scope.userId !== b.scope.userId || c.scope.generation !== b.scope.generation) fail()
  return receipt(v, b)
} }
export function decodeCommand(b: CommandBinding, expected: string): Decoder<Application | Receipt> { return (v, c) => {
  if (c.path !== expected || c.scope.userId !== b.scope.userId || c.scope.eventId !== b.scope.eventId || c.scope.generation !== b.scope.generation) fail()
  if (c.status === 202) { const r = receipt(v, b); if (r.state !== 'PENDING') fail(); return r }
  if (c.status !== 200) fail()
  const a = decoded<Application>('Domain_party_Application', v); application(a, b.scope)
  if (a.party_id !== b.partyId || (b.applicationId && a.application_id !== b.applicationId)
    || a.state !== (b.operation === 'applyConcertParty' ? 'PENDING' : 'CANCELLED')) fail()
  return freeze(a)
} }
