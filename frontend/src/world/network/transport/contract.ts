import { z } from 'zod'
import world from '../../../../../contracts/realtime/v1/world.schema.json'
import core from '../../../../../contracts/core/v1/core.schema.json'
import avatar from '../../../../../contracts/assets/v1/avatar-config.schema.json'
import common from '../../../../../contracts/assets/v1/common.schema.json'
import policy from '../../../../../contracts/realtime/v1/policy.json'
import type { ClientMessage, ServerMessage, WorldTicketResponse } from './wire-types'
export { policy }
export class WorldTransportError extends Error {
  constructor(readonly kind: 'invalid-frame' | 'invalid-context' | 'unavailable' | 'timeout' | 'cancelled' | 'backpressure' | 'rate-limited' | 'closed' | 'disposed', readonly code = 4400) {
    super(kind); this.name = 'WorldTransportError'
  }
}
export function ensure(value: unknown, code = 4400): asserts value {
  if (!value) throw new WorldTransportError('invalid-frame', code)
}
type Schema = { [key: string]: unknown }
const record = (v: unknown): Schema => { ensure(v && typeof v === 'object' && !Array.isArray(v)); return v as Schema }
const docs = [world, core, avatar, common] as { $id: string; $defs: Record<string, unknown> }[]
const cache = new Map<string, (value: unknown) => boolean>()
const keywords = new Set(['$ref', 'type', 'const', 'enum', 'properties', 'required', 'additionalProperties', 'items', 'minItems', 'maxItems', 'uniqueItems', 'minLength', 'maxLength', 'pattern', 'format', 'minimum', 'maximum', 'anyOf', 'oneOf'])
function compile(input: unknown, doc: typeof docs[number]): (v: unknown) => boolean {
  const s = record(input)
  ensure(Object.keys(s).every(k => keywords.has(k) || ['description', 'title'].includes(k)))
  const ref = s.$ref as string | undefined
  let referenced: ((v: unknown) => boolean) | undefined
  if (ref) {
    const [id, pointer] = ref.split('#'), target = id ? docs.find(d => d.$id === id) : doc
    ensure(target && pointer?.startsWith('/$defs/'))
    const key = pointer!.slice(7), identity = `${target.$id}#${pointer}`
    ensure(Object.hasOwn(target.$defs, key))
    referenced = cache.get(identity)
    if (!referenced) { referenced = compile(target.$defs[key], target); cache.set(identity, referenced) }
  }
  const props = s.properties ? Object.fromEntries(Object.entries(record(s.properties)).map(([k,v]) => [k, compile(v, doc)])) : {}
  const choices = (s.oneOf ?? s.anyOf) as unknown[] | undefined
  const checks = choices?.map(v => compile(v, doc))
  const item = s.items ? compile(s.items, doc) : undefined
  const pattern = s.pattern ? new RegExp(s.pattern as string, 'u') : undefined
  ensure(!s.format || ['uuid', 'date-time'].includes(s.format as string))
  ensure(!s.type || ['object', 'array', 'number', 'integer', 'boolean', 'null', 'string'].includes(s.type as string))
  ensure(s.additionalProperties === undefined || typeof s.additionalProperties === 'boolean')
  return v => {
    if (referenced && !referenced(v)) return false
    if ('const' in s && v !== s.const) return false
    if (s.enum && !(s.enum as unknown[]).includes(v)) return false
    if (s.type === 'null' && v !== null) return false
    if (s.type === 'boolean' && typeof v !== 'boolean') return false
    if (s.type === 'string' && typeof v !== 'string') return false
    if (s.type === 'number' && typeof v !== 'number') return false
    if (s.type === 'integer' && !Number.isSafeInteger(v)) return false
    if (s.type === 'array' && !Array.isArray(v)) return false
    if (s.type === 'object' && (!v || typeof v !== 'object' || Array.isArray(v))) return false
    if (typeof v === 'number' && ((s.minimum !== undefined && v < Number(s.minimum)) || (s.maximum !== undefined && v > Number(s.maximum)))) return false
    if (typeof v === 'string') {
      const n = Array.from(v).length
      if ((s.minLength !== undefined && n < Number(s.minLength)) || (s.maxLength !== undefined && n > Number(s.maxLength)) || (pattern && !pattern.test(v))) return false
      if (s.format === 'uuid' && !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v)) return false
      if (s.format === 'date-time' && !z.iso.datetime({ offset: true }).safeParse(v).success) return false
    }
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      const r = v as Schema
      if ((s.required as string[] | undefined)?.some(k => !Object.hasOwn(r,k))) return false
      for (const [k, value] of Object.entries(r)) {
        if (Object.hasOwn(props,k)) { if (!props[k]!(value)) return false }
        else if (s.additionalProperties === false) return false
      }
    }
    if (Array.isArray(v)) {
      if ((s.minItems !== undefined && v.length < Number(s.minItems)) || (s.maxItems !== undefined && v.length > Number(s.maxItems))) return false
      if (item && !v.every(item)) return false
      if (s.uniqueItems && new Set(v.map(e => JSON.stringify(e))).size !== v.length) return false
    }
    if (checks) { const count = checks.filter(c => c(v)).length; if (s.oneOf ? count !== 1 : count === 0) return false }
    return true
  }
}
const validators = new Map<string, z.ZodType>()
export function validate<T = unknown>(name: string, value: unknown): T {
  let validator = validators.get(name)
  if (!validator) {
    ensure(Object.hasOwn(world.$defs, name))
    const check = compile({ $ref: `#/$defs/${name}` }, docs[0]!)
    validator = z.custom(check); validators.set(name, validator)
  }
  inspect(value)
  ensure(validator.safeParse(value).success)
  const v = record(value)
  if (v.type === 'input') ensure(Number(v.axisX) ** 2 + Number(v.axisY) ** 2 <= 1 + 1e-9)
  if (v.type === 'snapshot' || v.type === 'delta') {
    const rows = (v.players ?? v.upserts) as { userId: string }[]
    const ids = rows.map(r => r.userId)
    ensure(new Set(ids).size === ids.length)
    if (v.type === 'snapshot') {
      ensure(ids.includes(v.selfUserId as string))
      ensure((v.requestId !== null) === ['RESYNC', 'ROOM_SWITCH'].includes(v.cause as string))
    } else ensure(!(v.removes as string[]).some(id => ids.includes(id)))
  }
  if (name === 'WorldTicketResponse') {
    const ttl = Date.parse(v.expires_at as string) - Date.parse(v.server_time as string)
    ensure(ttl > 0 && ttl <= policy.ticket_ttl_ms)
  }
  return value as T
}
const wellFormed = (value: string) => Array.from(value).every(c => { const n = c.codePointAt(0)!; return n < 0xd800 || n > 0xdfff })
function inspect(value: unknown, depth = 0): void {
  ensure(depth <= 16)
  if (typeof value === 'number') ensure(Number.isFinite(value) && Math.abs(value) <= Number.MAX_SAFE_INTEGER)
  else if (typeof value === 'string') ensure(wellFormed(value), 1007)
  else if (Array.isArray(value)) value.forEach(v => inspect(v, depth + 1))
  else if (value && typeof value === 'object') for (const [k,v] of Object.entries(value)) { inspect(k, depth + 1); inspect(v, depth + 1) }
  else ensure(value === null || typeof value === 'boolean')
}
// Parse rather than sanitize: escaped duplicate keys must fail before JSON.parse loses them.
function parse(text: string): unknown {
  let i = 0
  const space = () => { while (/[\x20\t\r\n]/.test(text[i] ?? '') && i < text.length) i++ }
  function string(): string {
    const start = i++
    while (i < text.length) { const c = text[i++]; if (c === '\\') i++; else if (c === '"') return JSON.parse(text.slice(start,i)) as string }
    throw new WorldTransportError('invalid-frame')
  }
  function value(depth: number): unknown {
    ensure(depth <= 16); space()
    const c = text[i]
    if (c === '"') return string()
    if (c === '{') {
      i++; space(); const out: Schema = Object.create(null)
      if (text[i] === '}') { i++; return out }
      while (i < text.length) {
        ensure(text[i] === '"'); const key = string(); ensure(!Object.hasOwn(out,key)); space(); ensure(text[i++] === ':')
        out[key] = value(depth + 1); space(); const end = text[i++]; if (end === '}') return out; ensure(end === ','); space()
      }
    }
    if (c === '[') {
      i++; space(); const out: unknown[] = []
      if (text[i] === ']') { i++; return out }
      while (i < text.length) { out.push(value(depth + 1)); space(); const end = text[i++]; if (end === ']') return out; ensure(end === ',') }
    }
    const match = /^(?:true|false|null|-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?)/.exec(text.slice(i))
    ensure(match); i += match[0].length; return JSON.parse(match[0]) as unknown
  }
  const result = value(0); space(); ensure(i === text.length); return result
}
export function decode(payload: unknown, direction: 'client' | 'server'): ClientMessage | ServerMessage {
  ensure(typeof payload === 'string', 1003)
  ensure(payload.length <= policy[`${direction}_message_bytes`], 1009)
  ensure(wellFormed(payload), 1007)
  ensure(new TextEncoder().encode(payload).length <= policy[`${direction}_message_bytes`], 1009)
  try { return validate(direction === 'client' ? 'ClientMessage' : 'ServerMessage', parse(payload)) }
  catch (error) { if (error instanceof WorldTransportError) throw error; throw new WorldTransportError('invalid-frame') }
}
export const decodeTicket = (value: unknown) => validate<WorldTicketResponse>('WorldTicketResponse', value)
