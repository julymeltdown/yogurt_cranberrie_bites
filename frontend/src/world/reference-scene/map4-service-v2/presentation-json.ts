import { requireMap } from '../../map/json'

/** Map illustration coordinates may be fractional. This does not widen the C06 wire parser. */
export function canonicalPresentationJson(value: unknown, depth = 0): string {
  requireMap(depth <= 32)
  if (value === null || typeof value === 'boolean') return String(value)
  if (typeof value === 'number') { requireMap(Number.isFinite(value)); return JSON.stringify(value) }
  if (typeof value === 'string') return JSON.stringify(value)
  if (Array.isArray(value)) return `[${value.map(v => canonicalPresentationJson(v, depth + 1)).join(',')}]`
  requireMap(!!value && typeof value === 'object')
  const record = value as Record<string, unknown>
  return `{${Object.keys(record).sort().map(key => `${JSON.stringify(key)}:${canonicalPresentationJson(record[key], depth + 1)}`).join(',')}}`
}
