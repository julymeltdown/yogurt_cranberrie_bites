import contract from '../../../../contracts/generated/mcr-domain.openapi.json'
import type { Decoder } from '../../shared/api/http'

export const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/
export type Filter = 'ALL' | 'WITH_MEDIA' | 'PARTY_LINK' | 'GIVEAWAY_LINK'
export interface Author {
  readonly user_id: string; readonly display_name: string; readonly bio: string | null
  readonly avatar: { readonly revision: number; readonly config: {
    readonly contract_version: '1.0.0'; readonly kind: 'avatar_config'; readonly manifest_id: string
    readonly manifest_version: number; readonly skin_tone_id: string
    readonly item_ids: Readonly<{ body: string; eyes: string; hair: string; outfit: string; accessory: string | null }>
  } }
}
export interface ResourceLink { readonly target_type: 'PARTY' | 'GIVEAWAY' | 'POST' | 'INTRO' | 'MURAL' | 'NOTICE'; readonly target_id: string }
export interface Post {
  readonly post_id: string; readonly event_id: string; readonly author: Author; readonly text: string
  readonly media_ids: readonly string[]; readonly links: readonly ResourceLink[]; readonly created_at: string
  readonly edited_at: string | null; readonly comment_count: number; readonly reaction_count: number
  readonly reacted_by_me: boolean; readonly state: 'PUBLISHED'; readonly revision: number
}
export interface Notice {
  readonly notice_id: string; readonly event_id: string; readonly title: string; readonly text: string
  readonly source_uri: string; readonly confirmed_at: string; readonly created_at: string; readonly state: 'PUBLISHED'; readonly revision: number
}
export interface Comment { readonly comment_id: string; readonly post_id: string; readonly author: Author; readonly text: string; readonly created_at: string; readonly revision: number }
export interface Page<T> { readonly items: readonly T[]; readonly next_cursor: string | null }
export interface PostDetail { readonly post: Post; readonly comments: readonly Comment[]; readonly next_cursor: string | null }
export interface Reaction { readonly post_id: string; readonly reacted_by_me: boolean; readonly reaction_count: number; readonly revision: number }
export interface Deleted { readonly resource_id: string; readonly revision: number; readonly state: 'DELETED' }
export interface Receipt {
  readonly receipt_id: string; readonly event_id: string | null; readonly actor_id: string; readonly operation_id: string
  readonly idempotency_key: string; readonly state: 'PENDING' | 'SUCCEEDED' | 'FAILED' | 'RETIRED'
  readonly resource_id: string | null; readonly resource_revision: number | null; readonly failure_reason: string | null
  readonly created_at: string; readonly expires_at: string; readonly result_status: number | null
}
type Rule = { readonly [key: string]: unknown }
const schemas: Record<string, unknown> = contract.components.schemas
const keywords = new Set(['type', '$ref', 'const', 'enum', 'properties', 'required', 'additionalProperties', 'items', 'minItems', 'maxItems', 'uniqueItems', 'minLength', 'maxLength', 'pattern', 'format', 'minimum', 'maximum', 'anyOf'])
function record(value: unknown): value is Rule { return value !== null && typeof value === 'object' && !Array.isArray(value) }
function valid(rule: unknown, value: unknown, depth = 0): boolean {
  if (!record(rule) || depth > 30 || !Object.keys(rule).every(k => keywords.has(k))) return false
  if (typeof rule.$ref === 'string') {
    if (!/^#\/components\/schemas\/[A-Za-z0-9_]+$/.test(rule.$ref)) return false
    return valid(schemas[rule.$ref.split('/').at(-1)!], value, depth + 1)
  }
  if (Array.isArray(rule.anyOf)) return rule.anyOf.some(r => valid(r, value, depth + 1))
  if (Object.hasOwn(rule, 'const')) return value === rule.const
  if (Array.isArray(rule.enum)) return rule.enum.includes(value)
  if (rule.type === 'null') return value === null
  if (rule.type === 'boolean') return typeof value === 'boolean'
  if (rule.type === 'integer') return Number.isSafeInteger(value) && typeof value === 'number' && value >= Number(rule.minimum ?? Number.MIN_SAFE_INTEGER) && value <= Number(rule.maximum ?? Number.MAX_SAFE_INTEGER)
  if (rule.type === 'string') {
    if (typeof value !== 'string' || Array.from(value).length < Number(rule.minLength ?? 0) || Array.from(value).length > Number(rule.maxLength ?? 10000)
      || (typeof rule.pattern === 'string' && !new RegExp(rule.pattern, 'u').test(value))) return false
    if (rule.format === 'uuid') return uuid.test(value)
    if (rule.format === 'date-time') return /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/.test(value) && Number.isFinite(Date.parse(value)) && new Date(value).toISOString().slice(0, 19) === value.slice(0, 19)
    if (rule.format === 'uri') { try { const url = new URL(value); return url.protocol === 'https:' && !url.username && !url.password } catch { return false } }
    return rule.format === undefined
  }
  if (rule.type === 'array') return Array.isArray(value) && value.length >= Number(rule.minItems ?? 0) && value.length <= Number(rule.maxItems ?? 100)
    && (rule.uniqueItems !== true || new Set(value.map(v => JSON.stringify(v))).size === value.length) && value.every(v => valid(rule.items, v, depth + 1))
  if (rule.type === 'object') {
    if (!record(value) || !record(rule.properties) || rule.additionalProperties !== false || !Array.isArray(rule.required)) return false
    const properties = rule.properties
    return rule.required.every(k => typeof k === 'string' && Object.hasOwn(value, k)) && Object.keys(value).every(k => Object.hasOwn(properties, k) && valid(properties[k], value[k], depth + 1))
  }
  return false
}
export function freeze<T>(value: T): T {
  if (value && typeof value === 'object') { Object.values(value).forEach(v => freeze(v)); Object.freeze(value) }
  return value
}
/** Validate the pinned generated public schemas, including rejection of private/unknown fields. */
export function parse<T>(name: string, value: unknown): T {
  if (!Object.hasOwn(schemas, name) || !valid(schemas[name], value)) throw new Error('Invalid feed response')
  return freeze(structuredClone(value)) as T
}
export function decoder<T>(name: string, check: (data: T) => boolean): Decoder<T> {
  return value => { const data = parse<T>(name, value); if (!check(data)) throw new Error('Wrong feed context'); return data }
}
function compareTime(a: string, b: string): number {
  const seconds = a.slice(0, 19).localeCompare(b.slice(0, 19))
  if (seconds) return seconds
  const fraction = (time: string) => time.slice(19, -1).replace(/^\./, '')
  const af = fraction(a), bf = fraction(b), width = Math.max(af.length, bf.length)
  return af.padEnd(width, '0').localeCompare(bf.padEnd(width, '0'))
}
export function ordered<T extends { readonly created_at: string }>(items: readonly T[], id: (row: T) => string): boolean {
  return new Set(items.map(id)).size === items.length && items.every((row, i) => {
    if (i === 0) return true
    const previous = items[i - 1]!, time = compareTime(previous.created_at, row.created_at)
    return time > 0 || (time === 0 && id(previous) > id(row))
  })
}
