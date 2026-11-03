import { z } from 'zod'
import browserContract from '../../../../../contracts/generated/mcr-domain.openapi.json'
import type { DecodeContext, Decoder, SessionScope } from '../../../shared/api/http'
import type { AvatarState } from '../../../entities/avatar/whole-admission'

export const giftId = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/
export type GiftState = 'DRAFT' | 'OPEN' | 'PAUSED' | 'CLOSED' | 'CANCELLED'
export interface PublicOwner {
  readonly user_id: string; readonly display_name: string; readonly bio: string | null
  readonly avatar: AvatarState
}
export interface Giveaway {
  readonly giveaway_id: string; readonly event_id: string; readonly owner: PublicOwner
  readonly title: string; readonly description: string; readonly image_media_id: string | null
  readonly quantity_total: number; readonly available: number; readonly pickup_starts_at: string
  readonly pickup_ends_at: string; readonly pickup_grace_seconds: number; readonly public_place_id: string
  readonly created_at: string; readonly state: GiftState; readonly revision: number
}
export interface Reservation {
  readonly reservation_id: string; readonly event_id: string; readonly giveaway_id: string
  readonly recipient_id: string; readonly expires_at: string; readonly created_at: string
  readonly claim_receipt_id: string | null; readonly state: 'HELD' | 'READY' | 'CLAIMED' | 'CANCELLED' | 'EXPIRED'
  readonly revision: number
}
export interface Page<T> { readonly items: readonly T[]; readonly next_cursor: string | null }
export interface GiftActivity { readonly giving: Page<Giveaway>; readonly receiving: Page<Reservation> }
export interface GiftFilters {
  readonly state?: Exclude<GiftState, 'DRAFT'>; readonly in_stock?: boolean
  readonly pickup_after?: string; readonly pickup_before?: string; readonly public_place_id?: string
}
export type GiftQuery = Readonly<Record<string, string | number | boolean>>

function assert(value: unknown): asserts value { if (!value) throw new Error('Invalid giveaway response') }
const record = (value: unknown): Record<string, unknown> => {
  assert(value !== null && typeof value === 'object' && !Array.isArray(value)); return value as Record<string, unknown>
}
const schemas: Record<string, unknown> = browserContract.components.schemas
const compiled = new Map<string, z.ZodType>()
const pending = new Set<string>()
const supported = new Set(['type', '$ref', 'const', 'enum', 'properties', 'required', 'additionalProperties', 'items',
  'minItems', 'maxItems', 'minLength', 'maxLength', 'pattern', 'format', 'minimum', 'maximum', 'anyOf', 'oneOf', 'description', 'default'])
// Compile ONLY the frozen, reachable browser schemas; unknown keywords fail closed.
// New schemas are not independently maintained approximations of the wire contract.
function compile(value: unknown): z.ZodType {
  const s = record(value)
  assert(Object.keys(s).every(key => supported.has(key)))
  if (typeof s.$ref === 'string') {
    assert(/^#\/components\/schemas\/[A-Za-z0-9_]+$/.test(s.$ref)); return named(s.$ref.split('/').at(-1)!)
  }
  if (Array.isArray(s.anyOf)) {
    const branches = s.anyOf.map(compile)
    return z.unknown().refine(v => branches.some(branch => branch.safeParse(v).success))
  }
  if (Array.isArray(s.oneOf)) {
    const branches = s.oneOf.map(compile)
    return z.unknown().refine(v => branches.filter(branch => branch.safeParse(v).success).length === 1)
  }
  if (Object.hasOwn(s, 'const')) return z.unknown().refine(v => v === s.const)
  if (Array.isArray(s.enum)) return z.unknown().refine(v => (s.enum as unknown[]).includes(v))
  if (s.type === 'null') return z.null()
  if (s.type === 'boolean') return z.boolean()
  if (s.type === 'object') {
    assert(s.additionalProperties === false)
    const properties = record(s.properties), required = s.required as string[]
    assert(Array.isArray(required) && required.every(key => Object.hasOwn(properties, key)))
    return z.strictObject(Object.fromEntries(Object.entries(properties).map(([key, rule]) => {
      const field = compile(rule); return [key, required.includes(key) ? field : field.optional()]
    })))
  }
  if (s.type === 'array') return z.array(compile(s.items)).min(Number(s.minItems ?? 0)).max(Number(s.maxItems ?? 100))
  if (s.type === 'integer') return z.number().int().min(Number(s.minimum ?? Number.MIN_SAFE_INTEGER)).max(Number(s.maximum ?? Number.MAX_SAFE_INTEGER))
  if (s.type === 'string') {
    assert(s.format === undefined || ['uuid', 'date-time'].includes(String(s.format)))
    const pattern = typeof s.pattern === 'string' ? new RegExp(s.pattern, 'u') : null
    return z.string().refine(v => {
      const length = Array.from(v).length
      return Array.from(v).every(char => char.codePointAt(0)! < 0xd800 || char.codePointAt(0)! > 0xdfff)
        && length >= Number(s.minLength ?? 0) && length <= Number(s.maxLength ?? 10000)
        && (!pattern || pattern.test(v)) && (s.format !== 'uuid' || giftId.test(v))
        && (s.format !== 'date-time' || (z.iso.datetime().safeParse(v).success && Number.isFinite(Date.parse(v))))
    })
  }
  throw new Error('Unsupported giveaway schema')
}
function named(name: string): z.ZodType {
  const found = compiled.get(name)
  if (found) return found
  assert(Object.hasOwn(schemas, name) && !pending.has(name)); pending.add(name)
  try { const schema = compile(schemas[name]); compiled.set(name, schema); return schema } finally { pending.delete(name) }
}
const giftSchema = named('Domain_giveaway_GiveawayPublic')
const pageSchema = named('Domain_giveaway_GiveawayList')
const activitySchema = named('Domain_giveaway_GiveawayActivity')
const querySchema = named('Domain_giveaway_GiveawayQuery')
export function freezeGift<T>(value: T): T {
  if (value && typeof value === 'object') { for (const child of Object.values(value)) freezeGift(child); Object.freeze(value) }
  return value
}
function sameScope(a: SessionScope, b: SessionScope) { return a.generation === b.generation && a.eventId === b.eventId && a.userId === b.userId }
function bind(context: DecodeContext, captured: SessionScope, path: string) {
  assert(context.status === 200 && sameScope(context.scope, captured) && context.path === path
    && captured.userId && giftId.test(captured.userId) && captured.eventId && giftId.test(captured.eventId))
}
function gift(value: Giveaway, scope: SessionScope, own: boolean) {
  assert(value.event_id === scope.eventId && value.available <= value.quantity_total
    && Date.parse(value.pickup_starts_at) < Date.parse(value.pickup_ends_at)
    && Date.parse(value.created_at) < Date.parse(value.pickup_ends_at)
    && (own ? value.owner.user_id === scope.userId : value.state !== 'DRAFT'))
}
function reservation(value: Reservation, scope: SessionScope) {
  assert(value.event_id === scope.eventId && value.recipient_id === scope.userId
    && Date.parse(value.created_at) < Date.parse(value.expires_at)
    && (value.state === 'CLAIMED' ? value.claim_receipt_id !== null : value.claim_receipt_id === null))
}
function ordered<T extends { readonly created_at: string }>(items: readonly T[], id: (item: T) => string, limit: number) {
  assert(items.length <= limit && new Set(items.map(id)).size === items.length)
  items.forEach((row, index) => {
    const previous = items[index - 1]
    if (previous) assert(Date.parse(previous.created_at) > Date.parse(row.created_at)
      || (Date.parse(previous.created_at) === Date.parse(row.created_at) && id(previous) > id(row)))
  })
}
export function validateFilters(input: GiftFilters): GiftFilters {
  querySchema.parse(input)
  assert(!input.pickup_after || !input.pickup_before || Date.parse(input.pickup_after) < Date.parse(input.pickup_before))
  return freezeGift({ ...input })
}
export function giveawayListDecoder(scope: SessionScope, query: GiftQuery): Decoder<Page<Giveaway>> {
  const path = `/api/concert/events/${scope.eventId}/giveaways`
  return (value, context) => {
    bind(context, scope, path)
    const result = pageSchema.parse(value) as Page<Giveaway>
    ordered(result.items, row => row.giveaway_id, Number(query.limit ?? 20))
    assert(result.next_cursor === null || result.next_cursor !== query.cursor)
    result.items.forEach(row => {
      gift(row, scope, false)
      assert((query.state === undefined || row.state === query.state)
        && (query.in_stock === undefined || (row.available > 0) === query.in_stock)
        && (query.public_place_id === undefined || row.public_place_id === query.public_place_id)
        && (query.pickup_after === undefined || Date.parse(row.pickup_starts_at) >= Date.parse(String(query.pickup_after)))
        && (query.pickup_before === undefined || Date.parse(row.pickup_starts_at) < Date.parse(String(query.pickup_before))))
    })
    return freezeGift(result)
  }
}
export function giveawayDecoder(scope: SessionScope, id: string): Decoder<Giveaway> {
  const path = `/api/concert/events/${scope.eventId}/giveaways/${id}`
  return (value, context) => {
    bind(context, scope, path); assert(giftId.test(id))
    const result = giftSchema.parse(value) as Giveaway
    gift(result, scope, false); assert(result.giveaway_id === id)
    return freezeGift(result)
  }
}
export function giftActivityDecoder(scope: SessionScope, query: GiftQuery): Decoder<GiftActivity> {
  const path = `/api/concert/events/${scope.eventId}/me/giveaways`
  return (value, context) => {
    bind(context, scope, path)
    const result = activitySchema.parse(value) as GiftActivity, limit = Number(query.limit ?? 20)
    ordered(result.giving.items, row => row.giveaway_id, limit)
    ordered(result.receiving.items, row => row.reservation_id, limit)
    result.giving.items.forEach(row => gift(row, scope, true))
    result.receiving.items.forEach(row => reservation(row, scope))
    const activeTargets = result.receiving.items.filter(row => row.state === 'HELD' || row.state === 'READY').map(row => row.giveaway_id)
    assert(new Set(activeTargets).size === activeTargets.length
      && (result.giving.next_cursor === null || result.giving.next_cursor !== query.giving_cursor)
      && (result.receiving.next_cursor === null || result.receiving.next_cursor !== query.receiving_cursor))
    return freezeGift(result)
  }
}
