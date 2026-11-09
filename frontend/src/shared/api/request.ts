/**
 * The one place a feature turns an `HttpClient` call into decoded data.
 *
 * Every feature client in `src/features/*\/api` and every controller that talks
 * to the server was re-deriving the same three things:
 *
 * * `request(...).then(result => result.data)` — the transport returns
 *   `{ data, scope }`, and every caller only ever wanted `data`;
 * * the same `TransportError` predicates — "is this a conflict", "is this the
 *   opaque 404", "is this worth retrying";
 * * the same `crypto.randomUUID` idempotency-key fallback for environments
 *   without it.
 *
 * This module is that derivation, written once. It deliberately does not wrap
 * the transport: callers still build their own `HttpRequest`, so the frozen
 * operation id, path, body shape and decoder stay visible at the call site, and
 * the shared transport's validation is unchanged.
 */
import type { HttpClient, HttpRequest } from './http'
import { TransportError } from './errors'

/** The single transport entry point every feature client accepts. */
export interface RequestPort {
  readonly request: HttpClient['request']
}

/**
 * Send one request and return its decoded body.
 *
 * The `scope` the transport returns is intentionally dropped here: a caller that
 * needs it (the avatar persistence, which proves the answer belongs to the
 * account it asked for) calls `request` directly. Dropping it everywhere else is
 * what stops a feature from quietly trusting a scope it never checked.
 */
export async function send<T>(port: RequestPort, request: HttpRequest<T>): Promise<T> {
  const result = await port.request(request)
  return result.data
}

/** Whether a failure is a revision conflict (`409`). */
export function isConflict(error: unknown): boolean {
  return error instanceof TransportError && error.status === 409
}

/**
 * Whether a failure is the opaque "unavailable" answer.
 *
 * `404` is the product's single answer for "absent", "not yours" and "you may
 * not see this". A client must treat all three the same, so this predicate is
 * the one place that decision is written.
 */
export function isUnavailable(error: unknown): boolean {
  return error instanceof TransportError && error.status === 404
}

/** Whether a failure is worth retrying: a network blip or a 5xx, never a 4xx. */
export function isTransient(error: unknown): boolean {
  if (!(error instanceof TransportError)) return false
  if (error.kind === 'network' || error.kind === 'timeout') return true
  return typeof error.status === 'number' && error.status >= 500
}

/** Whether the caller cancelled the request. */
export function isAborted(error: unknown): boolean {
  return error instanceof TransportError && error.kind === 'aborted'
}

/**
 * One idempotency key.
 *
 * `crypto.randomUUID` is the production source. The fallback exists only for a
 * non-browser test environment, keeps the transport's UUID grammar satisfied,
 * and never derives authority.
 */
export function newIdempotencyKey(): string {
  const crypto = globalThis.crypto
  if (crypto && typeof crypto.randomUUID === 'function') return crypto.randomUUID()
  const bytes = Array.from({ length: 16 }, () => Math.floor(Math.random() * 256))
  bytes[6] = ((bytes[6] ?? 0) & 0x0f) | 0x40
  bytes[8] = ((bytes[8] ?? 0) & 0x3f) | 0x80
  const hex = bytes.map((byte) => byte.toString(16).padStart(2, '0')).join('')
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}

/** Drop empty query values so a closed query parser never sees an empty field. */
export function cleanQuery(
  query: Readonly<Record<string, string | number | undefined>>,
): Record<string, string | number> {
  const result: Record<string, string | number> = {}
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined && value !== '') result[key] = value
  }
  return result
}

/**
 * Retry a transient failure, bounded.
 *
 * Only the failures `isTransient` accepts are retried; a 4xx is returned to the
 * caller on the first attempt, because retrying a rejection is how a client
 * turns one refusal into a rate limit. The attempt is cancelled between tries
 * when `signal` aborts.
 */
export async function withRetry<T>(
  run: () => Promise<T>,
  options: { readonly retries?: number; readonly signal?: AbortSignal } = {},
): Promise<T> {
  const retries = options.retries ?? 1
  let lastError: unknown
  for (let index = 0; index <= retries; index += 1) {
    if (options.signal?.aborted) throw new TransportError('aborted')
    try {
      return await run()
    } catch (error) {
      lastError = error
      if (!isTransient(error) || index === retries) throw error
    }
  }
  throw lastError
}
