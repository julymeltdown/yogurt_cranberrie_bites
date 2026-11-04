import { DOMAIN_ERROR_RULES, type DomainReason } from './domain-operations.generated'
/** C03 core and explicitly qualified C04 candidate errors; no backend implementation import. */
export const CORE_REASONS = [
  'PARTY_FULL', 'GIVEAWAY_SOLD_OUT', 'RESERVATION_EXPIRED', 'VERSION_CONFLICT',
  'CONSENT_REQUIRED', 'RESOURCE_UNAVAILABLE', 'MURAL_COOLDOWN', 'MAP_VERSION_MISMATCH',
  'EVENT_READ_ONLY', 'INVALID_INPUT', 'AUTHENTICATION_REQUIRED', 'PERMISSION_DENIED',
  'RATE_LIMITED', 'DEPENDENCY_UNAVAILABLE', 'PAYLOAD_TOO_LARGE', 'INVALID_MEDIA_TYPE',
  'INTERNAL_ERROR', 'DEADLINE_EXCEEDED',
] as const
export type CoreReason = typeof CORE_REASONS[number]
export type OriginReason = 'ORIGIN_NOT_ALLOWED' | 'BROWSER_INTENT_REQUIRED'
export type CoreErrorDetails =
  | { readonly reason: 'VERSION_CONFLICT'; readonly current_revision?: number }
  | { readonly reason: 'MURAL_COOLDOWN' | 'RATE_LIMITED'; readonly retry_after_ms?: number }
  | { readonly reason: Exclude<CoreReason, 'VERSION_CONFLICT' | 'MURAL_COOLDOWN' | 'RATE_LIMITED'> }
export type JsonValue = null | boolean | number | string | readonly JsonValue[] | { readonly [key: string]: JsonValue }
export interface ServerErrorEnvelope {
  readonly code: string
  readonly message: string
  readonly traceId?: string | null
  readonly timestamp?: string | null
  readonly details: Readonly<Record<string, JsonValue>> | null
}
export interface ParsedServerError {
  readonly envelope: ServerErrorEnvelope
  readonly core: CoreErrorDetails | null
  readonly reason: CoreReason | OriginReason | DomainReason | null
}
export type ErrorFamily = 'auth' | 'core' | 'domain'
export type TransportErrorKind = 'http' | 'network' | 'aborted' | 'timeout'
  | 'invalid-response' | 'invalid-request' | 'session-changed' | 'authentication-required'

/** Does not retain Request, Response, credentials, raw response bodies or underlying errors. */
export class TransportError extends Error {
  readonly kind: TransportErrorKind
  readonly status: number | null
  readonly code: string
  readonly server: ParsedServerError | null
  readonly retryAfterMs: number | null
  readonly uncertain: boolean
  constructor(kind: TransportErrorKind, options: {
    status?: number
    server?: ParsedServerError | null
    retryAfterMs?: number | null
    uncertain?: boolean
  } = {}) {
    super(options.status === undefined ? `Request failed: ${kind}.` : `Request failed (HTTP ${options.status}).`)
    this.name = 'TransportError'
    this.kind = kind
    this.status = options.status ?? null
    this.server = options.server ?? null
    this.code = this.server?.envelope.code ?? (this.status === null ? kind.toUpperCase().replaceAll('-', '_') : `HTTP_${this.status}`)
    this.retryAfterMs = options.retryAfterMs ?? null
    this.uncertain = options.uncertain ?? false
  }
}

export const MAX_RETRY_AFTER_MS = 86_400_000
const record = (value: unknown): value is Record<string, unknown> => value !== null
  && typeof value === 'object' && !Array.isArray(value)
const boundedString = (value: unknown, max: number, min = 1): value is string => typeof value === 'string'
  && value.length >= min && value.length <= max && Array.from(value).every(character => {
    const code = character.charCodeAt(0)
    return code !== 127 && (code >= 32 || code === 9 || code === 10 || code === 13)
  })
const integer = (value: unknown, max = Number.MAX_SAFE_INTEGER): value is number => Number.isSafeInteger(value)
  && (value as number) >= 1 && (value as number) <= max
const only = (value: Record<string, unknown>, keys: readonly string[]) => Object.keys(value).every(key => keys.includes(key))

export function parseCoreDetails(value: unknown): CoreErrorDetails | null {
  if (!record(value) || !CORE_REASONS.some(reason => reason === value.reason)) return null
  const reason = value.reason as CoreReason
  if (reason === 'VERSION_CONFLICT') {
    if (!only(value, ['reason', 'current_revision'])
      || ('current_revision' in value && !integer(value.current_revision))) return null
    return Object.freeze({ reason, ...('current_revision' in value ? { current_revision: value.current_revision as number } : {}) })
  }
  if (reason === 'RATE_LIMITED' || reason === 'MURAL_COOLDOWN') {
    if (!only(value, ['reason', 'retry_after_ms'])
      || ('retry_after_ms' in value && !integer(value.retry_after_ms, MAX_RETRY_AFTER_MS))) return null
    return Object.freeze({ reason, ...('retry_after_ms' in value ? { retry_after_ms: value.retry_after_ms as number } : {}) })
  }
  return only(value, ['reason']) ? Object.freeze({ reason }) : null
}

// Inherited auth details are open JSON. Bound and copy them without assigning arbitrary object keys.
function copyDetails(value: unknown, depth = 0, budget = { nodes: 0 }): JsonValue {
  if (++budget.nodes > 128 || depth > 4) throw new Error('Invalid error details')
  if (value === null || typeof value === 'boolean') return value
  if (typeof value === 'number' && Number.isFinite(value)) return value
  if (boundedString(value, 512, 0)) return value
  if (Array.isArray(value) && value.length <= 32) return Object.freeze(value.map(item => copyDetails(item, depth + 1, budget)))
  if (record(value) && Object.keys(value).length <= 32) {
    const result: Record<string, JsonValue> = Object.create(null)
    for (const [key, item] of Object.entries(value)) {
      if (!boundedString(key, 64) || ['__proto__', 'constructor', 'prototype'].includes(key)) throw new Error('Invalid error details')
      result[key] = copyDetails(item, depth + 1, budget)
    }
    return Object.freeze(result)
  }
  throw new Error('Invalid error details')
}

/** Parses the safe envelope, not an operation-specific schema or an English error message. */
export function parseServerError(value: unknown, family: ErrorFamily, status: number): ParsedServerError | null {
  if (!record(value) || !boundedString(value.code, 64) || !boundedString(value.message, 512)) return null
  if (family !== 'auth' && !only(value, ['code', 'message', 'traceId', 'timestamp', 'details'])) return null
  if ('traceId' in value && value.traceId !== null && !boundedString(value.traceId, 128, 0)) return null
  if ('timestamp' in value && value.timestamp !== null
    && (!boundedString(value.timestamp, 64) || !Number.isFinite(Date.parse(value.timestamp)))) return null
  const origin = record(value.details) && (value.details.reason === 'ORIGIN_NOT_ALLOWED'
    || value.details.reason === 'BROWSER_INTENT_REQUIRED')
  if ((family !== 'auth' || origin) && (
    !only(value, ['code', 'message', 'traceId', 'timestamp', 'details'])
    || (typeof value.timestamp === 'string' && !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/.test(value.timestamp))
  )) return null
  if (family === 'domain' && typeof value.timestamp === 'string'
    && new Date(value.timestamp).toISOString().slice(0, 19) !== value.timestamp.slice(0, 19)) return null
  let core: CoreErrorDetails | null = null
  let reason: CoreReason | OriginReason | DomainReason | null = null
  let details: Readonly<Record<string, JsonValue>> | null = null
  if (family === 'domain') {
    if (!record(value.details) || typeof value.details.reason !== 'string' || !Object.hasOwn(DOMAIN_ERROR_RULES, value.details.reason)) return null
    reason = value.details.reason as DomainReason
    const rule = DOMAIN_ERROR_RULES[reason]
    const hasHint = Object.hasOwn(value.details, 'retry_after_ms')
    if (rule ? !only(value.details, ['reason', 'retry_after_ms'])
      || (!hasHint && rule.required)
      || (hasHint && (!integer(value.details.retry_after_ms, rule.maximum) || value.details.retry_after_ms < rule.minimum))
      : !only(value.details, ['reason'])) return null
    if ((reason === 'ORIGIN_NOT_ALLOWED' || reason === 'BROWSER_INTENT_REQUIRED') && status !== 403) return null
    details = Object.freeze({ reason, ...(rule && hasHint ? { retry_after_ms: value.details.retry_after_ms as number } : {}) })
  } else if (record(value.details) && (value.details.reason === 'ORIGIN_NOT_ALLOWED' || value.details.reason === 'BROWSER_INTENT_REQUIRED')) {
    if (status !== 403 || !only(value.details, ['reason'])) return null
    reason = value.details.reason
    details = Object.freeze({ reason })
  } else if (family === 'core') {
    core = parseCoreDetails(value.details)
    if (!core) return null
    reason = core.reason
    details = core
  } else if (value.details !== undefined && value.details !== null) {
    if (!record(value.details)) return null
    try { details = copyDetails(value.details) as Readonly<Record<string, JsonValue>> } catch { return null }
  }
  const envelope: ServerErrorEnvelope = Object.freeze({
    code: value.code, message: value.message, details,
    ...('traceId' in value ? { traceId: value.traceId as string | null } : {}),
    ...('timestamp' in value ? { timestamp: value.timestamp as string | null } : {}),
  })
  return Object.freeze({ envelope, core, reason })
}

/** Bounded UI hint only. It never authorizes or schedules a replay. */
export function parseRetryAfter(value: string | null, now = Date.now()): number | null {
  if (value === null || value.length > 128) return null
  const text = value.trim()
  if (/^\d+$/.test(text)) return Math.min(Number(text) * 1000, MAX_RETRY_AFTER_MS)
  // Restrict to HTTP-date syntax; Date.parse also accepts misleading bare numbers.
  if (!/^(Mon|Tue|Wed|Thu|Fri|Sat|Sun), \d{2} [A-Z][a-z]{2} \d{4} \d{2}:\d{2}:\d{2} GMT$/.test(text)) return null
  const date = Date.parse(text)
  return Number.isFinite(date) ? Math.min(MAX_RETRY_AFTER_MS, Math.max(0, date - now)) : null
}
