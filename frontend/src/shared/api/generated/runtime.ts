// Generated public C07 runtime. No storage, application imports, or server authority.
import { z } from 'zod'
import { document } from './document.js'
import { uriPattern } from './formats.js'
import type { Schemas, Operations } from './types.js'
type Rule = boolean | Record<string, any>
const doc: any = document
const uri = new RegExp(uriPattern)
const object = (v: unknown): v is Record<string, any> => v !== null && typeof v === 'object' && !Array.isArray(v)
const patterns = new Map<string, RegExp>()
function patternAccepts(source: string, value: string): boolean {
  let compiled = patterns.get(source)
  if (!compiled) {
    let expression = '', escaped = false, inside = false
    for (const char of source) {
      if (escaped) { expression += char; escaped = false; continue }
      if (char === '\\') { expression += char; escaped = true; continue }
      if (char === '[') inside = true
      if (char === ']') inside = false
      expression += char === '$' && !inside ? '(?=\\n?(?![\\s\\S]))' : char
    }
    compiled = new RegExp(expression, 'u'); patterns.set(source, compiled)
  }
  return compiled.test(value)
}
function canonical(v: any): string {
  if (Array.isArray(v)) return `[${v.map(canonical).join(',')}]`
  if (object(v)) return `{${Object.keys(v).sort().map(k => `${JSON.stringify(k)}:${canonical(v[k])}`).join(',')}}`
  return JSON.stringify(v)
}
function resolve(ref: string): Rule {
  if (!ref.startsWith('#/')) throw new Error('C07 external reference refused')
  let node = doc
  for (const token of ref.slice(2).split('/')) {
    const key = token.replace(/~1/g, '/').replace(/~0/g, '~')
    if (!Object.hasOwn(node, key)) throw new Error('C07 unresolved reference')
    node = node[key]
  }
  return node
}
function date(value: string): boolean {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value)
  if (!m || m[0] !== value) return false
  const y = Number(m[1]), month = Number(m[2]), day = Number(m[3])
  if (y < 1) return false
  const d = new Date(0); d.setUTCFullYear(y, month - 1, day)
  return d.getUTCFullYear() === y && d.getUTCMonth() === month - 1 && d.getUTCDate() === day
}
function format(kind: string, value: string): boolean {
  if (kind === 'uuid') return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}(?![\s\S])/i.test(value)
  if (kind === 'date') return date(value)
  if (kind === 'date-time') {
    const input = value.endsWith('\n') ? value.slice(0, -1) : value
    const m = /^(\d{4}-\d{2}-\d{2})[Tt](\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(?:[Zz]|([+-])(\d{2}):(\d{2}))$/.exec(input)
    return !!m && m[0] === input && date(m[1]!) && Number(m[2]) < 24 && Number(m[3]) < 60 && Number(m[4]) < 60 && Number(m[6] ?? 0) < 24 && Number(m[7] ?? 0) < 60
  }
  // Match the retained Python JSON Schema email annotation. Product policy is separate.
  if (kind === 'email') return value.includes('@')
  if (kind === 'uri') {
    return uri.test(value)
  }
  return kind === 'password' || kind === 'int64' || kind === 'binary'
}
function matches(s: Rule, v: any, depth: number, budget: { left: number }): boolean {
  if (--budget.left < 0 || depth > 96) return false
  if (typeof s === 'boolean') return s
  if (s.$ref && !matches(resolve(s.$ref), v, depth + 1, budget)) return false
  if (s.allOf && !s.allOf.every((r: Rule) => matches(r, v, depth + 1, budget))) return false
  if (s.anyOf && !s.anyOf.some((r: Rule) => matches(r, v, depth + 1, budget))) return false
  if (s.oneOf && s.oneOf.filter((r: Rule) => matches(r, v, depth + 1, budget)).length !== 1) return false
  if (s.if !== undefined) {
    const branch = matches(s.if, v, depth + 1, budget) ? s.then : s.else
    if (branch !== undefined && !matches(branch, v, depth + 1, budget)) return false
  }
  if (Object.hasOwn(s, 'const') && canonical(v) !== canonical(s.const)) return false
  if (s.enum && !s.enum.some((x: unknown) => canonical(x) === canonical(v))) return false
  if (s.type) {
    const kinds = Array.isArray(s.type) ? s.type : [s.type]
    if (!kinds.some((t: string) => t === 'null' ? v === null : t === 'object' ? object(v) : t === 'array' ? Array.isArray(v) : t === 'integer' ? Number.isInteger(v) : t === 'number' ? typeof v === 'number' && Number.isFinite(v) : typeof v === t)) return false
  }
  if (typeof v === 'number' && ((s.minimum !== undefined && v < s.minimum) || (s.maximum !== undefined && v > s.maximum))) return false
  if (typeof v === 'string') {
    const length = Array.from(v).length
    if ((s.minLength !== undefined && length < s.minLength) || (s.maxLength !== undefined && length > s.maxLength) || (s.pattern && !patternAccepts(s.pattern, v)) || (s.format && !format(s.format, v))) return false
    if (s['x-mcr-utf8-bytes'] !== undefined) {
      if (/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/u.test(v)) return false
      const bytes = new TextEncoder().encode(v).length, bounds = s['x-mcr-utf8-bytes']
      if (bytes < bounds.minimum || bytes > bounds.maximum) return false
    }
  }
  if (Array.isArray(v)) {
    if ((s.minItems !== undefined && v.length < s.minItems) || (s.maxItems !== undefined && v.length > s.maxItems) || (s.uniqueItems && new Set(v.map(canonical)).size !== v.length)) return false
    if (s.items !== undefined && !v.every(x => matches(s.items, x, depth + 1, budget))) return false
  }
  if (object(v)) {
    if (s.minProperties !== undefined && Object.keys(v).length < s.minProperties) return false
    if (s.required && !s.required.every((k: string) => Object.hasOwn(v, k))) return false
    for (const [key, value] of Object.entries(v)) {
      if (Object.hasOwn(s.properties ?? {}, key)) { if (!matches(s.properties[key], value, depth + 1, budget)) return false }
      else if (s.additionalProperties === false) return false
      else if (object(s.additionalProperties) && !matches(s.additionalProperties, value, depth + 1, budget)) return false
    }
  }
  return true
}
function json(value: unknown, seen = new Set<object>(), depth = 0): boolean {
  if (depth > 96) return false
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true
  if (typeof value === 'number') return Number.isFinite(value)
  if (typeof value !== 'object' || seen.has(value)) return false
  if (!Array.isArray(value) && Object.getPrototypeOf(value) !== Object.prototype && Object.getPrototypeOf(value) !== null) return false
  seen.add(value); const valid = Object.values(value).every(v => json(v, seen, depth + 1)); seen.delete(value); return valid
}
export function accepts(rule: Rule, value: unknown): boolean {
  try { return json(value) && matches(rule, value, 0, { left: 100000 }) } catch { return false }
}
export function schema<N extends keyof Schemas>(name: N): z.ZodType<Schemas[N]> {
  if (!Object.hasOwn(doc.components.schemas, name)) throw new Error('C07 unknown schema')
  return z.custom<Schemas[N]>(value => accepts(doc.components.schemas[name], value), { message: `C07 invalid ${name}` })
}
export const schemas = Object.fromEntries(Object.keys(doc.components.schemas).map(name => [name, schema(name as keyof Schemas)])) as { [N in keyof Schemas]: z.ZodType<Schemas[N]> }
export class ContractError extends Error {
  constructor(readonly operation: string, readonly phase: 'request' | 'response' | 'transport') { super(`C07 ${phase} contract failed: ${operation}`); this.name = 'ContractError' }
}
export interface ClientOptions {
  baseUrl: string
  fetch?: typeof globalThis.fetch
  getAccessToken?: () => string | undefined
  credentials?: RequestCredentials
}
export interface CallOptions { signal?: AbortSignal }
function deref(value: any): any { return value?.$ref ? resolve(value.$ref) : value }
function parse(rule: Rule, value: unknown, id: string, phase: 'request' | 'response'): void {
  if (!accepts(rule, value)) throw new ContractError(id, phase)
}
export function createClient(options: ClientOptions) {
  const base = new URL(options.baseUrl)
  if (!/^https?:$/.test(base.protocol) || base.username || base.password || base.search || base.hash || base.pathname !== '/') throw new Error('C07 baseUrl must be an HTTP origin')
  const send = options.fetch ?? globalThis.fetch
  if (!send) throw new Error('C07 fetch unavailable')
  async function call<K extends keyof Operations>(id: K, input: Operations[K]['request'], control: CallOptions = {}): Promise<Operations[K]['response']> {
    const op = doc.operations[id]
    if (!op || !object(input)) throw new ContractError(String(id), 'request')
    const args: any = input
    if (Object.keys(args).some(k => !['path', 'query', 'headers', 'body', 'mediaType'].includes(k))) throw new ContractError(String(id), 'request')
    let path: string = op.path
    const query = new URLSearchParams(), headers = new Headers()
    for (const group of ['path', 'query', 'headers']) {
      const parameters = op.parameters.filter((p: any) => (p.in === 'header' ? 'headers' : p.in) === group)
      if (args[group] !== undefined && !object(args[group])) throw new ContractError(String(id), 'request')
      for (const name of Object.keys(args[group] ?? {})) if (!parameters.some((p: any) => p.name === name)) throw new ContractError(String(id), 'request')
      for (const parameter of parameters) {
        const value = args[group]?.[parameter.name]
        if (value === undefined) { if (parameter.required) throw new ContractError(String(id), 'request'); continue }
        parse(parameter.schema, value, String(id), 'request')
        if (group === 'path') path = path.replace(`{${parameter.name}}`, encodeURIComponent(String(value)))
        else if (group === 'headers') headers.set(parameter.name, String(value))
        else if (Array.isArray(value)) for (const item of value) query.append(parameter.name, String(item))
        else query.append(parameter.name, String(value))
      }
    }
    if (/\{[^}]+\}/.test(path)) throw new ContractError(String(id), 'request')
    let body: BodyInit | undefined
    const media = args.mediaType ?? Object.keys(op.requestBody?.content ?? {})[0]
    if (args.mediaType !== undefined && !op.requestBody?.content?.[args.mediaType]) throw new ContractError(String(id), 'request')
    if (args.body !== undefined) {
      const rule = op.requestBody?.content?.[media]?.schema
      if (!rule) throw new ContractError(String(id), 'request')
      if (media.startsWith('image/')) { if (!(args.body instanceof Blob) || args.body.type !== media) throw new ContractError(String(id), 'request'); body = args.body }
      else { parse(rule, args.body, String(id), 'request'); body = media === 'application/x-www-form-urlencoded' ? new URLSearchParams(args.body) : JSON.stringify(args.body) }
      headers.set('Content-Type', media)
    } else if (op.requestBody?.required) throw new ContractError(String(id), 'request')
    const token = options.getAccessToken?.()
    if (op.security?.some((s: any) => Object.hasOwn(s, 'BearerAuth')) && token) headers.set('Authorization', `Bearer ${token}`)
    let response: Response
    try { response = await send(new URL(path + (query.size ? `?${query}` : ''), base), { method: op.method, headers, ...(body !== undefined ? { body } : {}), credentials: options.credentials ?? 'same-origin', redirect: 'manual', ...(control.signal ? { signal: control.signal } : {}) }) }
    catch (error) { if (control.signal?.aborted) throw error; throw new ContractError(String(id), 'transport') }
    const spec = op.responses[String(response.status)] ?? op.responses.default
    if (!spec) throw new ContractError(String(id), 'response')
    const actualHeaders: Record<string, string> = {}
    for (const [name, raw] of Object.entries(spec.headers ?? {})) {
      if (name.toLowerCase() === 'set-cookie') continue // Browser-owned HttpOnly credentials.
      const definition = deref(raw), value = response.headers.get(name)
      if (value !== null) {
        const rule = deref(definition.schema)
        const decoded = rule.type === 'integer' && /^-?(?:0|[1-9][0-9]*)$/.test(value) ? Number(value) : rule.type === 'boolean' && /^(?:true|false)$/.test(value) ? value === 'true' : value
        parse(definition.schema, decoded, String(id), 'response'); actualHeaders[name] = value
      }
      else if (definition.required || (response.status === 302 && name.toLowerCase() === 'location')) throw new ContractError(String(id), 'response')
    }
    const content = spec.content ?? {}
    if (Object.keys(content).length === 0) {
      if ((await response.text()) !== '') throw new ContractError(String(id), 'response')
      return { status: response.status, mediaType: null, body: undefined, headers: actualHeaders } as Operations[K]['response']
    }
    const actualMedia = response.headers.get('content-type')?.split(';')[0]?.trim().toLowerCase()
    if (!actualMedia || !Object.hasOwn(content, actualMedia)) throw new ContractError(String(id), 'response')
    let value: unknown
    try { value = actualMedia.startsWith('image/') ? await response.blob() : actualMedia === 'application/json' ? await response.json() : await response.text() } catch { throw new ContractError(String(id), 'response') }
    if (!actualMedia.startsWith('image/')) parse(content[actualMedia].schema, value, String(id), 'response')
    return { status: response.status, mediaType: actualMedia, body: value, headers: actualHeaders } as Operations[K]['response']
  }
  return { call, operations: Object.fromEntries(Object.keys(doc.operations).map(id => [id, (input: any, control?: CallOptions) => call(id as keyof Operations, input, control)])) as { [K in keyof Operations]: (input: Operations[K]['request'], control?: CallOptions) => Promise<Operations[K]['response']> } }
}
