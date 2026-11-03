import { DOMAIN_OPERATIONS, DOMAIN_QUERIES, type DomainOperationId } from './domain-operations.generated'

interface ScalarRule {
  readonly type?: string
  readonly enum?: readonly unknown[]
  readonly format?: string
  readonly pattern?: string
  readonly minimum?: number
  readonly maximum?: number
  readonly minLength?: number
  readonly maxLength?: number
}
export interface DomainOperation {
  readonly method: string
  readonly path: string
  readonly parameters: Readonly<Record<string, ScalarRule>>
  readonly query: keyof typeof DOMAIN_QUERIES
  readonly idempotency: boolean
  readonly errors: Readonly<Record<string, readonly string[]>>
  readonly success: readonly number[]
}
export function domainOperation(id: DomainOperationId | undefined): DomainOperation | null {
  return id !== undefined && Object.hasOwn(DOMAIN_OPERATIONS, id) ? DOMAIN_OPERATIONS[id] : null
}
function validScalar(value: unknown, rule: ScalarRule): boolean {
  if (rule.enum && !rule.enum.includes(value)) return false
  if (rule.type === 'boolean') return typeof value === 'boolean'
  if (rule.type === 'integer') return Number.isSafeInteger(value) && typeof value === 'number'
    && value >= (rule.minimum ?? Number.MIN_SAFE_INTEGER) && value <= (rule.maximum ?? Number.MAX_SAFE_INTEGER)
  if (rule.type === 'string') {
    if (typeof value !== 'string' || Array.from(value).length < (rule.minLength ?? 0)
      || Array.from(value).length > (rule.maxLength ?? Number.MAX_SAFE_INTEGER)
      || (rule.pattern && !new RegExp(rule.pattern).test(value))) return false
    if (rule.format === 'date-time') {
      // UTC and real calendar dates; Date.parse alone normalizes invalid days.
      if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/.test(value)) return false
      const milliseconds = Date.parse(value)
      if (!Number.isFinite(milliseconds) || new Date(milliseconds).toISOString().slice(0, 19) !== value.slice(0, 19)) return false
    }
  }
  return true
}
/** Request qualification only; response DTO/context validation remains the feature decoder's job. */
export function matchesDomainRequest(op: DomainOperation, method: string, path: string,
  query: Readonly<Record<string, string | number | boolean>> = {}): boolean {
  if (op.method !== method) return false
  const expected = op.path.split('/'), actual = path.split('/')
  if (expected.length !== actual.length || !expected.every((part, i) => part.startsWith('{')
    ? Object.hasOwn(op.parameters, part.slice(1, -1)) && validScalar(actual[i], op.parameters[part.slice(1, -1)]!)
    : part === actual[i])) return false
  const schema: { readonly required: readonly string[]; readonly properties: Readonly<Record<string, ScalarRule>> } = DOMAIN_QUERIES[op.query]
  return schema.required.every(key => Object.hasOwn(query, key)) && Object.entries(query)
    .every(([key, value]) => Object.hasOwn(schema.properties, key) && validScalar(value, schema.properties[key]!))
}
