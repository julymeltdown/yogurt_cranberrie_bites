import { validDateTime, validUri, validUuid } from './formats'
import common from '../../../../contracts/assets/v1/common.schema.json'
import map from '../../../../contracts/assets/v1/map-manifest.schema.json'
import theme from '../../../../contracts/assets/v1/theme-manifest.schema.json'
import { canonicalJson, record, requireMap } from './json'
const documents: Record<string, unknown> = { [common.$id]: common, [map.$id]: map, [theme.$id]: theme }
const keywords = new Set(['$id', '$schema', '$defs', 'title', 'description', '$ref', 'type', 'const', 'enum', 'properties', 'required', 'additionalProperties', 'items', 'minItems', 'maxItems', 'uniqueItems', 'minLength', 'maxLength', 'minimum', 'maximum', 'pattern', 'format', 'anyOf', 'allOf', 'if', 'then', 'else'])
/** Evaluates only the checked-in C06 common/map/theme keyword subset; local refs only, no network. */
export function schemaValid(input: unknown, kind: 'map' | 'theme'): boolean {
  let nodes = 0
  function check(value: unknown, definition: unknown, root: unknown, depth = 0): boolean {
    requireMap(++nodes <= 300_000 && depth <= 64)
    const s = record(definition)
    requireMap(Object.keys(s).every(k => keywords.has(k)))
    if (typeof s.$ref === 'string') {
      const [uri, fragment] = s.$ref.split('#'), document = uri ? documents[uri] : root
      requireMap(document && fragment && fragment.startsWith('/$defs/') && fragment.split('/').length === 3)
      const target = record(record(document).$defs)[fragment.split('/')[2]!]; requireMap(target)
      if (!check(value, target, document, depth + 1)) return false
    }
    if ('const' in s && canonicalJson(value) !== canonicalJson(s.const)) return false
    if (Array.isArray(s.enum) && !s.enum.some(v => canonicalJson(v) === canonicalJson(value))) return false
    if (s.type && !(s.type === 'object' ? value !== null && typeof value === 'object' && !Array.isArray(value)
      : s.type === 'array' ? Array.isArray(value) : s.type === 'integer' ? typeof value === 'number' && Number.isSafeInteger(value)
        : s.type === 'null' ? value === null : typeof value === s.type)) return false
    if (typeof value === 'number' && typeof s.minimum === 'number' && value < s.minimum) return false
    if (typeof value === 'number' && typeof s.maximum === 'number' && value > s.maximum) return false
    if (typeof value === 'string') {
      const length = Array.from(value).length
      if (typeof s.minLength === 'number' && length < s.minLength || typeof s.maxLength === 'number' && length > s.maxLength) return false
      if (typeof s.pattern === 'string' && !new RegExp(s.pattern).test(value)) return false
      if (s.format === 'uuid' && !validUuid(value)) return false
      if (s.format === 'date-time' && !validDateTime(value)) return false
      if (s.format === 'uri' && !validUri(value)) return false
      requireMap(!s.format || ['uuid', 'date-time', 'uri'].includes(s.format as string))
    }
    if (Array.isArray(value)) {
      if (typeof s.minItems === 'number' && value.length < s.minItems) return false
      if (typeof s.maxItems === 'number' && value.length > s.maxItems) return false
      if (s.uniqueItems && new Set(value.map(v => canonicalJson(v))).size !== value.length) return false
      if (s.items && !value.every(v => check(v, s.items, root, depth + 1))) return false
    }
    if (value !== null && typeof value === 'object' && !Array.isArray(value)) {
      const v = record(value), props = s.properties ? record(s.properties) : {}
      if (Array.isArray(s.required) && !s.required.every(k => typeof k === 'string' && Object.hasOwn(v, k))) return false
      if (s.additionalProperties === false && !Object.keys(v).every(k => Object.hasOwn(props, k))) return false
      if (!Object.entries(props).every(([k, p]) => !Object.hasOwn(v, k) || check(v[k], p, root, depth + 1))) return false
    }
    if (Array.isArray(s.anyOf) && !s.anyOf.some(p => check(value, p, root, depth + 1))) return false
    if (Array.isArray(s.allOf) && !s.allOf.every(p => check(value, p, root, depth + 1))) return false
    if (s.if) { const branch = check(value, s.if, root, depth + 1) ? s.then : s.else; if (branch && !check(value, branch, root, depth + 1)) return false }
    return true
  }
  try { canonicalJson(input); return check(input, kind === 'map' ? map : theme, kind === 'map' ? map : theme) } catch { return false }
}
