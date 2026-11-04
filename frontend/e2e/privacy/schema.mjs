/** QA-only interpreter of the pinned public OpenAPI schema. Unknown assertions fail closed. */
const annotations = ['$schema', '$id', 'description', 'title', 'default', 'examples', 'deprecated', 'readOnly', 'writeOnly']
const assertions = ['$ref', 'type', 'const', 'enum', 'allOf', 'anyOf', 'oneOf', 'not', 'properties', 'required', 'additionalProperties', 'items', 'minItems', 'maxItems', 'uniqueItems', 'minLength', 'maxLength', 'pattern', 'format', 'minimum', 'maximum', 'exclusiveMinimum', 'exclusiveMaximum', 'multipleOf']
const keys = new Set([...annotations, ...assertions])
export function schemaAccepts(spec, schema, value, depth = 0) {
  if (depth > 50 || !schema || typeof schema !== 'object' || Object.keys(schema).some(key => !keys.has(key))) return false
  const next = (rule, data = value) => schemaAccepts(spec, rule, data, depth + 1)
  if (schema.$ref) {
    const match = /^#\/components\/schemas\/([A-Za-z0-9_]+)$/.exec(schema.$ref)
    if (!match || !next(spec.components.schemas[match[1]])) return false
  }
  if (schema.allOf && !schema.allOf.every(rule => next(rule))) return false
  if (schema.anyOf && !schema.anyOf.some(rule => next(rule))) return false
  if (schema.oneOf && schema.oneOf.filter(rule => next(rule)).length !== 1) return false
  if (schema.not && next(schema.not)) return false
  if ('const' in schema && JSON.stringify(value) !== JSON.stringify(schema.const)) return false
  if (schema.enum && !schema.enum.some(candidate => JSON.stringify(value) === JSON.stringify(candidate))) return false
  const object = value !== null && typeof value === 'object' && !Array.isArray(value)
  const types = { object, array: Array.isArray(value), string: typeof value === 'string', boolean: typeof value === 'boolean', null: value === null,
    integer: Number.isSafeInteger(value), number: typeof value === 'number' && Number.isFinite(value) }
  if (schema.type && types[schema.type] !== true) return false
  if (object) {
    if (!(schema.required ?? []).every(key => Object.hasOwn(value, key))) return false
    for (const [key, data] of Object.entries(value)) {
      if (schema.properties && Object.hasOwn(schema.properties, key)) { if (!next(schema.properties[key], data)) return false }
      else if (schema.additionalProperties === false) return false
      else if (typeof schema.additionalProperties === 'object' && !next(schema.additionalProperties, data)) return false
    }
  }
  if (Array.isArray(value)) {
    if (value.length < (schema.minItems ?? 0) || value.length > (schema.maxItems ?? Infinity)) return false
    if (schema.items && !value.every(item => next(schema.items, item))) return false
    if (schema.uniqueItems && new Set(value.map(item => JSON.stringify(item))).size !== value.length) return false
  }
  if (typeof value === 'string') {
    const length = Array.from(value).length
    if (length < (schema.minLength ?? 0) || length > (schema.maxLength ?? Infinity) || schema.pattern && !new RegExp(schema.pattern, 'u').test(value)) return false
    if (schema.format === 'uuid' && !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value)) return false
    if (schema.format === 'date-time' && (!/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?Z$/.test(value) || !Number.isFinite(Date.parse(value)) || new Date(value).toISOString().slice(0, 19) !== value.slice(0, 19))) return false
    if (schema.format === 'date' && (!/^\d{4}-\d\d-\d\d$/.test(value) || !Number.isFinite(Date.parse(value)) || new Date(value).toISOString().slice(0, 10) !== value)) return false
    if (schema.format && !['uuid', 'date-time', 'date'].includes(schema.format)) return false
  }
  if (typeof value === 'number') {
    if (value < (schema.minimum ?? -Infinity) || value > (schema.maximum ?? Infinity)
      || schema.exclusiveMinimum !== undefined && value <= schema.exclusiveMinimum || schema.exclusiveMaximum !== undefined && value >= schema.exclusiveMaximum
      || schema.multipleOf !== undefined && value % schema.multipleOf !== 0) return false
  }
  return true
}
