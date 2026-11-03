import { z } from 'zod'
import contract from '../../../../../contracts/generated/mcr-core.openapi.json'

export type ObjectValue = Record<string, unknown>
export const object = (value: unknown): ObjectValue => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid contract value')
  return value as ObjectValue
}
export const list = (value: unknown): unknown[] => { if (!Array.isArray(value)) throw new Error('Invalid list'); return value }
export const text = (value: unknown) => { if (typeof value !== 'string') throw new Error('Invalid text'); return value }
export const number = (value: unknown) => { if (!Number.isSafeInteger(value)) throw new Error('Invalid integer'); return value as number }
export function requireValue(condition: unknown): asserts condition { if (!condition) throw new Error('Invalid contract response') }
const wellFormed = (value: string) => Array.from(value).every(c => { const n = c.codePointAt(0)!; return n < 0xd800 || n > 0xdfff })
export const canonicalUuid = (value: unknown): value is string => typeof value === 'string'
  && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value)
export const equal = (a: unknown, b: unknown): boolean => {
  if (a === b) return true
  if (Array.isArray(a) && Array.isArray(b)) return a.length === b.length && a.every((v, i) => equal(v, b[i]))
  if (a && b && typeof a === 'object' && typeof b === 'object') {
    const left = object(a), right = object(b)
    return Object.keys(left).length === Object.keys(right).length && Object.keys(left).every(k => Object.hasOwn(right, k) && equal(left[k], right[k]))
  }
  return false
}
// C06 canonical JSON matches validate-assets.py: sorted keys, ASCII escapes, integers only.
export function canonicalJson(value: unknown): string {
  if (value === null || typeof value === 'boolean') return String(value)
  if (typeof value === 'number') { requireValue(Number.isSafeInteger(value)); return String(value) }
  if (typeof value === 'string') return JSON.stringify(value).replace(/[\u007f-\uffff]/g, c => `\\u${c.charCodeAt(0).toString(16).padStart(4, '0')}`)
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  const record = object(value)
  return `{${Object.keys(record).sort().map(k => `${canonicalJson(k)}:${canonicalJson(record[k])}`).join(',')}}`
}
const schemas: Record<string, unknown> = contract.components.schemas
// Deliberately limited to the keywords reachable from C03 AuthResponse, LoginRequest,
// AuthCapabilities and Bootstrap. This is not a general JSON Schema implementation.
const metadata = new Set(['description', 'example', 'readOnly'])
const supported = new Set(['$ref', 'type', 'const', 'enum', 'properties', 'required', 'additionalProperties',
  'items', 'minItems', 'maxItems', 'uniqueItems', 'minLength', 'maxLength', 'pattern', 'format',
  'minimum', 'maximum', 'anyOf', 'allOf', 'if', 'then', 'else'])
type Check = (value: unknown, budget: { remaining: number }, depth: number) => boolean
const compiled = new Map<string, Check>()
const compiling = new Set<string>()
function compile(input: unknown): Check {
  const schema = object(input)
  for (const key of Object.keys(schema)) requireValue(supported.has(key) || metadata.has(key))
  const children = schema.properties ? Object.fromEntries(Object.entries(object(schema.properties)).map(([k, v]) => [k, compile(v)])) : null
  const required = schema.required ? list(schema.required).map(text) : []
  const ref = schema.$ref ? text(schema.$ref) : null
  if (ref) requireValue(/^#\/components\/schemas\/[A-Za-z0-9]+$/.test(ref) && Object.hasOwn(schemas, ref.split('/').at(-1)!))
  const referenced = ref ? compileName(ref.split('/').at(-1)!) : null
  const alternatives = schema.anyOf ? list(schema.anyOf).map(compile) : null
  const intersections = schema.allOf ? list(schema.allOf).map(compile) : []
  const conditional = schema.if ? compile(schema.if) : null
  const yes = schema.then ? compile(schema.then) : null, no = schema.else ? compile(schema.else) : null
  const item = schema.items ? compile(schema.items) : null
  const pattern = schema.pattern ? new RegExp(text(schema.pattern), 'u') : null
  const format = schema.format ? text(schema.format) : null
  requireValue(!format || ['uuid', 'email', 'date-time', 'password', 'int64', 'uri'].includes(format))
  requireValue(schema.additionalProperties === undefined || typeof schema.additionalProperties === 'boolean')
  requireValue(schema.type === undefined || ['object', 'array', 'string', 'integer', 'boolean', 'null'].includes(text(schema.type)))
  return (value, budget, depth) => {
    if (--budget.remaining < 0 || depth > 40) return false
    const apply = (check: Check) => check(value, budget, depth + 1)
    if (referenced && !apply(referenced)) return false
    if (Object.hasOwn(schema, 'const') && !equal(value, schema.const)) return false
    if (schema.enum && !list(schema.enum).some(v => equal(v, value))) return false
    if (schema.type === 'null' && value !== null) return false
    if (schema.type === 'boolean' && typeof value !== 'boolean') return false
    if (schema.type === 'string' && typeof value !== 'string') return false
    if (schema.type === 'integer' && !Number.isSafeInteger(value)) return false
    if (schema.type === 'array' && !Array.isArray(value)) return false
    if (schema.type === 'object' && (!value || typeof value !== 'object' || Array.isArray(value))) return false
    if (typeof value === 'number' && ((schema.minimum !== undefined && value < number(schema.minimum))
      || (schema.maximum !== undefined && value > number(schema.maximum)))) return false
    if (typeof value === 'string') {
      if (!wellFormed(value)) return false
      const length = Array.from(value).length
      if ((schema.minLength !== undefined && length < number(schema.minLength))
        || (schema.maxLength !== undefined && length > number(schema.maxLength)) || (pattern && !pattern.test(value))) return false
      if (format === 'uuid' && !canonicalUuid(value)) return false
      if (format === 'uri' && !z.url().safeParse(value).success) return false
      if (format === 'email' && !z.email().safeParse(value).success) return false
      if (format === 'date-time' && !z.iso.datetime({ offset: true }).safeParse(value).success) return false
    }
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      const record = object(value)
      if (required.some(k => !Object.hasOwn(record, k))) return false
      if (!children && schema.additionalProperties === false && Object.keys(record).length) return false
      if (children) for (const [key, entry] of Object.entries(record)) {
        if (Object.hasOwn(children, key)) { if (!children[key]!(entry, budget, depth + 1)) return false }
        else if (schema.additionalProperties === false) return false
      }
    }
    if (Array.isArray(value)) {
      if ((schema.minItems !== undefined && value.length < number(schema.minItems))
        || (schema.maxItems !== undefined && value.length > number(schema.maxItems))) return false
      if (item && !value.every(v => item(v, budget, depth + 1))) return false
      if (schema.uniqueItems === true && new Set(value.map(canonicalJson)).size !== value.length) return false
    }
    if (alternatives && !alternatives.some(apply)) return false
    if (!intersections.every(apply)) return false
    if (conditional) { const branch = apply(conditional) ? yes : no; if (branch && !apply(branch)) return false }
    return budget.remaining >= 0
  }
}
function compileName(name: string): Check {
  const prior = compiled.get(name)
  if (prior) return prior
  // No recursive definitions in the frozen reachable schema; reject a future cycle.
  requireValue(!compiling.has(name)); compiling.add(name)
  try { const check = compile(schemas[name]); compiled.set(name, check); return check }
  finally { compiling.delete(name) }
}
export function decodeContract(name: 'AuthResponse' | 'AuthCapabilities' | 'Bootstrap' | 'LoginRequest', value: unknown): ObjectValue {
  requireValue(compileName(name)(value, { remaining: 200_000 }, 0))
  return object(value)
}
export function decodeAuth(value: unknown) {
  const auth = decodeContract('AuthResponse', value)
  const keys = Object.keys(object(schemas.AuthResponse).properties as object)
  requireValue(Object.keys(auth).every(k => keys.includes(k)))
  requireValue(auth.tokenType === 'Bearer' && auth.refreshToken === '' && typeof auth.accessToken === 'string'
    && auth.accessToken.length <= 8192 && /^[A-Za-z0-9._~+/-]+=*$/.test(auth.accessToken))
  requireValue(new Set(list(auth.linkedProviders)).size === list(auth.linkedProviders).length)
  return { userId: text(auth.userId), accessToken: text(auth.accessToken), expiresIn: number(auth.expiresIn) }
}
export function decodeCapabilities(value: unknown) {
  const result = decodeContract('AuthCapabilities', value)
  const providers = list(result.providers).map(object)
  requireValue(providers.length === 4 && new Set(providers.map(p => p.provider)).size === 4)
  return Object.freeze({ email: providers.some(p => p.provider === 'EMAIL' && p.enabled === true) })
}
export function validLogin(email: string, password: string): boolean {
  // Login accepts existing 1–128-byte passwords. Signup/reset's 8-byte minimum is a different operation.
  const bytes = new TextEncoder().encode(password).length
  if (!wellFormed(password) || bytes < 1 || bytes > 128) return false
  try { decodeContract('LoginRequest', { email, password }); return true } catch { return false }
}
