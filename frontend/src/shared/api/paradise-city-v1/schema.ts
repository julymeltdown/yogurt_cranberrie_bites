/** Bounded, strict decoder for this standalone JSON Schema family. No authority is created. */
export interface WireSchema {
  readonly $ref?: string
  readonly type?: string
  readonly const?: unknown
  readonly enum?: readonly unknown[]
  readonly oneOf?: readonly WireSchema[]
  readonly anyOf?: readonly WireSchema[]
  readonly not?: WireSchema
  readonly properties?: Readonly<Record<string, WireSchema>>
  readonly required?: readonly string[]
  readonly additionalProperties?: boolean
  readonly items?: WireSchema
  readonly minItems?: number
  readonly maxItems?: number
  readonly minLength?: number
  readonly maxLength?: number
  readonly minimum?: number
  readonly maximum?: number
  readonly pattern?: string
  readonly format?: string
  readonly 'x-maxUtf8Bytes'?: number
}

export class VenueContractError extends Error {
  constructor(readonly kind: 'invalid-request' | 'invalid-response' | 'scope-expired') {
    super(`Venue contract: ${kind}`)
    this.name = 'VenueContractError'
  }
}

/** Reject non-JSON values/accessors and detach every caller-owned nested reference. */
export function wireSnapshot(value: unknown): unknown {
  let nodes = 0
  function copy(input: unknown, depth: number): unknown {
    if (++nodes > 25_000 || depth > 32) throw new Error('Wire budget')
    if (input === null || typeof input === 'string' || typeof input === 'boolean') return input
    if (typeof input === 'number' && Number.isFinite(input)) return input
    if (typeof input !== 'object' || input === null || Object.getOwnPropertySymbols(input).length) throw new Error('Not JSON')
    const proto = Object.getPrototypeOf(input)
    if (!Array.isArray(input) && proto !== Object.prototype && proto !== null) throw new Error('Not a JSON object')
    const descriptors = Object.getOwnPropertyDescriptors(input)
    if (Object.values(descriptors).some(d => !Object.hasOwn(d, 'value'))) throw new Error('Accessor rejected')
    if (Array.isArray(input)) {
      if (input.length > 25_000 || Object.keys(input).length !== input.length) throw new Error('Sparse or extended array')
      return Object.freeze(input.map(v => copy(v, depth + 1)))
    }
    const result: Record<string, unknown> = Object.create(null)
    for (const [key, descriptor] of Object.entries(descriptors)) {
      if (!descriptor.enumerable) throw new Error('Hidden property')
      result[key] = copy(descriptor.value, depth + 1)
    }
    return Object.freeze(result)
  }
  return copy(value, 0)
}

const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
const dateTime = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/
export function utcMilliseconds(value: string): number {
  const parsed = Date.parse(value)
  if (!dateTime.test(value) || !Number.isFinite(parsed)
    || new Date(parsed).toISOString().slice(0, 19) !== value.slice(0, 19)) throw new Error('Invalid UTC timestamp')
  return parsed
}

export function matchesWire(value: unknown, schema: WireSchema, definitions: Readonly<Record<string, WireSchema>>, depth = 0): boolean {
  if (depth > 48) return false
  const match = (v: unknown, s: WireSchema) => matchesWire(v, s, definitions, depth + 1)
  if (schema.$ref) {
    const prefix = '#/components/schemas/'
    if (!schema.$ref.startsWith(prefix)) return false
    const target = definitions[schema.$ref.slice(prefix.length)]
    return !!target && match(value, target)
  }
  if (Object.hasOwn(schema, 'const') && value !== schema.const) return false
  if (schema.enum && !schema.enum.includes(value)) return false
  if (schema.oneOf && schema.oneOf.filter(s => match(value, s)).length !== 1) return false
  if (schema.anyOf && !schema.anyOf.some(s => match(value, s))) return false
  if (schema.not && match(value, schema.not)) return false
  switch (schema.type) {
    case undefined: break
    case 'null': if (value !== null) return false; break
    case 'boolean': if (typeof value !== 'boolean') return false; break
    case 'number': case 'integer': {
      if (typeof value !== 'number' || !Number.isFinite(value)
        || schema.type === 'integer' && !Number.isSafeInteger(value)
        || value < (schema.minimum ?? -Infinity) || value > (schema.maximum ?? Infinity)) return false
      break
    }
    case 'string': {
      if (typeof value !== 'string') return false
      const scalars = Array.from(value)
      if (scalars.some(c => { const n = c.codePointAt(0)!; return n >= 0xd800 && n <= 0xdfff })) return false
      if (scalars.length < (schema.minLength ?? 0) || scalars.length > (schema.maxLength ?? Infinity)
        || schema.pattern && !new RegExp(schema.pattern).test(value)
        || schema.format === 'uuid' && !uuid.test(value)
        || schema['x-maxUtf8Bytes'] !== undefined && new TextEncoder().encode(value).length > schema['x-maxUtf8Bytes']) return false
      if (schema.format === 'date-time') { try { utcMilliseconds(value) } catch { return false } }
      break
    }
    case 'array': {
      if (!Array.isArray(value) || value.length < (schema.minItems ?? 0) || value.length > (schema.maxItems ?? Infinity)
        || !schema.items || !value.every(v => match(v, schema.items!))) return false
      break
    }
    case 'object': {
      if (value === null || typeof value !== 'object' || Array.isArray(value)) return false
      const record = value as Record<string, unknown>, properties = schema.properties ?? {}
      if (schema.required?.some(key => !Object.hasOwn(record, key))) return false
      if (schema.additionalProperties === false && Object.keys(record).some(key => !Object.hasOwn(properties, key))) return false
      if (Object.entries(properties).some(([key, child]) => Object.hasOwn(record, key) && !match(record[key], child))) return false
      break
    }
    default: return false
  }
  return true
}

export function decodeWire<T>(value: unknown, schema: WireSchema, definitions: Readonly<Record<string, WireSchema>>,
  kind: 'invalid-request' | 'invalid-response'): T {
  try {
    const detached = wireSnapshot(value)
    if (!matchesWire(detached, schema, definitions)) throw new Error('Schema mismatch')
    return detached as T
  } catch { throw new VenueContractError(kind) }
}
