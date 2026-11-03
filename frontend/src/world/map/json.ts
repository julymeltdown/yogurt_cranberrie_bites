/** C06 wire text parser: duplicate keys and lexical floats must be rejected BEFORE JSON.parse loses them. */
export class MapError extends Error {
  constructor(readonly reason: 'invalid' | 'unavailable' | 'load-failed' | 'timeout' | 'cancelled') { super(`Map resource ${reason}.`); this.name = 'MapError' }
}
export function requireMap(condition: unknown): asserts condition { if (!condition) throw new MapError('invalid') }
export function strictJson(text: string): unknown {
  requireMap(new TextEncoder().encode(text).length <= 1_048_576)
  let at = 0, nodes = 0
  const space = () => { while (/[\t\n\r ]/.test(text[at] ?? '\0')) at++ }
  function string(): string {
    requireMap(text[at] === '"'); const begin = at++
    while (at < text.length) {
      const char = text[at++]
      if (char === '\\') { at++; continue }
      if (char === '"') { const value: unknown = JSON.parse(text.slice(begin, at)); requireMap(typeof value === 'string' && value.length <= 65_536); return value }
    }
    throw new MapError('invalid')
  }
  function value(depth: number): unknown {
    requireMap(depth <= 32 && ++nodes <= 100_000); space()
    const char = text[at]
    if (char === '"') return string()
    if (char === '{') {
      at++; space(); const result: Record<string, unknown> = Object.create(null)
      if (text[at] === '}') { at++; return result }
      while (true) {
        space(); const key = string(); requireMap(!Object.hasOwn(result, key)); space(); requireMap(text[at++] === ':')
        result[key] = value(depth + 1); space(); const end = text[at++]; if (end === '}') return result
        requireMap(end === ',')
      }
    }
    if (char === '[') {
      at++; space(); const result: unknown[] = []
      if (text[at] === ']') { at++; return result }
      while (true) { result.push(value(depth + 1)); space(); const end = text[at++]; if (end === ']') return result; requireMap(end === ',') }
    }
    for (const [literal, result] of [['true', true], ['false', false], ['null', null]] as const) {
      if (text.startsWith(literal, at)) { at += literal.length; return result }
    }
    const number = /^-?(?:0|[1-9][0-9]*)/.exec(text.slice(at))?.[0]
    requireMap(number !== undefined); at += number.length
    requireMap(!/[.eE0-9]/.test(text[at] ?? '\0'))
    const result = Number(number); requireMap(Number.isSafeInteger(result)); return result
  }
  try { const result = value(0); space(); requireMap(at === text.length); return result } catch { throw new MapError('invalid') }
}
export const record = (value: unknown): Record<string, unknown> => { requireMap(value !== null && typeof value === 'object' && !Array.isArray(value)); return value as Record<string, unknown> }
const ascii = (text: string) => JSON.stringify(text).replace(/[\u007f-\uffff]/g, c => `\\u${c.charCodeAt(0).toString(16).padStart(4, '0')}`)
function keyOrder(a: string, b: string) {
  const left = Array.from(a, c => c.codePointAt(0)!), right = Array.from(b, c => c.codePointAt(0)!)
  for (let i = 0; i < Math.min(left.length, right.length); i++) if (left[i] !== right[i]) return left[i]! - right[i]!
  return left.length - right.length
}
export function canonicalJson(value: unknown, depth = 0): string {
  requireMap(depth <= 32)
  if (value === null || typeof value === 'boolean') return String(value)
  if (typeof value === 'number') { requireMap(Number.isSafeInteger(value)); return String(value) }
  if (typeof value === 'string') return ascii(value)
  if (Array.isArray(value)) return `[${value.map(v => canonicalJson(v, depth + 1)).join(',')}]`
  const object = record(value)
  return `{${Object.keys(object).sort(keyOrder).map(k => `${ascii(k)}:${canonicalJson(object[k], depth + 1)}`).join(',')}}`
}
export async function sha256(bytes: Uint8Array<ArrayBuffer>) {
  const hash = await crypto.subtle.digest('SHA-256', bytes)
  return `sha256:${Array.from(new Uint8Array(hash), n => n.toString(16).padStart(2, '0')).join('')}`
}
export const digest = (value: unknown) => sha256(new TextEncoder().encode(canonicalJson(value)))
export function contentHash(value: Record<string, unknown>) { return digest(Object.fromEntries(Object.entries(value).filter(([key]) => key !== 'content_hash'))) }
